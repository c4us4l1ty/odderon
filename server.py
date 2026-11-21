#!/usr/bin/env python3
"""Odderon dev server.

No dependencies, no build step, no bundler. Serves the repository root with the two headers the
architecture needs:

    Cross-Origin-Opener-Policy:   same-origin
    Cross-Origin-Embedder-Policy: require-corp

...because SharedArrayBuffer (the sim worker <-> renderer shared state ring, PERFORMANCE.md I1) is
only available in a cross-origin-isolated context. The game degrades gracefully without them, but
this is how it is meant to run.

    python3 server.py            # http://localhost:8080
    python3 server.py --port 3000
    python3 server.py --host 0.0.0.0

Bind to 0.0.0.0 by default so that a browser outside this machine (a phone on the same network, or a
sandbox preview proxy) can reach it.
"""
import argparse
import os
import socket
import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.abspath(__file__))


class Handler(SimpleHTTPRequestHandler):
    extensions_map = {
        **SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
        ".mjs": "text/javascript",
        ".wasm": "application/wasm",
        ".glsl": "text/plain",
        ".json": "application/json",
    }

    def end_headers(self):
        # cross-origin isolation: enables SharedArrayBuffer for the sim worker
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        # no caching, so a pull is always what you play
        self.send_header("Cache-Control", "no-store, max-age=0")
        super().end_headers()

    def log_message(self, fmt, *args):
        # quieter: only errors, so the terminal stays readable
        code = args[1] if len(args) > 1 else ""
        if str(code).startswith(("4", "5")):
            sys.stderr.write("  %s %s\n" % (self.address_string(), fmt % args))


def free_port(host, preferred):
    for port in [preferred] + list(range(preferred + 1, preferred + 20)):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                s.bind((host, port))
                return port
            except OSError:
                continue
    raise SystemExit("no free port near %d" % preferred)


def main():
    ap = argparse.ArgumentParser(description="Odderon dev server")
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--host", default="0.0.0.0")
    ap.add_argument("--root", default=ROOT)
    args = ap.parse_args()

    port = free_port("0.0.0.0" if args.host == "0.0.0.0" else args.host, args.port)
    handler = partial(Handler, directory=args.root)
    httpd = ThreadingHTTPServer((args.host, port), handler)

    print()
    print("  ODDERON — Block 1: The Room")
    print("  ------------------------------------------------------------")
    print("  http://localhost:%d/" % port)
    if args.host == "0.0.0.0":
        try:
            ip = socket.gethostbyname(socket.gethostname())
            print("  http://%s:%d/   (from another device on this network)" % (ip, port))
        except OSError:
            pass
    print()
    print("  controls:  WASD walk · mouse look · H help")
    print("             E curtain · F air conditioner")
    print("             M mark the gauge · R let reality answer")
    print("             V visor · P provenance · G ghost twin")
    print()
    print("  tests:     node test/run.mjs")
    print("  stop:      ctrl-c")
    print()
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n  stopped.\n")


if __name__ == "__main__":
    main()
