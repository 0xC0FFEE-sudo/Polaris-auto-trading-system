FROM golang:1.21-alpine AS builder

WORKDIR /app

# Copy go mod files
COPY services/firehose_bridge/go.mod services/firehose_bridge/go.sum ./
RUN go mod download

# Copy source code
COPY services/firehose_bridge/ .

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -o firehose_bridge .

FROM alpine:latest

# Install ca-certificates for HTTPS
RUN apk --no-cache add ca-certificates

WORKDIR /root/

# Copy the binary
COPY --from=builder /app/firehose_bridge .

CMD ["./firehose_bridge"]