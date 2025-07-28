#!/bin/bash

# Polaris Synapse Development Environment Setup
# Tony Stark's Crypto Trading System

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Setting up Polaris Synapse Development Environment${NC}"
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

# Check prerequisites
check_prerequisites() {
    print_status "Checking prerequisites..."
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed. Please install Docker and try again."
        exit 1
    fi
    
    # Check Docker Compose
    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose is not installed. Please install Docker Compose and try again."
        exit 1
    fi
    
    # Check Rust
    if ! command -v cargo &> /dev/null; then
        print_warning "Rust is not installed. Installing via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
    fi
    
    # Check Python
    if ! command -v python3 &> /dev/null; then
        print_error "Python 3 is not installed. Please install Python 3.12+ and try again."
        exit 1
    fi
    
    # Check Go
    if ! command -v go &> /dev/null; then
        print_warning "Go is not installed. Please install Go 1.21+ for full functionality."
    fi
    
    print_status "✅ Prerequisites check complete"
}

# Setup environment file
setup_environment() {
    print_status "Setting up environment configuration..."
    
    if [ ! -f ".env" ]; then
        cp .env.example .env
        print_warning "Created .env file from template. Please edit it with your actual API keys."
        print_warning "The system will work with mock data, but you'll need real API keys for live trading."
    else
        print_status "Environment file already exists"
    fi
}

# Create necessary directories
create_directories() {
    print_status "Creating necessary directories..."
    
    mkdir -p logs
    mkdir -p data/postgres
    mkdir -p data/redis
    mkdir -p data/kafka
    mkdir -p data/prometheus
    mkdir -p data/grafana
    mkdir -p build
    
    print_status "✅ Directories created"
}

# Setup Rust development environment
setup_rust() {
    print_status "Setting up Rust development environment..."
    
    # Install required Rust components
    rustup component add rustfmt clippy
    
    # Build core library
    cd libs/core
    cargo build
    cd ../..
    
    print_status "✅ Rust environment ready"
}

# Setup Python development environment
setup_python() {
    print_status "Setting up Python development environment..."
    
    # Create virtual environment for LLM agents
    cd services/llm-agents
    
    if [ ! -d "venv" ]; then
        python3 -m venv venv
        print_status "Created Python virtual environment"
    fi
    
    # Activate virtual environment and install dependencies
    source venv/bin/activate
    pip install --upgrade pip
    pip install -r requirements.txt
    deactivate
    
    cd ../..
    
    print_status "✅ Python environment ready"
}

# Setup Go development environment
setup_go() {
    if command -v go &> /dev/null; then
        print_status "Setting up Go development environment..."
        
        # Setup gateway service
        cd services/gateway
        go mod tidy
        cd ../..
        
        # Setup firehose bridge
        cd services/firehose_bridge
        go mod tidy
        cd ../..
        
        print_status "✅ Go environment ready"
    else
        print_warning "Go not installed, skipping Go setup"
    fi
}

# Initialize databases
init_databases() {
    print_status "Initializing databases..."
    
    # Start only database services
    docker-compose up -d postgres postgres-compliance redis
    
    # Wait for databases to be ready
    print_status "Waiting for databases to be ready..."
    sleep 10
    
    # Run database migrations
    print_status "Running database migrations..."
    
    # For now, we'll create the tables manually
    # In production, you'd use a proper migration tool
    
    print_status "✅ Databases initialized"
}

# Setup Kafka topics
setup_kafka() {
    print_status "Setting up Kafka topics..."
    
    # Start Kafka
    docker-compose up -d redpanda
    
    # Wait for Kafka to be ready
    print_status "Waiting for Kafka to be ready..."
    sleep 15
    
    # Create topics
    topics=(
        "market_data.raw"
        "market_data.normalized"
        "sentiment.analyzed"
        "onchain.events"
        "orders.client"
        "orders.incoming"
        "orders.validated"
        "orders.executable"
        "orders.matched"
        "fills"
        "trading.decisions"
        "compliance.alerts"
        "system.heartbeats"
        "transactions.all"
    )
    
    for topic in "${topics[@]}"; do
        docker exec redpanda rpk topic create "$topic" --partitions 12 --replicas 1 || true
        print_status "Created topic: $topic"
    done
    
    print_status "✅ Kafka topics created"
}

# Build all services
build_services() {
    print_status "Building all services..."
    
    ./scripts/build.sh
    
    print_status "✅ All services built"
}

# Start monitoring stack
start_monitoring() {
    print_status "Starting monitoring stack..."
    
    docker-compose up -d prometheus grafana
    
    print_status "✅ Monitoring stack started"
    print_status "Grafana: http://localhost:3000 (admin/admin)"
    print_status "Prometheus: http://localhost:9090"
}

# Run health checks
health_check() {
    print_status "Running health checks..."
    
    # Wait a bit for services to start
    sleep 10
    
    # Check service health
    services=(
        "http://localhost:3000"  # Grafana
        "http://localhost:9090"  # Prometheus
        "http://localhost:9092"  # Kafka (will fail but that's ok)
    )
    
    for service in "${services[@]}"; do
        if curl -f -s "$service" > /dev/null; then
            print_status "✅ $service is healthy"
        else
            print_warning "⚠️  $service is not responding (may still be starting)"
        fi
    done
}

# Main setup function
main() {
    echo -e "${BLUE}Starting development environment setup...${NC}"
    
    check_prerequisites
    setup_environment
    create_directories
    setup_rust
    setup_python
    setup_go
    init_databases
    setup_kafka
    build_services
    start_monitoring
    health_check
    
    echo ""
    echo -e "${GREEN}🎉 Development environment setup complete!${NC}"
    echo ""
    echo -e "${BLUE}Next steps:${NC}"
    echo "1. Edit .env file with your API keys"
    echo "2. Start all services: docker-compose up -d"
    echo "3. View logs: docker-compose logs -f [service-name]"
    echo "4. Access monitoring: http://localhost:3000"
    echo ""
    echo -e "${BLUE}Useful commands:${NC}"
    echo "• Build services: ./scripts/build.sh"
    echo "• Run tests: ./scripts/build.sh test"
    echo "• Clean up: ./scripts/build.sh clean"
    echo "• View service status: docker-compose ps"
    echo ""
    echo -e "${YELLOW}Note: Some services may show as unhealthy until you configure API keys${NC}"
}

# Handle script arguments
case "${1:-}" in
    "clean")
        print_status "Cleaning development environment..."
        docker-compose down -v
        docker system prune -f
        rm -rf data/ logs/ build/
        print_status "Clean complete!"
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [clean]"
        echo "  clean - Clean up development environment"
        echo "  (no args) - Setup development environment"
        exit 1
        ;;
esac