#!/bin/sh
# Checks the frontend against docs/frontend-konventionen.md:
# 1. static.mjs: rules on the sources (no build needed),
# 2. pages.mjs: every route in a browser (phone and desktop) against the mock backend, once per
#    role (ROLES, default "ADMIN PLANNER READER"), after a build of the frontend.
# SKIP_BUILD=1 uses the existing dist/, VERBOSE=1 lists the checks that are not yet switched on
# ("ausstehend"). Needs a Playwright Chromium (`npx playwright install chromium` in local/mock).
# Exit code 1 if a switched-on check has hits.
cd "$(dirname "$0")/../.." || exit 1
WORK=$(mktemp -d)
PORT=${MOCK_PORT:-8099}
MOCK=
cleanup() {
  [ -n "$MOCK" ] && kill "$MOCK" 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM
status=0

echo "== Statische Prüfung"
node local/konventionen/static.mjs || status=1

if [ -z "$SKIP_BUILD" ]; then
  echo "== Frontend bauen"
  (cd local/mock && npm install --no-fund --no-audit >/dev/null) || exit 1
  if command -v trunk >/dev/null 2>&1; then
    (cd cable-editor-frontend && trunk build >"$WORK/trunk.log" 2>&1)
  else
    (cd cable-editor-frontend && nix shell nixpkgs#trunk -c trunk build >"$WORK/trunk.log" 2>&1)
  fi || { tail -30 "$WORK/trunk.log"; exit 1; }
fi

for role in ${ROLES:-ADMIN PLANNER READER}; do
  echo "== Seiten als $role"
  MOCK_PORT=$PORT MOCK_ROLE=$role node local/mock/server.mjs >"$WORK/mock.log" 2>&1 &
  MOCK=$!
  until curl -sf "http://localhost:$PORT/listofplans" >/dev/null; do
    kill -0 "$MOCK" 2>/dev/null || { cat "$WORK/mock.log"; exit 1; }
    sleep 0.2
  done
  MOCK_PORT=$PORT MOCK_ROLE=$role node local/konventionen/pages.mjs || status=1
  kill "$MOCK" 2>/dev/null
  wait "$MOCK" 2>/dev/null
  MOCK=
done
exit $status
