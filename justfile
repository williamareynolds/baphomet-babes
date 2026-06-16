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

dev-hub:
    cd hub && trunk serve

# Build (stamps the git SHA into the bundle + dist/version.json for update checks)
build-hub:
    cd hub && BUILD_SHA=$(git rev-parse HEAD) trunk build --release && echo "{\"version\":\"$(git rev-parse HEAD)\"}" > dist/version.json

# Check (no WASM toolchain needed for type check)
check-backend:
    cargo check -p backend

check-hub:
    cargo check -p hub --target wasm32-unknown-unknown

check: check-backend check-hub

# Test — unit, property, and golden suites (integration self-skips without emulator).
# hub is WASM-only (excluded); it gets type-checked by `just check`.
test:
    cargo test --workspace --exclude hub

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

# Firestore security rules — default-deny (backend SA bypasses; clients blocked).
deploy-firestore-rules:
    firebase deploy --only firestore:rules --project baphomet-babes

# Billing kill-switch — hard $30/mo spend cap (disables billing on overrun).
# Idempotent; re-run after editing infra/billing-killswitch/main.py to redeploy.
setup-killswitch:
    infra/billing-killswitch/setup.sh

# Deploy (CI handles this; use for manual deploys)
deploy-hub: build-hub
    firebase deploy --only hosting:hub
