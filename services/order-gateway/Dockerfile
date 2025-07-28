FROM rust:latest as builder

WORKDIR /app

# Copy core library
COPY libs/core ./libs/core

# Copy service source
COPY services/order-gateway ./services/order-gateway

# Build the application
WORKDIR /app/services/order-gateway
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/services/order-gateway/target/release/order-gateway /usr/local/bin/order-gateway

# Create non-root user
RUN useradd -r -s /bin/false order-gateway
USER order-gateway

EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["order-gateway"]