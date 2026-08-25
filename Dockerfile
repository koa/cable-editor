# ==============================================================================
# 1. Builder Stage
# ==============================================================================
FROM rust:alpine AS builder

# Alle nötigen Abhängigkeiten installieren (inklusive postgresql-dev für Diesel)
RUN apk add --no-cache musl-dev build-base ca-certificates tar wget nodejs npm postgresql-dev gcompat openssl-dev pkgconfig

# WebAssembly (WASM) Target für das Yew-Frontend hinzufügen
RUN rustup target add wasm32-unknown-unknown

# Trunk CLI installieren
RUN wget -qO- https://github.com/trunk-rs/trunk/releases/latest/download/trunk-x86_64-unknown-linux-musl.tar.gz | tar -xzf- -C /usr/local/bin

WORKDIR /usr/src/app
COPY . .

# ------------------------------------------------------------------------------
# SCHRITT A: Frontend bauen
# ------------------------------------------------------------------------------
# Da Trunk das Frontend isoliert baut, gibt es hier keinen Deadlock mehr.
WORKDIR /usr/src/app/cable-editor-frontend
RUN trunk build --release

# ------------------------------------------------------------------------------
# SCHRITT B: Backend bauen
# ------------------------------------------------------------------------------
# Zurück ins Hauptverzeichnis
WORKDIR /usr/src/app

# .cargo/config.toml anlegen, damit MUSL/Postgres-Flags NUR fürs Backend gelten
RUN mkdir -p .cargo && \
    echo '[target.x86_64-unknown-linux-musl]' > .cargo/config.toml && \
    echo 'rustflags = ["-C", "target-feature=-crt-static", "-l", "pgcommon", "-l", "pgport", "-l", "ssl", "-l", "crypto"]' >> .cargo/config.toml

# Backend kompilieren (inkludiert die von Trunk in dist/ generierten Assets via rust-embed)
RUN cargo build --release --target x86_64-unknown-linux-musl --manifest-path cable-editor-binary/Cargo.toml


# ==============================================================================
# 2. Final Stage
# ==============================================================================
FROM alpine:latest

# Zertifikate für reqwest
RUN apk add --no-cache ca-certificates libpq libgcc

# Das fertige Binary rüberkopieren
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/cable-editor-binary /cable-editor-binary

ENV LOG_LEVEL=info
EXPOSE 8080
EXPOSE 9090

ENTRYPOINT ["/cable-editor-binary"]