#!/bin/sh
# Frontend against a mock backend (server.mjs), without Postgres, Keycloak or the Rust backend:
# builds the frontend (which also generates the GraphQL schemas) and serves it on
# http://localhost:8099. Screenshots: `node local/mock/screenshot.mjs /plan/0/cabinet/1/overview`
# (needs a Playwright Chromium: `npx playwright install chromium`). MOCK_ROLE=READER, PLANNER or
# ADMIN (default) sets the role of the mock user.
set -e
cd "$(dirname "$0")"
npm install --no-fund --no-audit
(cd ../../cable-editor-frontend && trunk build)
exec node server.mjs
