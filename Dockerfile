# syntax=docker/dockerfile:1

# 1) Builder image
FROM rust:1-bookworm AS builder
WORKDIR /app

# Pre-copy manifest files for better caching
COPY Cargo.toml Cargo.lock ./
# Copy source
COPY src ./src

# Build only the server binary in release mode
RUN cargo build --release --bin server

# 2) Runtime image (small, glibc-based)
FROM debian:bookworm-slim AS runtime
# Install CA certs in case your app or deps need TLS
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/server /app/server

# The server listens on TCP 4000
EXPOSE 4000

# Provide a default runtime command (override with `docker run ... /app/server --flags` if needed)
ENTRYPOINT ["/app/server"]