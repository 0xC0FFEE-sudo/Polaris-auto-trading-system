#!/bin/bash

# Polaris Synapse Build Script
# Tony Stark's Crypto Trading System

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="polaris-synapse"
VERSION=${VERSION:-"latest"}
REGISTRY=${REGISTRY:-"localhost:5000"}
BUILD_PARALLEL=${BUILD_PARALLEL:-"true"}

echo -e "${BLUE}🚀 Building Polaris Synapse - Tony Stark's Crypto Trading System${NC}"
echo -e "${BLUE}Version: ${VERSION}${NC}"
echo -e "${BLUE}Registry: ${REGISTRY}${NC}"
echo ""

# Function to print status
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to build Rust service
build_rust_service() {
    local service_name=$1
    local service_path=$2
    
    print_status "Building Rust service: ${service_name}"
    
    cd "${service_path}"
    
    # Check if Dockerfile exists
    if [ ! -f "Dockerfile" ]; then
        print_warning "No Dockerfile found for ${service_name}, creating one..."
        cat > Dockerfile << EOF
FROM rust:1.75 as builder

WORKDIR /app

# Copy core library
COPY ../../libs/core ./libs/core

# Copy service source
COPY . ./services/${service_name}

# Build the application
WORKDIR /app/services/${service_name}
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \\
    ca-certificates \\
    libssl3 \\
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/services/${service_name}/target/release/${service_name} /usr/local/bin/${service_name}

# Create non-root user
RUN useradd -r -s /bin/false ${service_name}
USER ${service_name}

EXPOSE 8080

CMD ["${service_name}"]
EOF
    fi
    
    # Build Docker image
    docker build -t "${REGISTRY}/${PROJECT_NAME}/${service_name}:${VERSION}" \
                 -f Dockerfile \
                 --build-arg SERVICE_NAME="${service_name}" \
                 ../../
    
    print_status "✅ Built ${service_name}"
    cd - > /dev/null
}

