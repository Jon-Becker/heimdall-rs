# syntax=docker/dockerfile:1

FROM rust:1.91-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

# Compute a dependency recipe that only changes when the dependency graph does,
# so source-only edits reuse the cached dependency build below.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
# Build and cache dependencies first. Release optimization flags (fat LTO,
# codegen-units = 1) are defined once in Cargo.toml's [profile.release].
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin heimdall

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/heimdall /usr/local/bin/heimdall

ENTRYPOINT ["heimdall"]
