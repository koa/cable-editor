cd "$(dirname "$0")"

cat data.sql | podman exec -i cable-db psql -U postgres -d cable
