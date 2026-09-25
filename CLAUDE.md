# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Fiber-optic cable / duct / patch-panel planning tool. Single deployable binary: a Rust (actix-web) backend serving a GraphQL API plus an embedded Yew (WASM) SPA. Data lives in PostgreSQL + PostGIS; planned panel/port changes can be synced to NetBox. Domain names are partly German (`kabel` = cable, `schacht` = manhole/cabinet, `trasse` = duct route, `sequenz`); code comments and log messages are mixed German/English.

## Workspace layout

Cargo workspace (`default-members = cable-editor-binary`):

- `cable-editor-backend` — library: Diesel models/migrations, async-graphql schemas, NetBox client, INTERLIS2 export (`export.rs`).
- `cable-editor-binary` — actix-web server (`main.rs`): wires OIDC/JWT auth, DB pool, Prometheus, and embeds `cable-editor-frontend/dist` via `rust-embed` (SPA fallback to `index.html`).
- `cable-editor-frontend` — Yew + PatternFly + Leaflet SPA, built with Trunk for `wasm32-unknown-unknown`. Not built by plain `cargo build` from the root. Label printing uses the external `brady-web-sdk` crate (git dependency, `https://git.panter.ch/open-source/brady-web-sdk-rs.git`, pinned in `Cargo.lock`).
- `cable-editor-chart` — Helm chart; CI (`.github/workflows/build-and-publish.yml`) pushes the Docker image and chart to GHCR on pushes to `master`.

## Commands

```sh
# Local dev environment (Postgres/PostGIS + Keycloak via podman, writes .env and config.yaml, then `trunk serve`)
local/run-local.sh
local/import-data.sh              # load local/data.sql into the cable-db container
local/mock/run-mock.sh            # frontend on :8099 against a mock backend (no DB/Keycloak): fake OIDC login, in-memory GraphQL data
node local/mock/screenshot.mjs [--desktop] [--full] /plan/0/cabinet/1/overview   # Playwright screenshots (phone by default), reports layout overflow and console errors

# Frontend (from cable-editor-frontend/; pre_build hook runs `npm install` in node/)
trunk serve                       # dev server on :8082, proxies /graphql and /graphql_anonymous to :8080
trunk build [--release]           # outputs dist/, which the binary embeds

# Backend server (root) — needs DATABASE_URL (from .env) and config.yaml
cargo run                         # API on :8080, metrics/health on :9080 (port + 1000)
cargo build -p cable-editor-frontend --target wasm32-unknown-unknown   # typecheck frontend
cargo clippy --workspace          # (frontend needs the wasm target to compile cleanly)
# The frontend build script links the backend, so libpq must be installed (-lpq) even for wasm checks.

# Release container
docker build .
```

There are currently no tests in the repo.

**Build order matters:** the binary embeds `cable-editor-frontend/dist` at compile time, so run `trunk build` before building the binary (the Dockerfile does frontend first, then the musl backend build).

## Commits

- One focused commit per logical change; split unrelated work (e.g. a refactor and the feature built on it) into separate commits.
- Subject: short, lowercase, imperative, no trailing period (e.g. `print panel labels`).
- Body: why the change was needed and anything non-obvious, wrapped at ~72 columns.
- Update CLAUDE.md in the same commit as the change it describes.
- Commits by Claude end with the `Co-Authored-By: Claude …` trailer, without a session link.
- Pushing to `master` publishes the image and chart (CI), so only push when asked.

## Configuration

