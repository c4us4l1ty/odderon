# Portable game shell

Odderon's root route is the game, not a storefront or product landing page. The
current shell is a full-viewport title and command screen with no document
scroll, recurring timer, animation loop or hosting-provider dependency.

## Player routes

| Route | Player-facing role |
| --- | --- |
| `/` | Title screen and main game menu |
| `/mission.html` | Flight Deck: contracts, Academy, engineering and debrief |
| `/mission.html#onboarding` | Academy entry point |
| `/observatory.html` | Earth–Moon Observatory reference screen |

The title screen exposes Enter Flight Deck, Academy, Observatory and Settings.
The primary action receives initial keyboard focus, so Enter opens the Flight
Deck without a pointer. Settings are local-device preferences. Reduced visual
effects removes the grain and backdrop filters; high contrast increases menu
separation. Neither setting writes to a server.

The title screen checks whether `odd_view.wasm` is present and reports the
flight computer as ready or not built. It does not instantiate a second physics
runtime. A missing module does not disguise itself as a successful build.

## Storefront and hosting boundary

A storefront may link to the deployed root route, but it is not part of the
game. Core navigation and simulation use relative static URLs and do not call a
Cloudflare-specific browser API. The same web build can therefore be served by
Cloudflare Pages, another static host, a local HTTP server, or a future desktop
webview wrapper.

Ranked authority is a separate optional deployment adapter. A static build is
practice-only unless its origin also provides the authority API and pins the
matching public key. Moving the static game between hosts must not silently turn
practice into ranked play.

## Local build and launch

On a fresh checkout:

```sh
./scripts/provision.sh
bun run dev
```

Provisioning builds `web/odd_view.wasm`; the generated module is intentionally
ignored by Git. Subsequent launches need only `bun run dev` unless Rust/WASM
sources changed. The development command serves `web/` and prints the local URL.

The equivalent server-independent build sequence remains:

```sh
cargo build --locked --release -p odd-view --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_view.wasm web/odd_view.wasm
```

## Acceptance boundary

The browser gate verifies the title route in real Chromium before entering the
Observatory and Flight Deck. It checks:

- the WASM-ready state;
- primary keyboard focus and Flight Deck target;
- settings behavior and local persistence;
- title artwork application;
- desktop and mobile horizontal containment;
- Observatory rendering and zero-idle behavior;
- the existing mission, signature, replay and authority paths.

The shell makes the application recognizable and launchable as a game. It does
not claim that the current long-form Flight Deck has completed its planned
single-viewport mission-control redesign; that remains a focused UI work item.
