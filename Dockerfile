# Build stage
FROM rust:1.75 AS builder

WORKDIR /usr/src/rustymail

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY tests ./tests

# Build the application
RUN cargo build --release --bin rustymail-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1001 rustymail

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /usr/src/rustymail/target/release/rustymail-server /app/rustymail-server

# Copy frontend build files
COPY --from=builder /usr/src/rustymail/frontend/rustymail-app-main/dist /app/frontend/dist

# Set ownership
RUN chown -R rustymail:rustymail /app

# Switch to non-root user
USER rustymail

# Expose ports
# REST API
EXPOSE 9437
# SSE endpoint
EXPOSE 9438
# Dashboard
EXPOSE 9439

# Environment variables with defaults
ENV RUST_LOG=info
ENV REST_HOST=0.0.0.0
ENV REST_PORT=9437
ENV SSE_HOST=0.0.0.0
ENV SSE_PORT=9438
ENV DASHBOARD_ENABLED=true
ENV DASHBOARD_PORT=9439
ENV DASHBOARD_PATH=/app/frontend/dist
ENV IMAP_ADAPTER=mock

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:${REST_PORT}/health || exit 1

# Run the application
CMD ["./rustymail-server"]