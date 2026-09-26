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
cargo run                         # API on :8080, /metrics, /health (liveness) and /ready (DB reachable) on :9080 (port + 1000)
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
- `config.yaml` (gitignored) with `oauth:` (`auth_client_id`, `auth_issuer`, optional `user_info_url` (default: `userinfo_endpoint` from the issuer's OIDC discovery), optional `auth_scopes` (space separated scopes the frontend requests, default `openid profile groups`), `planner_groups` / `admin_groups` (space separated OIDC group names, see Authorization), `server_port`, ...) and `netbox:` (`url`, `token`, `provider_id`, `type_id`) sections; each value can be overridden by env vars with prefix `APP` and `__` separator (see `backend/src/config.rs`).
- `LOG_LEVEL` env var controls `env_logger`.
- Helm chart (`cable-editor-chart`): the Netbox token comes from an existing Secret (`config.netboxTokenSecret.name`/`key`) or from `config.netboxToken`, which the chart stores in a Secret of its own; either way the Deployment only references it. Rendering fails without one.

## Architecture

### GraphQL: two schemas, typed end-to-end with cynic
- Backend exposes `/graphql` (authenticated: `graphql/authenticated/` — `Query`, `mutation/`, `planned.rs`) and `/graphql_anonymous` (`graphql/anonymous.rs`).
- `cable-editor-frontend/build.rs` depends on the backend crate, calls `create_authenticated_schema()` / `create_anonymous_schema()`, writes the SDL to `cable-editor-frontend/graphql/*.graphql` (gitignored), and registers them with `cynic_codegen`. Frontend queries are `cynic::QueryFragment` structs in `frontend/src/graphql/{authenticated,anonymous}/*.rs`, so **any backend schema change is checked at frontend compile time** — change the backend resolver, then fix the frontend fragments.
- `cable-editor-backend/build.rs` registers `schema/schema.graphql` (the NetBox GraphQL schema) for the cynic-based NetBox client in `src/netbox/`.

### Per-request transaction
In `binary/src/main.rs`, each authenticated GraphQL request: validates the JWT (`actix-4-jwt-auth`), fetches OIDC userinfo (cached 30s) into `UserInfo`, opens a pooled connection, runs `BEGIN`, and puts it into the request data as `Arc<tokio::Mutex<Object<AsyncPgConnection>>>`. Resolvers obtain it via `graphql::authenticated::get_connection(ctx)`. The whole request is `COMMIT`ed if there are no errors, otherwise `ROLLBACK`ed — so resolvers should return errors rather than manage transactions themselves (nested `connection.transaction(...)` becomes a savepoint, e.g. `mutation/implement.rs`).

The schema references other objects as objects, never as bare ids (`Duct.schachtA: Schacht!`, not `schachtAId`). To keep that from costing a query per object, such references go through `graphql/loader.rs`: an async-graphql `DataLoader` (created per request in `main.rs`) on the same shared connection, so it sees the transaction, and batches the keys of one request into one query per kind (`load_one(ctx, SchachtId(id))`). A resolver must not hold `get_connection`'s guard while awaiting the loader, that deadlocks — load its own rows first, drop the guard, then load the references (see `Schacht.connectingDuct`).

### Authorization
Tokens must carry the client id in `aud` (Pocket ID does so by default, the local Keycloak realm has an audience mapper). Every logged in user may read (restrict logins at the provider, e.g. Pocket ID's allowed user groups). `graphql/authorization.rs` maps the `groups` from the userinfo to a `Role` (`Reader` < `Planner` < `Admin`) via the config's `planner_groups` / `admin_groups` (group names, in Pocket ID not the friendly names). **Every mutation needs a `#[graphql(guard = "RoleGuard(Role::…)")]`**: Planner for plans and the master data (cables, panels, ports), Admin for `implementPlan`, `syncPlanToNetbox` and `deleteCable`. `currentUser.role` tells the frontend what to hide: `components/user.rs` `UserProvider` provides the `Role` as context (`util::get_role`), `AppRoute::required_role` names the pages needing more than `Reader` (the panel editors), `RequireRole` guards them and `MenuDropdown` leaves them out; pages hide or disable their change buttons themselves (the cable page is read-only for readers). `UserMenu` at the end of the breadcrumb bar shows the user, the role and the provider's groups for support. The mock backend takes the role from `MOCK_ROLE` (`DENIED` refuses the login). `pages/mod.rs` `Login` starts the login only without a session; a failed login (e.g. the provider refusing the user's groups) shows an alert with a retry button, as starting the next login right away would redirect forever. Local users (password = name): `admin`, `planner`, `reader`.

### Plans
Plan 0 is the baseline (`BASELINE_PLAN_ID` in backend and frontend, `Plan.isBaseline` in GraphQL): its `port_usage` rows are the current state. Every other plan holds planned changes on top of it; implementing merges them into the baseline and deletes the plan, so there is no plan status (SQL uses the literal 0). The baseline can't be changed directly, renamed or implemented. `planned.rs` exposes `PlannedPanel` / `PlannedPort` (entity + plan context); `port_usage` rows are plan-scoped. `mutation/implement.rs` applies a plan; `mutation/sync.rs` diffs a plan against NetBox devices/rear ports and reports `SyncIssue`s before syncing.

### Database
Diesel 2 with `diesel-async` + deadpool; PostGIS geometries via `postgis_diesel`. Geometries are stored in LV95 (EPSG:2056, `schacht.geom`, `trasse.geom`; `local/data.sql` converts its WGS84 sample data on import). For the map GraphQL delivers them as WGS84 `GeoPoint { lat lng }` (`Schacht.location`, `Duct.line` from Schacht A to Z, `Cable.line`, missing if its ducts have a gap), converted by PostGIS with `st_transform(geom, WGS84)`; `Schacht.position` is the raw LV95 point. Table definitions are in `backend/src/db/schema.rs`, entities in `backend/src/db/entity/`. Note `diesel.toml` points `print_schema` at `src/schema.rs`, but the live file is `src/db/schema.rs` — move/merge the output if you regenerate with `diesel migration run`.

### Frontend
`pages/router.rs` defines `AppRoute` (yew-nested-router); plan-scoped views are nested under `AppRoute::Plan { plan_id, view }`. `AppRoute::content` builds PatternFly's page layout by hand (no `Page` component, no masthead/sidebar): `<main>` with a breadcrumb `PageSection` (menus in `components/menu/`) followed by the page. The breadcrumb is one hierarchy Plan › area › Schacht › panels › view: the menu of each object (`ListCabinet`, `ListPanel`) holds its views plus the objects at the same level, the last item the current view plus the objects one level down (a Schacht's root panels, a panel's children). Menus use the own popper-free `PopupMenu` (`components/menu/popup.rs`, see its doc comment) and value choices a native `FormSelect`; patternfly-yew's popper-based `Dropdown`/`SimpleSelect` log console errors and are not used. Every routed page wraps its output in `components/page_layout.rs` `PageLayout` (h1 title section + content section; title `"<view> – <object>"` via `object_title`, just the view while loading), so headings inside a page start at `h2`; `.app-page` in `style.scss` lets the document scroll instead of the main container, so pages must not add their own outer padding. Pages live in `pages/`, reusable pieces in `components/`. PatternFly's dark theme (`pf-v6-theme-dark` on `<html>`) follows `prefers-color-scheme` through a script in `index.html`, off while printing; styles use PF tokens (`--pf-t--global--*`), fixed colours only for the fibre colours and the print styles. Wherever a page (other than an edit page) shows a Schacht, panel or cable, it links to it with `components/links.rs` (`SchachtLink` → overview, `PanelLink` → connection overview, `CableLink` → cable page). Complex components (data fetching, state, event handling) are struct components (`impl Component`: `create` sends a fetch message, `spawn_local` + `util::get_credentials`, `changed` refetches on new ids); function components only for very simple ones. Hooks have struct counterparts, e.g. `label_printer::check_printer_supported` for `use_printer_supported`, `util::get_backdrop`, and `components/table.rs` `ListModel` instead of `use_table_data`. `PlanView::Cabinet { view: CabinetView::Overview }` (`pages/cabinet/overview.rs`) is the Schacht overview, built for phones in the field: its panels (links to their connection overview, panel labels) and the cables ending there (cable labels). Editing its panels is a separate page, `CabinetView::Edit` (`pages/cabinet/edit.rs`); the breadcrumb switches between the two. On phones, tree tables only show cells with a column label, and `PopupMenu`s open as a sheet at the bottom. `PlanView::Map` (`pages/map.rs`) is the plan's map, for now all Schächte with a position (`Schacht.location`), as markers labelled with their name on swisstopo's national map (WMTS, EPSG:3857), from zoom 17 on the cadastral map (WMS); a click on a marker or its name opens the Schacht overview. It drives Leaflet through the `leaflet` crate directly; the unrouted `pages/map_test.rs`, `components/map.rs` and `map_edit/` are older prototypes. The crate's `bind_tooltip_with_content` calls a method Leaflet doesn't have, bind a `Tooltip` instead; SVG colours come from CSS classes (`.map-view__schacht`), as Leaflet's colour options are attributes that can't take tokens. Auth uses `yew-oauth2` (OpenID); `graphql::query`/`mutate` attach the bearer token from `OAuth2Context` and return the data, GraphQL errors as `FrontendError::Graphql` (shown as a danger alert) and a response without data as `FrontendError::NotFound`. The GraphQL URL is derived from `window.location`, which is why `trunk serve` proxies to the backend.

Errors belong in the UI, never only in the console: return them as `FrontendError` and show them as an alert on the page (`IntoPropValue<Html>` for `&FrontendError`), or, where replacing the page would lose unsaved input (e.g. a failed request behind a button), as a danger toast via `util::get_toaster`. Console logging (`log::warn!`) is only for conditions that aren't errors, e.g. the label font falling back. No panics in reachable code (a panic takes down the whole wasm app): no `unwrap`/`expect`/`assert!` on data; `expect` only for programming errors such as a missing Yew context.

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
