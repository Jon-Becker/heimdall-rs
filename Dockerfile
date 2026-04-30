FROM rust:1.91-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY bifrost bifrost

RUN RUSTFLAGS="-C codegen-units=1" CARGO_PROFILE_RELEASE_LTO=true cargo build --release --bin heimdall

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/heimdall /usr/local/bin/heimdall

ENTRYPOINT ["heimdall"]
