set dotenv-load

# Dev — run each in a separate terminal
dev-backend:
    cargo run -p backend

# Local Firestore emulator (in-memory — data is lost when it stops)
dev-emulator:
    #!/usr/bin/env bash
    export PATH="/usr/local/opt/openjdk/bin:$PATH"
    gcloud emulators firestore start --host-port=127.0.0.1:8789

# Backend against the emulator instead of real Firestore.
# Fresh emulator = empty DB: re-register the superadmin with
# SUPERADMIN_INVITE_CODE from .env, then mint invites as needed.
dev-backend-emulated:
    FIRESTORE_EMULATOR_HOST=127.0.0.1:8789 cargo run -p backend

dev-movienight:
    cd frontend && trunk serve

dev-hub:
    cd hub && trunk serve

# Build
build-hub:
    cd hub && trunk build --release

build-movienight:
    cd frontend && trunk build --release

build-all: build-hub build-movienight

# Check (no WASM toolchain needed for type check)
check-backend:
    cargo check -p backend

check-hub:
    cargo check -p hub --target wasm32-unknown-unknown

check-movienight:
    cargo check -p frontend --target wasm32-unknown-unknown

check: check-backend check-hub check-movienight

# Test — unit, property, and golden suites (integration self-skips without emulator).
# frontend/hub are WASM-only (excluded); they get type-checked by `just check`.
test:
    cargo test --workspace --exclude frontend --exclude hub

# Integration tests against the Firestore emulator.
# Requires: gcloud firestore emulator component + Java 21+ on PATH
# (macOS: brew install openjdk, emulator picks it up via the PATH export below).
test-integration:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="/usr/local/opt/openjdk/bin:$PATH"
    if [ -z "${FIRESTORE_EMULATOR_HOST:-}" ]; then
        gcloud emulators firestore start --host-port=127.0.0.1:8787 --quiet > /tmp/firestore-emulator.log 2>&1 &
        EMULATOR_PID=$!
        trap 'kill $EMULATOR_PID 2>/dev/null || true' EXIT
        for i in $(seq 1 45); do
            grep -q "running" /tmp/firestore-emulator.log 2>/dev/null && break
            sleep 2
        done
        export FIRESTORE_EMULATOR_HOST=127.0.0.1:8787
    fi
    cargo test -p backend --test integration

# End-to-end browser tests (Playwright). Starts emulator + backend + hub itself.
# Node version pinned by e2e/.nvmrc via nvm (node 26 breaks Playwright's TS
# loader). One-time setup:
#   cd e2e && nvm install && npm install && npx playwright install chromium
# Stop any dev backend on :8080 first — the suite refuses to reuse it.
test-e2e:
    #!/usr/bin/env bash
    set -euo pipefail
    export NVM_DIR="$HOME/.nvm"
    source /usr/local/opt/nvm/nvm.sh
    cd e2e
    nvm exec --silent npx playwright test

test-all: test test-integration test-e2e

# Deploy (CI handles this; use for manual deploys)
deploy-hub: build-hub
    firebase deploy --only hosting:hub

deploy-movienight: build-movienight
    firebase deploy --only hosting:movienight
