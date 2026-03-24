FROM rust:1.82-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    afl++ \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/hivefuzz /usr/local/bin/hivefuzz

ENV RUST_LOG=info

ENTRYPOINT ["hivefuzz"]
