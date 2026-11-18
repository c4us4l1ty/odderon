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

## Flight Deck stations

The Flight Deck is a fixed-height command console rather than a stacked web
page. Its left station rail switches one active operational workspace at a time:

- **FDO / Flight:** contract selection, orbit viewport and maneuver controls;
- **ACD / Academy:** placement and lessons docked above the live flight controls;
- **PRP / Engine:** engine geometry, Schlieren view and fitted performance;
- **VHC / Vehicle:** delegation, tanks, thermal hardware and electrical sizing;
- **PCL / Predict:** prediction contract and launch commitment;
- **FDR / Debrief:** unlocked after execution and opened automatically.

The top command strip keeps the active station, operation, frame and authority
mode visible. On narrow screens the station rail becomes a bottom game bar.
Profile, leaderboard and verification overlays share the same flight-computer
visual language. The Observatory is also an instrument console and links
straight back to the game menu and Flight Deck. On desktop, all three primary
routes remain inside one browser viewport with no document-level scrolling or
website footer; narrow layouts scroll only inside their active game workspace.

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

Provisioning prefers an installed Rust/rustup toolchain and builds
`web/odd_view.wasm`; the generated module is intentionally ignored by Git. It
does not require GCC and does not install headless-browser test dependencies.
Subsequent launches need only `bun run dev` unless Rust/WASM sources changed.
The Bun-native development server has no downloaded server package and prints
the local URL. Contributors running the complete acceptance suite use
`./scripts/provision.sh --full`.

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
- all six Flight Deck station transitions and the automatic debrief transition;
- Academy access to the live flight controls;
- Observatory rendering and zero-idle behavior;
- the existing mission, signature, replay and authority paths.

The shipped UI is a game shell and operational console, not a storefront or
marketing site. It intentionally remains a dense engineering simulation: it
does not claim a free-flight 3D cockpit, walking character, or finished campaign.
