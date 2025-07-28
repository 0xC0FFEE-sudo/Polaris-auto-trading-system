FROM rust:latest as builder

WORKDIR /app

# Copy core library
COPY libs/core ./libs/core

# Copy service source
COPY services/market-data-handler ./services/market-data-handler

# Build the application
WORKDIR /app/services/market-data-handler
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/services/market-data-handler/target/release/market-data-handler /usr/local/bin/market-data-handler

# Create non-root user
RUN useradd -r -s /bin/false market-data-handler
USER market-data-handler

EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["market-data-handler"]