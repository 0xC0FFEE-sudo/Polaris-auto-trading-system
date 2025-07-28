FROM rust:latest as builder

WORKDIR /app

# Copy core library
COPY libs/core ./libs/core

# Copy compliance gateway source
COPY services/compliance-gateway ./services/compliance-gateway

# Build the application
WORKDIR /app/services/compliance-gateway
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/services/compliance-gateway/target/release/compliance-gateway /usr/local/bin/compliance-gateway

# Create non-root user
RUN useradd -r -s /bin/false compliance
USER compliance

EXPOSE 8080

CMD ["compliance-gateway"]