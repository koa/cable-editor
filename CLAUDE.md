# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Fiber-optic cable / duct / patch-panel planning tool. Single deployable binary: a Rust (actix-web) backend serving a GraphQL API plus an embedded Yew (WASM) SPA. Data lives in PostgreSQL + PostGIS; planned panel/port changes can be synced to NetBox. Domain names are partly German (`kabel` = cable, `schacht` = manhole/cabinet, `trasse` = duct route, `sequenz`); code comments and log messages are mixed German/English.

## Workspace layout

Cargo workspace (`default-members = cable-editor-binary`):

- `cable-editor-backend` — library: Diesel models/migrations, async-graphql schemas, NetBox client, INTERLIS2 export (`export.rs`).
- `cable-editor-binary` — actix-web server (`main.rs`): wires OIDC/JWT auth, DB pool, Prometheus, and embeds `cable-editor-frontend/dist` via `rust-embed` (SPA fallback to `index.html`).
- `cable-editor-frontend` — Yew + PatternFly + Leaflet SPA, built with Trunk for `wasm32-unknown-unknown`. Not built by plain `cargo build` from the root.
- `cable-editor-chart` — Helm chart; CI (`.github/workflows/build-and-publish.yml`) pushes the Docker image and chart to GHCR on pushes to `master`.

## Commands

```sh
# Local dev environment (Postgres/PostGIS + Keycloak via podman, writes .env and config.yaml, then `trunk serve`)
local/run-local.sh
local/import-data.sh              # load local/data.sql into the cable-db container

# Frontend (from cable-editor-frontend/; pre_build hook runs `npm install` in node/)
trunk serve                       # dev server on :8082, proxies /graphql and /graphql_anonymous to :8080
trunk build [--release]           # outputs dist/, which the binary embeds

# Backend server (root) — needs DATABASE_URL (from .env) and config.yaml
cargo run                         # API on :8080, metrics/health on :9080 (port + 1000)
cargo build -p cable-editor-frontend --target wasm32-unknown-unknown   # typecheck frontend
cargo clippy --workspace          # (frontend needs the wasm target to compile cleanly)

# Release container
docker build .
```

There are currently no tests in the repo.

**Build order matters:** the binary embeds `cable-editor-frontend/dist` at compile time, so run `trunk build` before building the binary (the Dockerfile does frontend first, then the musl backend build).

## Configuration

- `DATABASE_URL` env var (`.env`, loaded by dotenvy). Migrations in `cable-editor-backend/migrations` are embedded and run automatically at startup (`run_sync_migrations`).
- `config.yaml` (gitignored) with `oauth:` (`auth_client_id`, `auth_issuer`, optional `user_info_url`, `server_port`, ...) and `netbox:` (`url`, `token`, `provider_id`, `type_id`) sections; each value can be overridden by env vars with prefix `APP` and `__` separator (see `backend/src/config.rs`).
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
`pages/router.rs` defines `AppRoute` (yew-nested-router) and the sidebar; plan-scoped views are nested under `AppRoute::Plan { plan_id, view }`. Pages live in `pages/`, reusable pieces in `components/`. Auth uses `yew-oauth2` (OpenID); `graphql::query`/`mutate` attach the bearer token from `OAuth2Context`. The GraphQL URL is derived from `window.location`, which is why `trunk serve` proxies to the backend.
