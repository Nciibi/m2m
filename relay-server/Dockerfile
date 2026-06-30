# ── Stage 1: Build ─────────────────────────────────────────────────────────
FROM rust:1-slim AS builder
WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock* ./
COPY src-tauri/Cargo.toml ./src-tauri/
COPY relay-server/Cargo.toml ./relay-server/
RUN mkdir -p src-tauri/src relay-server/src && \
    echo "fn main() {}" > src-tauri/src/lib.rs && \
    echo "fn main() {}" > relay-server/src/main.rs && \
    cargo build --release -p m2m-relay 2>/dev/null || true

# Now copy real sources and rebuild
COPY . .
RUN cargo build --release -p m2m-relay

# ── Stage 2: Runtime ───────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/m2m-relay /usr/local/bin/m2m-relay

EXPOSE 3478

ENTRYPOINT ["m2m-relay"]
