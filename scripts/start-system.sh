#!/bin/bash

# Polaris Synapse System Startup Script
# Builds and starts the complete trading system

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Starting Polaris Synapse Trading System${NC}"
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

# Check if Docker is running
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
    print_status "✅ Docker is running"
}

# Build core library
build_core() {
    print_status "Building core library..."
    cd libs/core
    if cargo build --release; then
        print_status "✅ Core library built successfully"
    else
        print_error "Failed to build core library"
        exit 1
    fi
    cd ../..
}

# Build Rust services
build_rust_services() {
    print_status "Building Rust services..."
    
    local services=(
        "market-data-handler"
        "order-gateway" 
        "risk-manager"
        "order-matching-engine"
        "execution-engine"
        "compliance-gateway"
    )
    
    for service in "${services[@]}"; do
        print_status "Building $service..."
        cd "services/$service"
        if cargo build --release; then
            print_status "✅ $service built successfully"
        else
            print_warning "⚠️  $service build failed, continuing..."
        fi
        cd ../..
    done
}

# Build Go services
build_go_services() {
    print_status "Building Go services..."
    
    # Gateway service
    if [ -d "services/gateway" ]; then
        cd services/gateway
        if go mod tidy && go build; then
            print_status "✅ Gateway service built successfully"
        else
            print_warning "⚠️  Gateway service build failed"
        fi
        cd ../..
    fi
    
    # Firehose bridge
    if [ -d "services/firehose_bridge" ]; then
        cd services/firehose_bridge
        if go mod tidy && go build; then
            print_status "✅ Firehose bridge built successfully"
        else
            print_warning "⚠️  Firehose bridge build failed"
        fi
        cd ../..
    fi
}

# Setup Python environment
setup_python() {
    print_status "Setting up Python environment..."
    
    cd services/llm-agents
    
    # Create virtual environment if it doesn't exist
    if [ ! -d "venv" ]; then
        python3 -m venv venv
        print_status "Created Python virtual environment"
    fi
    
    # Install dependencies
    source venv/bin/activate
    pip install --upgrade pip
    if pip install -r requirements.txt; then
        print_status "✅ Python dependencies installed"
    else
        print_warning "⚠️  Some Python dependencies failed to install"
    fi
    deactivate
    
    cd ../..
}

# Start infrastructure services
start_infrastructure() {
    print_status "Starting infrastructure services..."
    
    # Start databases first
    docker-compose up -d postgres postgres-compliance redis
    print_status "Started databases"
    
    # Start Kafka
    docker-compose up -d redpanda
    print_status "Started Kafka"
    
    # Wait for services to be ready
    print_status "Waiting for infrastructure to be ready..."
    sleep 15
    
    # Create Kafka topics
    create_kafka_topics
}

# Create Kafka topics
create_kafka_topics() {
    print_status "Creating Kafka topics..."
    
    local topics=(
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
        docker exec redpanda rpk topic create "$topic" --partitions 12 --replicas 1 2>/dev/null || true
    done
    
    print_status "✅ Kafka topics created"
}

# Start trading services
start_trading_services() {
    print_status "Starting trading services..."
    
    # Start core trading services
    docker-compose up -d market-data-handler order-gateway risk-manager
    sleep 5
    
    # Start matching and execution engines
    docker-compose up -d order-matching-engine execution-engine
    sleep 5
    
    # Start AI and compliance services
    docker-compose up -d llm-agents compliance-gateway
    sleep 5
    
    print_status "✅ Trading services started"
}

# Start monitoring
start_monitoring() {
    print_status "Starting monitoring services..."
    
    docker-compose up -d prometheus grafana
    
    print_status "✅ Monitoring services started"
    print_status "Grafana: http://localhost:3000 (admin/admin)"
    print_status "Prometheus: http://localhost:9090"
}

# Health check
health_check() {
    print_status "Running health checks..."
    
    sleep 10  # Wait for services to start
    
    local services=(
        "http://localhost:3000"     # Grafana
        "http://localhost:9090"     # Prometheus
        "http://localhost:8080"     # Compliance Gateway
    )
    
    for service in "${services[@]}"; do
        if curl -f -s "$service" > /dev/null 2>&1; then
            print_status "✅ $service is healthy"
        else
            print_warning "⚠️  $service is not responding (may still be starting)"
        fi
    done
}

# Show system status
show_status() {
    echo ""
    echo -e "${BLUE}📊 System Status${NC}"
    echo "================="
    
    # Docker containers
    echo -e "\n${YELLOW}Docker Containers:${NC}"
    docker-compose ps
    
    # Service endpoints
    echo -e "\n${YELLOW}Service Endpoints:${NC}"
    echo "• Compliance Gateway: http://localhost:8080"
    echo "• LLM Agents: http://localhost:8000"
    echo "• Grafana: http://localhost:3000"
    echo "• Prometheus: http://localhost:9090"
    
    # Useful commands
    echo -e "\n${YELLOW}Useful Commands:${NC}"
    echo "• View logs: docker-compose logs -f [service-name]"
    echo "• Stop system: docker-compose down"
    echo "• Run tests: ./scripts/test-integration.sh"
    echo "• Check health: curl http://localhost:8080/health"
}

# Main execution
main() {
    check_docker
    
    # Build phase
    print_status "=== BUILD PHASE ==="
    build_core
    build_rust_services
    build_go_services
    setup_python
    
    # Start phase
    print_status "=== STARTUP PHASE ==="
    start_infrastructure
    start_trading_services
    start_monitoring
    
    # Verification phase
    print_status "=== VERIFICATION PHASE ==="
    health_check
    show_status
    
    echo ""
    echo -e "${GREEN}🎉 Polaris Synapse is now running!${NC}"
    echo -e "${BLUE}Tony Stark's Jarvis for Crypto Trading is ready to dominate the markets.${NC}"
    echo ""
}

# Handle script arguments
case "${1:-}" in
    "stop")
        print_status "Stopping Polaris Synapse..."
        docker-compose down
        print_status "System stopped"
        ;;
    "restart")
        print_status "Restarting Polaris Synapse..."
        docker-compose down
        sleep 5
        main
        ;;
    "logs")
        service=${2:-}
        if [ -n "$service" ]; then
            docker-compose logs -f "$service"
        else
            docker-compose logs -f
        fi
        ;;
    "status")
        show_status
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [stop|restart|logs [service]|status]"
        echo "  stop    - Stop all services"
        echo "  restart - Restart the system"
        echo "  logs    - View logs (optionally for specific service)"
        echo "  status  - Show system status"
        echo "  (no args) - Start the system"
        exit 1
        ;;
esac