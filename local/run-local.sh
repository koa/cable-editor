cd "$(dirname "$0")"/..

echo DATABASE_URL="postgres://postgres:postgres@localhost:5432/cable" >.env

cargo install --locked trunk


(cd cable-editor-frontend; trunk build)

cargo build

cat <<EOF >config.yaml
oauth:
  auth_client_id: "cable-editor"
  auth_issuer: "http://localhost:8081/realms/cable"
  user_info_url: "http://localhost:8081/realms/cable/protocol/openid-connect/userinfo"
  # The local realm has no groups scope, a client mapper adds groups
  auth_scopes: "openid profile"
netbox:
  url: https://netbox-dev.berg-turbenthal.ch/
  token: nbt_w8u3B27xffvJ.zOpY9y3wehOIR6HAtout8KiK4f7tlSj0CW5UwoeE
  provider_id: 1
  type_id: 1
EOF

podman run --name cable-db \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=cable \
  -p 5432:5432 \
  -d docker.io/postgis/postgis:16-3.4

podman run --name auth-server \
  --replace \
  -e KEYCLOAK_ADMIN=admin \
  -e KEYCLOAK_ADMIN_PASSWORD=admin \
  -v $(pwd)/local/realm.json:/opt/keycloak/data/import/realm.json:z \
  -p 8081:8080 \
  -d quay.io/keycloak/keycloak:latest start-dev --import-realm

(cd cable-editor-frontend; trunk serve)
