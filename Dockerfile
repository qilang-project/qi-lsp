# Multi-stage Dockerfile for Qi Language Server

# Stage 1: Build stage
FROM rust:1.75-slim as builder

# Set environment variables
ENV CARGO_TERM_COLOR=always
ENV RUSTFLAGS="-C target-feature=-crt-static"

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (cache layer)
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src
COPY ../qi ../qi

# Build the application
RUN cargo build --release

# Stage 2: Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false qi-lsp

# Create app directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/qi-lsp /usr/local/bin/

# Change ownership to non-root user
RUN chown qi-lsp:qi-lsp /usr/local/bin/qi-lsp

# Switch to non-root user
USER qi-lsp

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/qi-lsp"]

# Default command
CMD ["--help"]

# Labels
LABEL maintainer="Qi Language Team <team@qi-lang.org>"
LABEL description="Qi Language Server - LSP implementation for Qi programming language"
LABEL version="0.1.0"
LABEL repository="https://github.com/qi-lang/qi-compiler"

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD timeout 5s /usr/local/bin/qi-lsp --version || exit 1