# Function to build Python service
build_python_service() {
    local service_name=$1
    local service_path=$2
    
    print_status "Building Python service: ${service_name}"
    
    cd "${service_path}"
    
    # Check if Dockerfile exists
    if [ ! -f "Dockerfile" ]; then
        print_warning "No Dockerfile found for ${service_name}, creating one..."
        cat > Dockerfile << EOF
FROM python:3.12-slim

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \\
    gcc \\
    g++ \\
    && rm -rf /var/lib/apt/lists/*

# Copy requirements and install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY . .

# Create non-root user
RUN useradd -r -s /bin/false ${service_name}
RUN chown -R ${service_name}:${service_name} /app
USER ${service_name}

EXPOSE 8000

CMD ["python", "main.py"]
EOF
    fi
    
    # Build Docker image
    docker build -t "${REGISTRY}/${PROJECT_NAME}/${service_name}:${VERSION}" .
    
    print_status "✅ Built ${service_name}"
    cd - > /dev/null
}

# Function to build Go service
build_go_service() {
    local service_name=$1
    local service_path=$2
    
    print_status "Building Go service: ${service_name}"
    
    cd "${service_path}"
    
    # Check if Dockerfile exists
    if [ ! -f "Dockerfile" ]; then
        print_warning "No Dockerfile found for ${service_name}, creating one..."
        cat > Dockerfile << EOF
FROM golang:1.21-alpine AS builder

WORKDIR /app

# Copy go mod files
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -o ${service_name} .

FROM alpine:latest

# Install ca-certificates for HTTPS
RUN apk --no-cache add ca-certificates

WORKDIR /root/

# Copy the binary
COPY --from=builder /app/${service_name} .

EXPOSE 8080

CMD ["./${service_name}"]
EOF
    fi
    
    # Build Docker image
    docker build -t "${REGISTRY}/${PROJECT_NAME}/${service_name}:${VERSION}" .
    
    print_status "✅ Built ${service_name}"
    cd - > /dev/null
}

# Main build function
main() {
    print_status "Starting build process..."
    
    # Check if Docker is running
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
    
    # Create build directory
    mkdir -p build/logs
    
    # Build core library first
    print_status "Building core library..."
    cd libs/core
    cargo build --release
    cd - > /dev/null
    
    # Define services to build
    declare -A rust_services=(
        ["market-data-handler"]="services/market-data-handler"
        ["order-gateway"]="services/order-gateway"
        ["risk-manager"]="services/risk-manager"
        ["order-matching-engine"]="services/order-matching-engine"
        ["execution-engine"]="services/execution-engine"
        ["compliance-gateway"]="services/compliance-gateway"
    )
    
    declare -A python_services=(
        ["llm-agents"]="services/llm-agents"
    )
    
    declare -A go_services=(
        ["gateway"]="services/gateway"
        ["firehose-bridge"]="services/firehose_bridge"
    )
    
    # Build services
    if [ "${BUILD_PARALLEL}" = "true" ]; then
        print_status "Building services in parallel..."
        
        # Build Rust services in parallel
        for service in "${!rust_services[@]}"; do
            build_rust_service "$service" "${rust_services[$service]}" &
        done
        
        # Build Python services in parallel
        for service in "${!python_services[@]}"; do
            build_python_service "$service" "${python_services[$service]}" &
        done
        
        # Build Go services in parallel
        for service in "${!go_services[@]}"; do
            build_go_service "$service" "${go_services[$service]}" &
        done
        
        # Wait for all builds to complete
        wait
        
    else
        print_status "Building services sequentially..."
        
        # Build Rust services
        for service in "${!rust_services[@]}"; do
            build_rust_service "$service" "${rust_services[$service]}"
        done
        
        # Build Python services
        for service in "${!python_services[@]}"; do
            build_python_service "$service" "${python_services[$service]}"
        done
        
        # Build Go services
        for service in "${!go_services[@]}"; do
            build_go_service "$service" "${go_services[$service]}"
        done
    fi
    
    print_status "All services built successfully! 🎉"
    
    # List built images
    print_status "Built images:"
    docker images | grep "${REGISTRY}/${PROJECT_NAME}" | head -20
    
    # Generate docker-compose override for built images
    print_status "Generating docker-compose.override.yml..."
    cat > docker-compose.override.yml << EOF
version: '3.8'

# Override file with built images
services:
EOF
    
    for service in "${!rust_services[@]}" "${!python_services[@]}" "${!go_services[@]}"; do
        cat >> docker-compose.override.yml << EOF
  ${service}:
    image: ${REGISTRY}/${PROJECT_NAME}/${service}:${VERSION}
EOF
    done
    
    print_status "Build complete! You can now run:"
    echo -e "${BLUE}  docker-compose up -d${NC}"
    echo -e "${BLUE}  # or for production:${NC}"
    echo -e "${BLUE}  docker-compose -f docker-compose.yml -f deployments/docker-compose.prod.yml up -d${NC}"
}

# Handle script arguments
case "${1:-}" in
    "clean")
        print_status "Cleaning build artifacts..."
        docker system prune -f
        docker images | grep "${REGISTRY}/${PROJECT_NAME}" | awk '{print $3}' | xargs -r docker rmi -f
        rm -rf build/
        rm -f docker-compose.override.yml
        print_status "Clean complete!"
        ;;
    "push")
        print_status "Pushing images to registry..."
        docker images | grep "${REGISTRY}/${PROJECT_NAME}" | awk '{print $1":"$2}' | xargs -I {} docker push {}
        print_status "Push complete!"
        ;;
    "test")
        print_status "Running tests..."
        # Run Rust tests
        cd libs/core && cargo test && cd - > /dev/null
        for service in "${!rust_services[@]}"; do
            cd "${rust_services[$service]}" && cargo test && cd - > /dev/null
        done
        print_status "Tests complete!"
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [clean|push|test]"
        echo "  clean - Clean build artifacts and images"
        echo "  push  - Push built images to registry"
        echo "  test  - Run tests"
        echo "  (no args) - Build all services"
        exit 1
        ;;
esac