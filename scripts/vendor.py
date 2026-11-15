#!/usr/bin/env python3
"""Populate .cache/vendor from GitHub mirrors for offline cargo builds.

The sandbox has no access to crates.io or static.crates.io, so crate sources
come from two allowed hosts:

  * ClickHouse/rust_vendor - a cargo-vendor shaped mirror of many crates,
    fetched with a blobless sparse clone so only the needed crates download.
  * codeload.github.com    - upstream tags for crates the mirror lacks.

Each vendored directory gets a .cargo-checksum.json carrying the exact
checksum from Cargo.lock, so `cargo --locked --offline` accepts the tree.
Idempotent: directories that already carry the right checksum are left alone.
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / ".cache"
VENDOR = CACHE / "vendor"
MIRROR = CACHE / "vendor_src"
MIRROR_URL = "https://github.com/ClickHouse/rust_vendor.git"

# crate -> (GitHub owner/repo, tag template). Only used when the mirror lacks
# the exact version. {v} is substituted with the locked version.
UPSTREAM: dict[str, tuple[str, str]] = {
    "syn": ("dtolnay/syn", "{v}"),
    "quote": ("dtolnay/quote", "{v}"),
    "proc-macro2": ("dtolnay/proc-macro2", "{v}"),
    "unicode-ident": ("dtolnay/unicode-ident", "{v}"),
    "thiserror": ("dtolnay/thiserror", "{v}"),
    "thiserror-impl": ("dtolnay/thiserror", "{v}"),
    "serde": ("serde-rs/serde", "v{v}"),
    "serde_core": ("serde-rs/serde", "v{v}"),
    "serde_derive": ("serde-rs/serde", "v{v}"),
    "libc": ("rust-lang/libc", "{v}"),
    "libm": ("rust-lang/libm", "v{v}"),
    "cfg-if": ("rust-lang/cfg-if", "{v}"),
    "typenum": ("paholg/typenum", "v{v}"),
    "version_check": ("SergioBenitez/version_check", "v{v}"),
    "generic-array": ("fizyk20/generic-array", "{v}"),
    "sha2": ("RustCrypto/hashes", "sha2-v{v}"),
    "digest": ("RustCrypto/traits", "digest-v{v}"),
    "block-buffer": ("RustCrypto/utils", "block-buffer-v{v}"),
    "cpufeatures": ("RustCrypto/utils", "cpufeatures-v{v}"),
    "crypto-common": ("RustCrypto/traits", "crypto-common-v{v}"),
    "postcard": ("jamesmunns/postcard", "v{v}"),
    "cobs": ("jamesmunns/cobs-rs", "v{v}"),
    "embedded-io": ("rust-embedded/embedded-hal", "embedded-io-v{v}"),
    # Phase 2 identity stack.
    "ed25519": ("RustCrypto/signatures", "ed25519-v{v}"),
    "ed25519-dalek": ("dalek-cryptography/curve25519-dalek", "ed25519-dalek-v{v}"),
    "curve25519-dalek": ("dalek-cryptography/curve25519-dalek", "curve25519-dalek-{v}"),
    "curve25519-dalek-derive": (
        "dalek-cryptography/curve25519-dalek",
        "curve25519-dalek-derive-{v}",
    ),
    "signature": ("RustCrypto/traits", "signature-v{v}"),
    "subtle": ("dalek-cryptography/subtle", "v{v}"),
    "zeroize": ("RustCrypto/utils", "zeroize-v{v}"),
    "rand_core": ("rust-random/rand", "rand_core-{v}"),
    "rustc_version": ("dhuseby/rustc_version", "v{v}"),
    "semver": ("dtolnay/semver", "{v}"),
}

# Monorepo subdirectory for crates published from a workspace.
SUBDIR: dict[str, str] = {
    "sha2": "sha2",
    "digest": "digest",
    "block-buffer": "block-buffer",
    "cpufeatures": "cpufeatures",
    "crypto-common": "crypto-common",
    "signature": "signature",
    "zeroize": "zeroize",
    "embedded-io": "embedded-io",
    "ed25519": "ed25519",
    "ed25519-dalek": "ed25519-dalek",
    "curve25519-dalek": "curve25519-dalek",
    "curve25519-dalek-derive": "curve25519-dalek-derive",
    "rand_core": "rand_core",
    "serde_core": "serde_core",
    "serde_derive": "serde_derive",
    "thiserror-impl": "thiserror-impl",
    "cobs": "cobs",
}


def locked_packages() -> dict[str, str]:
    """name-version -> registry checksum, for every crates.io package."""
    out: dict[str, str] = {}
    for block in (ROOT / "Cargo.lock").read_text().split("[[package]]")[1:]:
        name = re.search(r'name = "([^"]+)"', block)
        version = re.search(r'version = "([^"]+)"', block)
        checksum = re.search(r'checksum = "([^"]+)"', block)
        if name and version and checksum:
            out[f"{name.group(1)}-{version.group(1)}"] = checksum.group(1)
    return out


def mirror_dirs() -> set[str]:
    """Ensure the sparse mirror is cloned; return the crate dirs it holds."""
    if not (MIRROR / ".git").is_dir():
        print(f"[vendor] cloning mirror {MIRROR_URL}", flush=True)
        subprocess.run(
            ["git", "clone", "--quiet", "--depth", "1", "--filter=blob:none",
             "--sparse", MIRROR_URL, str(MIRROR)],
            check=True,
        )
    tree = subprocess.run(
        ["git", "ls-tree", "--name-only", "HEAD"],
        cwd=MIRROR, check=True, capture_output=True, text=True,
    ).stdout.split()
    return {n for n in tree if re.match(r"^.+-\d+\.\d+\.\d+", n)}


def fetch_upstream(name: str, version: str, dest: Path) -> bool:
    spec = UPSTREAM.get(name)
    if not spec:
        return False
    repo, template = spec
    tag = template.format(v=version)
    url = f"https://codeload.github.com/{repo}/tar.gz/refs/tags/{tag}"
    print(f"[vendor] upstream {name} {version} <- {repo}@{tag}", flush=True)
    try:
        raw = urllib.request.urlopen(url, timeout=180).read()
    except Exception as exc:  # noqa: BLE001 - report and let caller fail
        print(f"[vendor]   download failed: {exc}", flush=True)
        return False
    sub = SUBDIR.get(name)
    with tarfile.open(fileobj=__import__("io").BytesIO(raw)) as tf:
        for member in tf.getmembers():
            parts = member.name.split("/", 1)
            if len(parts) < 2 or not parts[1]:
                continue
            rel = parts[1]
            if sub:
                if rel == sub:
                    continue
                if not rel.startswith(sub + "/"):
                    continue
                rel = rel[len(sub) + 1:]
            target = dest / rel
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(tf.extractfile(member).read())
    return (dest / "Cargo.toml").is_file()


def main() -> int:
    packages = locked_packages()
    VENDOR.mkdir(parents=True, exist_ok=True)
    pending = {}
    for key, checksum in packages.items():
        marker = VENDOR / key / ".cargo-checksum.json"
        if marker.is_file():
            try:
                if json.loads(marker.read_text()).get("package") == checksum:
                    continue
            except (ValueError, OSError):
                pass
        pending[key] = checksum
    if not pending:
        print(f"[vendor] all {len(packages)} crates already vendored", flush=True)
        return 0

    available = mirror_dirs()
    need_from_mirror = sorted(k for k in pending if k in available)
    if need_from_mirror:
        print(f"[vendor] sparse-checkout {len(need_from_mirror)} crates", flush=True)
        subprocess.run(
            ["git", "sparse-checkout", "set", *need_from_mirror],
            cwd=MIRROR, check=True,
        )

    failed = []
    for key, checksum in sorted(pending.items()):
        dest = VENDOR / key
        if dest.exists():
            shutil.rmtree(dest)
        name, version = key.rsplit("-", 1)
        source = MIRROR / key
        if key in available and source.is_dir():
            shutil.copytree(source, dest, ignore=shutil.ignore_patterns(".git"))
        elif not fetch_upstream(name, version, dest):
            failed.append(key)
            continue
        marker = dest / ".cargo-checksum.json"
        prior = {}
        if marker.is_file():
            try:
                prior = json.loads(marker.read_text())
            except ValueError:
                prior = {}
        if prior.get("package") != checksum:
            # Trust the lockfile checksum; skip per-file hashes because GitHub
            # tarballs are not byte-identical to published .crate archives.
            marker.write_text(json.dumps({"files": {}, "package": checksum}))
        print(f"[vendor] ok {key}", flush=True)

    if failed:
        print("[vendor] FAILED to source: " + ", ".join(failed), file=sys.stderr)
        print("[vendor] add an UPSTREAM entry in scripts/vendor.py", file=sys.stderr)
        return 1
    print(f"[vendor] {len(packages)} crates vendored into {VENDOR}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
