# Pin to bookworm so the binary's glibc matches the bookworm-slim runtime below
# (plain rust:slim now tracks trixie/glibc 2.38, which the runtime lacks).
FROM rust:1.96-slim-bookworm AS builder
# cmake is required to build aws-lc-rs (pulled in via reqwest -> hyper-rustls);
# pkg-config/libssl-dev for the existing native deps.
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release -p backend

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
EXPOSE 8080
CMD ["backend"]
