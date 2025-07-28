FROM golang:1.21-alpine AS builder

WORKDIR /app

# Copy go mod files
COPY services/gateway/go.mod services/gateway/go.sum ./
RUN go mod download

# Copy source code
COPY services/gateway/ .

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -o gateway .

FROM alpine:latest

# Install ca-certificates for HTTPS
RUN apk --no-cache add ca-certificates curl

WORKDIR /root/

# Copy the binary
COPY --from=builder /app/gateway .

EXPOSE 7000
EXPOSE 7001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:7000/health || exit 1

CMD ["./gateway"]