- `DATABASE_URL` env var (`.env`, loaded by dotenvy). Migrations in `cable-editor-backend/migrations` are embedded and run automatically at startup (`run_sync_migrations`).
- `config.yaml` (gitignored) with `oauth:` (`auth_client_id`, `auth_issuer`, optional `user_info_url` (default: `userinfo_endpoint` from the issuer's OIDC discovery), optional `auth_scopes` (space separated scopes the frontend requests, default `openid profile groups`), `server_port`, ...) and `netbox:` (`url`, `token`, `provider_id`, `type_id`) sections; each value can be overridden by env vars with prefix `APP` and `__` separator (see `backend/src/config.rs`).
- `LOG_LEVEL` env var controls `env_logger`.

## Architecture

### GraphQL: two schemas, typed end-to-end with cynic
- Backend exposes `/graphql` (authenticated: `graphql/authenticated/` — `Query`, `mutation/`, `planned.rs`) and `/graphql_anonymous` (`graphql/anonymous.rs`).
- `cable-editor-frontend/build.rs` depends on the backend crate, calls `create_authenticated_schema()` / `create_anonymous_schema()`, writes the SDL to `cable-editor-frontend/graphql/*.graphql` (gitignored), and registers them with `cynic_codegen`. Frontend queries are `cynic::QueryFragment` structs in `frontend/src/graphql/{authenticated,anonymous}/*.rs`, so **any backend schema change is checked at frontend compile time** — change the backend resolver, then fix the frontend fragments.
- `cable-editor-backend/build.rs` registers `schema/schema.graphql` (the NetBox GraphQL schema) for the cynic-based NetBox client in `src/netbox/`.

### Per-request transaction
In `binary/src/main.rs`, each authenticated GraphQL request: validates the JWT (`actix-4-jwt-auth`), fetches OIDC userinfo (cached 30s) into `UserInfo`, opens a pooled connection, runs `BEGIN`, and puts it into the request data as `Arc<tokio::Mutex<Object<AsyncPgConnection>>>`. Resolvers obtain it via `graphql::authenticated::get_connection(ctx)`. The whole request is `COMMIT`ed if there are no errors, otherwise `ROLLBACK`ed — so resolvers should return errors rather than manage transactions themselves (nested `connection.transaction(...)` becomes a savepoint, e.g. `mutation/implement.rs`).

### Plans
Changes to panels/ports are made inside a `plan` (status `Open` / `Implemented` / `Rejected`). `planned.rs` exposes `PlannedPanel` / `PlannedPort` (entity + plan context); `port_usage` rows are plan-scoped. `mutation/implement.rs` applies an open plan; `mutation/sync.rs` diffs a plan against NetBox devices/rear ports and reports `SyncIssue`s before syncing.

### Database
Diesel 2 with `diesel-async` + deadpool; PostGIS geometries via `postgis_diesel`. Table definitions are in `backend/src/db/schema.rs`, entities in `backend/src/db/entity/`. Note `diesel.toml` points `print_schema` at `src/schema.rs`, but the live file is `src/db/schema.rs` — move/merge the output if you regenerate with `diesel migration run`.

### Frontend
`pages/router.rs` defines `AppRoute` (yew-nested-router); plan-scoped views are nested under `AppRoute::Plan { plan_id, view }`. `AppRoute::content` builds PatternFly's page layout by hand (no `Page` component, no masthead/sidebar): `<main>` with a breadcrumb `PageSection` (menus in `components/menu/`) followed by the page. The breadcrumb is one hierarchy Plan › area › Schacht › panels › view: the menu of each object (`ListCabinet`, `ListPanel`) holds its views plus the objects at the same level, the last item the current view plus the objects one level down (a Schacht's root panels, a panel's children). Menus use the own popper-free `PopupMenu` (`components/menu/popup.rs`, see its doc comment) and value choices a native `FormSelect`; patternfly-yew's popper-based `Dropdown`/`SimpleSelect` log console errors and are not used. Every routed page wraps its output in `components/page_layout.rs` `PageLayout` (h1 title section + content section; title `"<view> – <object>"` via `object_title`, just the view while loading), so headings inside a page start at `h2`; `.app-page` in `style.scss` lets the document scroll instead of the main container, so pages must not add their own outer padding. Pages live in `pages/`, reusable pieces in `components/`. `PlanView::Cabinet { view: CabinetView::Overview }` (`pages/cabinet/overview.rs`) is the Schacht overview, built for phones in the field: its panels (links to their connection overview, panel labels) and the cables ending there (cable labels). Editing its panels is a separate page, `CabinetView::Edit` (`pages/cabinet/edit.rs`); the breadcrumb switches between the two. On phones, tree tables only show cells with a column label, and `PopupMenu`s open as a sheet at the bottom. Auth uses `yew-oauth2` (OpenID); `graphql::query`/`mutate` attach the bearer token from `OAuth2Context`. The GraphQL URL is derived from `window.location`, which is why `trunk serve` proxies to the backend.

### Label printing (Brady M211)
- Printer: Brady M211 over Web Bluetooth (Chrome/Edge only), tape M21-1250-427 (self-laminating, 1.25" wide, continuous). Label text is `"<cable> - <far schacht>"` (`graphql/authenticated/schacht_cables.rs`; `Schacht.cables` paths are oriented from that Schacht, so `farSchacht` is the destination).
- Wiring: `@bradycorporation/brady-web-sdk` in `node/package.json`; `index.html` maps it with an importmap (must precede any module script) and copies its whole `dist/` to `/brady-web-sdk/` (the bundle loads its wasm/pdf.js relative to itself). `BradyProvider` wraps the `BackdropViewer` in `pages/mod.rs` so the connection survives navigation and dialogs (rendered by the viewer) can `use_brady()`; `PrinterStatusBar` next to the router shows the connection (icon button to connect/disconnect, printer, tape, battery) fixed at the bottom of every page, only in browsers with Web Bluetooth.
- `components/label_printer.rs`: `PrintLabelButton` connects, waits for the reported tape and opens a dialog with a live preview (the geometry follows the printer status, e.g. a cartridge change while it is open), a choice between its `texts` and, with `diameter`, the cable diameter (last value in `localStorage`). Cable labels come from the Schacht overview, panel labels (`PanelLabelButton`: name or path from the root panel joined with " - ", no diameter) from the Schacht overview and the panel overview (`components/panel/show.rs`); label rendering on a canvas in Barlow Bold (DIN-like, npm `@fontsource/barlow`, copied by `index.html`, declared in `style.scss`; its straight shapes show fewer steps at 203 dpi than Roboto), loaded before the dialog opens. Text height is capped at 70% of the diameter; small text is repeated across the band (as many copies as fit, with half a text height between them, within the 80% a maximal line takes: 3 mm → 3, 4 mm → 2, from ~5 mm → 1) so it reads from more sides of the cable (`text_layout`). The label is cropped to the glyphs along the tape.
- `LabelGeometry::from_status` derives the layout from the tape the printer reports (`PrinterStatus::print_zone`, `supply_width`, model), mirroring the SDK: text band = print zone width, else the tape width covered by the head (M211 0.63", M511 1.44"). Die-cut labels are refused. Verified offline for M21-1250-427 and M21-750-499.
- SDK quirks handled there, verify on hardware before changing:
  - Printing before the printer reported its supply fails (`Invalid typed array length: -1`), so it waits for `supply_width`.
  - The SDK scales the image height to the text band (0.43" zone on M21-1250-427); label length follows the image aspect ratio. The M211 adds ~0.87" blank feed per label (hard-coded in the SDK).
  - For tapes with a print zone the SDK places the zone without subtracting the tape the head does not cover (0.62" on the 1.25" tape), which yields an empty job and a print timeout; the geometry shifts it back via the x print offset.
  - `supplyYNumber` is numeric (e.g. `4900577`), not the part name.
- Updating the crate: push it, then change only its rev in `Cargo.lock` (`cargo update -p brady-web-sdk` also churns unrelated `windows-sys` entries). To test local crate changes before pushing: `--config 'patch."https://git.panter.ch/open-source/brady-web-sdk-rs.git".brady-web-sdk.path="../brady-web-sdk"'` (revert `Cargo.lock` afterwards).
