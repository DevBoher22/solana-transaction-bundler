# Multi-stage build for optimized production image
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build dependencies (this layer will be cached)
RUN cargo build --release --bin bundler-cli --bin bundler-service

# Production stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r bundler && useradd -r -g bundler bundler

# Create app directory
WORKDIR /app

# Copy binaries from builder stage
COPY --from=builder /app/target/release/bundler-cli /usr/local/bin/bundler-cli
COPY --from=builder /app/target/release/bundler-service /usr/local/bin/bundler-service

# Copy configuration examples
COPY examples/bundler.config.toml /app/bundler.config.toml.example
COPY examples/bundle_request.json /app/bundle_request.json.example

# Create directories for logs and data
RUN mkdir -p /app/logs /app/data && \
    chown -R bundler:bundler /app

# Switch to non-root user
USER bundler

# Create default config if none exists
RUN cp /app/bundler.config.toml.example /app/bundler.config.toml.default

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:8080/v1/health || exit 1

# Expose ports
EXPOSE 8080 9090

# Set environment variables
ENV RUST_LOG=info

# Default command with config fallback
CMD ["/bin/bash", "-c", "if [ ! -f /app/bundler.config.toml ]; then cp /app/bundler.config.toml.default /app/bundler.config.toml; fi && bundler-service --config /app/bundler.config.toml"]

# Labels for metadata
LABEL org.opencontainers.image.title="Solana Transaction Bundler"
LABEL org.opencontainers.image.description="Production-ready Solana transaction bundler with low latency and high success rate"
LABEL org.opencontainers.image.version="0.1.0"
LABEL org.opencontainers.image.authors="DevBoher22"
LABEL org.opencontainers.image.source="https://github.com/DevBoher22/solana-transaction-bundler"
LABEL org.opencontainers.image.licenses="MIT"
