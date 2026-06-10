set dotenv-load

# Dev — run each in a separate terminal
dev-backend:
    cargo run -p backend

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

# Deploy (CI handles this; use for manual deploys)
deploy-hub: build-hub
    firebase deploy --only hosting:hub

deploy-movienight: build-movienight
    firebase deploy --only hosting:movienight
