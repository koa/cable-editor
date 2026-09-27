#!/bin/sh
# The real backend against a throwaway PostgreSQL/PostGIS from nix, without podman or Keycloak:
# - PostgreSQL 16 with PostGIS (nix), fresh on every start, every statement logged,
# - the login of the mock backend (local/mock/server.mjs, only its OIDC endpoints are used),
# - a static userinfo with the groups of REALDB_ROLE (ADMIN (default), PLANNER or READER),
# - the backend (applies the migrations itself), then local/data.sql.
# The app runs on http://localhost:8080 (the embedded frontend, login without a form), checks of
# the API: `node local/realdb/check.mjs` (the command with the log path is printed). Ctrl-C stops
# everything and removes the database. SKIP_BUILD=1 uses the existing builds.
set -e
cd "$(dirname "$0")/../.."
ROOT=$(pwd)
WORK=$(mktemp -d)
PG_PORT=55432
USERINFO_PORT=8098
ROLE=${REALDB_ROLE:-ADMIN}

cleanup() {
  trap - EXIT INT TERM
  set +e # a process that already ended must not stop the cleanup
  [ -n "$BACKEND" ] && kill "$BACKEND" 2>/dev/null
  [ -n "$MOCK" ] && kill "$MOCK" 2>/dev/null
  [ -n "$USERINFO" ] && kill "$USERINFO" 2>/dev/null
  [ -x "$WORK/pg/bin/pg_ctl" ] && "$WORK/pg/bin/pg_ctl" -D "$WORK/data" -m fast stop >/dev/null 2>&1
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

if [ -z "$SKIP_BUILD" ]; then
  (cd local/mock && npm install --no-fund --no-audit)
  (cd cable-editor-frontend && trunk build)
  cargo build
fi

echo "PostgreSQL mit PostGIS aus nix ..."
nix build --impure -o "$WORK/pg" --expr \
  'let pkgs = import (builtins.getFlake "nixpkgs") {}; in pkgs.postgresql_16.withPackages (p: [ p.postgis ])'
PATH="$WORK/pg/bin:$PATH"
initdb -D "$WORK/data" -U postgres -A trust >/dev/null
# TCP only: the socket path in a temporary directory can be longer than the 107 bytes allowed
pg_ctl -D "$WORK/data" -l "$WORK/pg.log" -w start \
  -o "-k '' -p $PG_PORT -c listen_addresses=127.0.0.1 -c log_statement=all" >/dev/null
createdb -h 127.0.0.1 -p $PG_PORT -U postgres cable
psql -h 127.0.0.1 -p $PG_PORT -U postgres -d cable -qc 'create extension postgis'

case "$ROLE" in
  ADMIN) GROUPS='["admins"]' ;;
  PLANNER) GROUPS='["planners"]' ;;
  *) GROUPS='[]' ;;
esac
mkdir "$WORK/userinfo"
echo "{\"sub\":\"user-1\",\"preferred_username\":\"tester\",\"display_name\":\"Tester ($ROLE)\",\"groups\":$GROUPS}" \
  >"$WORK/userinfo/userinfo"
python3 -m http.server $USERINFO_PORT --bind 127.0.0.1 --directory "$WORK/userinfo" >"$WORK/userinfo.log" 2>&1 &
USERINFO=$!
node local/mock/server.mjs >"$WORK/mock.log" 2>&1 &
MOCK=$!

cat >"$WORK/config.yaml" <<EOF
oauth:
  auth_client_id: "cable-editor"
  auth_issuer: "http://localhost:8099/realms/cable"
  user_info_url: "http://127.0.0.1:$USERINFO_PORT/userinfo"
  planner_groups: "planners"
  admin_groups: "admins"
netbox:
  url: http://127.0.0.1:1/
  token: none
  provider_id: 1
  type_id: 1
EOF
# The backend reads the issuer's discovery at startup, so the mock must be up
until curl -sf http://localhost:8099/realms/cable/.well-known/openid-configuration >/dev/null; do sleep 0.2; done
(cd "$WORK" && DATABASE_URL="postgres://postgres@127.0.0.1:$PG_PORT/cable" LOG_LEVEL=${LOG_LEVEL:-info} \
  exec "$ROOT/target/debug/cable-editor-binary" >"$WORK/backend.log" 2>&1) &
BACKEND=$!
until curl -sf http://127.0.0.1:9080/ready >/dev/null; do
  kill -0 $BACKEND 2>/dev/null || { cat "$WORK/backend.log"; exit 1; }
  sleep 0.5
done
psql -h 127.0.0.1 -p $PG_PORT -U postgres -d cable -q -f local/data.sql >/dev/null

cat <<EOF

Backend mit echter Datenbank läuft (Rolle $ROLE):
  App:       http://localhost:8080
  Datenbank: psql -h 127.0.0.1 -p $PG_PORT -U postgres cable
  Prüfung:   PG_LOG=$WORK/pg.log node local/realdb/check.mjs
  Logs:      $WORK/{backend,pg,mock}.log
Ctrl-C beendet alles und löscht die Datenbank.
EOF
wait $BACKEND
