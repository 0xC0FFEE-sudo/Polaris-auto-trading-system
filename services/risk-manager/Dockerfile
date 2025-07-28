FROM rust:latest as builder

WORKDIR /app

# Copy core library
COPY libs/core ./libs/core

# Copy service source
COPY services/risk-manager ./services/risk-manager

# Build the application
WORKDIR /app/services/risk-manager
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/services/risk-manager/target/release/risk-manager /usr/local/bin/risk-manager

# Create non-root user
RUN useradd -r -s /bin/false risk-manager
USER risk-manager

EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["risk-manager"]