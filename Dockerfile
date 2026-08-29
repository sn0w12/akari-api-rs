# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        python3 \
        curl \
        libssl-dev \
        libfontconfig1-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/akari

# Copy manifests first to leverage layer caching for dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY build.rs ./build.rs
COPY assets ./assets
COPY src ./src
# Test/bench/example sources must exist for the manifest to resolve
# (even though they aren't compiled into the release binary).
COPY benches ./benches
COPY examples ./examples
COPY tests ./tests

RUN cargo build --release -p akari-api-rs --bin akari-api-rs

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root user
RUN useradd --create-home --uid 10001 app
USER app
WORKDIR /home/app

COPY --from=builder /usr/src/akari/target/release/akari-api-rs /usr/local/bin/akari-api-rs

# Defaults; all real configuration arrives via environment variables / CLI flags
ENV HOST=0.0.0.0 \
    PORT=3000

EXPOSE 3000

CMD ["akari-api-rs"]