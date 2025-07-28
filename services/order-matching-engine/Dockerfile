FROM rust:latest as builder

WORKDIR /app

# Copy core library
COPY libs/core ./libs/core

# Copy service source
COPY services/order-matching-engine ./services/order-matching-engine

# Build the application
WORKDIR /app/services/order-matching-engine
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/services/order-matching-engine/target/release/order-matching-engine /usr/local/bin/order-matching-engine

# Create non-root user
RUN useradd -r -s /bin/false order-matching-engine
USER order-matching-engine

EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["order-matching-engine"]