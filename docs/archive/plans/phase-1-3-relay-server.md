# Phase 1.3 — Package Relay Server for Self-Hosting

## Goal
Package the existing TCP relay server (currently at `src-tauri/examples/relay-server.rs`) into a proper standalone binary with Docker support and documentation, so users can self-host with one command.

## What exists
- **`src-tauri/examples/relay-server.rs`** — fully functional ~400-line TCP relay server
- **`src-tauri/src/relay.rs`** — relay client (already integrated, tested)
- **`src-tauri/Cargo.toml`** has `[[example]] name = "relay-server"` — compiles but not `cargo install`-friendly

## Plan

### 1. Extract to workspace member `relay-server/`

Move `examples/relay-server.rs` into its own Cargo workspace member so users can:
- `cargo install --path relay-server`
- `cargo build -p m2m-relay`

**New files:**
- `relay-server/Cargo.toml` — standalone binary, deps: tokio, tracing, hex
- `relay-server/src/main.rs` — the relay server (extracted from examples)
- `Cargo.toml` (root) — add `relay-server` as a workspace member

No code changes to the relay logic itself — just trim dead code that depended on m2m_lib types.

### 2. Dockerfile

`relay-server/Dockerfile` — multi-stage:
```dockerfile
FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p m2m-relay

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/m2m-relay /usr/local/bin/
EXPOSE 3478
ENTRYPOINT ["m2m-relay"]
```

### 3. docker-compose.yml

`docker-compose.yml` (project root):
```yaml
version: "3.9"
services:
  relay:
    build: ./relay-server
    ports:
      - "3478:3478"
    environment:
      - RELAY_PORT=3478
      - RELAY_AUTH_TOKEN=${RELAY_AUTH_TOKEN:-}
    restart: unless-stopped
```

### 4. Documentation

Update `README.md` with a "Run Your Own Relay" section — copy-paste commands:
```sh
# Quick deploy
docker compose up -d

# With auth
RELAY_AUTH_TOKEN=mysecret docker compose up -d
```

### 5. CI / Build integration (optional)

Add a `Makefile` at the relay-server level:
```makefile
build:
    cd .. && cargo build --release -p m2m-relay
    cp ../target/release/m2m-relay .

image:
    docker build -t m2m-relay .
```

## What we DON'T change

- ❌ No changes to relay client (`src-tauri/src/relay.rs`) — already works
- ❌ No changes to relay protocol — already stable  
- ❌ No DHT relay discovery in this pass — future work
- ❌ No auth mechanism changes — existing `RELAY_AUTH_TOKEN` env var is sufficient

## Files to create

| File | Purpose |
|---|---|
| `relay-server/Cargo.toml` | Workspace member manifest |
| `relay-server/src/main.rs` | Standalone binary (extracted from examples) |
| `relay-server/Dockerfile` | Multi-stage Docker build |
| `relay-server/Makefile` | Convenience build targets |
| `docker-compose.yml` | One-command deploy |
| `.dockerignore` | Exclude node_modules, target, etc. |

## Files to modify

| File | Change |
|---|---|
| `Cargo.toml` (root) | Add `relay-server` workspace member |
| `src-tauri/Cargo.toml` | Remove `[[example]] relay-server` (replaced by workspace member) |
| `README.md` | Add "Run Your Own Relay" deploy section |

## Test plan

1. `cargo build -p m2m-relay` — compiles standalone binary
2. `docker build -t m2m-relay ./relay-server` — Docker image builds
3. `docker compose up -d` — relay starts, logs show "relay server started"
4. Existing relay tests in `src-tauri/src/relay.rs` prove client ↔ server protocol compatibility
