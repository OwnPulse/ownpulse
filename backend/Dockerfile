# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) OwnPulse Contributors

# NOTE: Build context must be the repository root (not backend/).
# CI uses: docker build -f backend/Dockerfile .

# Stage 1: Build
FROM rust:1.94-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/api/Cargo.toml api/Cargo.toml

# Create dummy main to cache dependency build
RUN mkdir -p api/src && echo "fn main() {}" > api/src/main.rs
RUN cargo build --release --locked && rm -rf api/src

# Copy real source and migration SQL files (needed for include_str! at compile time)
COPY backend/api/ api/
COPY db/migrations/ /db/migrations/
RUN touch api/src/main.rs && cargo build --release --locked

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1000 app && useradd --uid 1000 --gid app --shell /bin/false app

COPY --from=builder /app/target/release/api /usr/local/bin/api

USER app
EXPOSE 8080
ENTRYPOINT ["api"]
