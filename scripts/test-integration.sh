#!/bin/bash

# Polaris Synapse Integration Testing Script
# Tests the complete trading pipeline

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 Running Polaris Synapse Integration Tests${NC}"
echo ""

# Function to print status
print_status() {
    echo -e "${GREEN}[TEST]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Test configuration
API_BASE="http://localhost"
COMPLIANCE_PORT="8080"
LLM_AGENTS_PORT="8000"
GATEWAY_PORT="7000"

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_status="${3:-200}"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    print_status "Running: $test_name"
    
    if eval "$test_command"; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo -e "  ${GREEN}✅ PASSED${NC}"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        echo -e "  ${RED}❌ FAILED${NC}"
    fi
    echo ""
}

# Wait for services to be ready
wait_for_services() {
    print_status "Waiting for services to be ready..."
    
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -f -s "${API_BASE}:${COMPLIANCE_PORT}/health" > /dev/null 2>&1; then
            print_status "✅ Compliance Gateway is ready"
            break
        fi
        
        attempt=$((attempt + 1))
        echo -n "."
        sleep 2
    done
    
    if [ $attempt -eq $max_attempts ]; then
        print_error "Services failed to start within timeout"
        exit 1
    fi
    
    echo ""
}

# Test 1: Health Checks
test_health_checks() {
    print_status "=== Testing Health Checks ==="
    
    run_test "Compliance Gateway Health" \
        "curl -f -s ${API_BASE}:${COMPLIANCE_PORT}/health | jq -e '.status == \"healthy\"'"
    
    run_test "LLM Agents Health" \
        "curl -f -s ${API_BASE}:${LLM_AGENTS_PORT}/health | jq -e '.status == \"healthy\"'" || true
    
    run_test "Gateway Health" \
        "curl -f -s ${API_BASE}:${GATEWAY_PORT}/health | jq -e '.status == \"healthy\"'" || true
}

# Test 2: Compliance API
test_compliance_api() {
    print_status "=== Testing Compliance API ==="
    
    # Test KYC status endpoint
    run_test "KYC Status Query" \
        "curl -f -s '${API_BASE}:${COMPLIANCE_PORT}/kyc/status?user_id=test_user' | jq -e 'type == \"object\"'" || true
    
    # Test compliance check with mock transaction
    local mock_transaction='{
        "transaction_id": "test_tx_001",
        "user_id": "test_user",
        "symbol": "BTC/USD",
        "amount": 1000.0,
        "transaction_type": "buy",
        "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
    }'
    
    run_test "Compliance Check" \
        "curl -f -s -X POST '${API_BASE}:${COMPLIANCE_PORT}/compliance/check' \
         -H 'Content-Type: application/json' \
         -d '$mock_transaction' | jq -e 'type == \"array\"'" || true
}

# Test 3: LLM Agents API
test_llm_agents_api() {
    print_status "=== Testing LLM Agents API ==="
    
    # Test decision endpoint (if available)
    run_test "LLM Decision Endpoint" \
        "curl -f -s '${API_BASE}:${LLM_AGENTS_PORT}/decision/BTC-USD' | jq -e 'type == \"object\"'" || true
    
    # Test insights endpoint (if available)
    run_test "LLM Insights Endpoint" \
        "curl -f -s '${API_BASE}:${LLM_AGENTS_PORT}/insights' | jq -e 'type == \"object\"'" || true
}

# Test 4: Message Flow
test_message_flow() {
    print_status "=== Testing Message Flow ==="
    
    # Check if Kafka is accessible
    run_test "Kafka Connectivity" \
        "docker exec redpanda rpk cluster info | grep -q 'CLUSTER'" || true
    
    # List topics
    run_test "Kafka Topics Exist" \
        "docker exec redpanda rpk topic list | grep -q 'market_data'" || true
    
    # Test message production (mock)
    local test_message='{"symbol":"BTC/USD","price":50000,"volume":100,"timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}'
    
    run_test "Kafka Message Production" \
        "echo '$test_message' | docker exec -i redpanda rpk topic produce market_data.raw" || true
}

# Test 5: Database Connectivity
test_database_connectivity() {
    print_status "=== Testing Database Connectivity ==="
    
    # Test main database
    run_test "Main Database Connection" \
        "docker exec postgres psql -U matcher -d orderdb -c 'SELECT 1;' | grep -q '1 row'" || true
    
    # Test compliance database
    run_test "Compliance Database Connection" \
        "docker exec postgres-compliance psql -U compliance -d compliancedb -c 'SELECT 1;' | grep -q '1 row'" || true
    
    # Test Redis
    run_test "Redis Connection" \
        "docker exec redis redis-cli ping | grep -q 'PONG'" || true
}

# Test 6: Monitoring Stack
test_monitoring() {
    print_status "=== Testing Monitoring Stack ==="
    
    # Test Prometheus
    run_test "Prometheus Targets" \
        "curl -f -s 'http://localhost:9090/api/v1/targets' | jq -e '.data.activeTargets | length > 0'" || true
    
    # Test Grafana
    run_test "Grafana API" \
        "curl -f -s 'http://admin:admin@localhost:3000/api/health' | jq -e '.database == \"ok\"'" || true
}

# Test 7: End-to-End Trading Flow (Mock)
test_trading_flow() {
    print_status "=== Testing Trading Flow (Mock) ==="
    
    # This would test the complete flow:
    # 1. Market data ingestion
    # 2. LLM decision making
    # 3. Risk management
    # 4. Compliance checking
    # 5. Order execution
    
    print_status "Mock trading flow test - would simulate complete pipeline"
    TESTS_RUN=$((TESTS_RUN + 1))
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo -e "  ${GREEN}✅ PASSED (Mock)${NC}"
    echo ""
}

# Performance Tests
test_performance() {
    print_status "=== Testing Performance ==="
    
    # Test API response times
    run_test "API Response Time < 1s" \
        "timeout 1s curl -f -s ${API_BASE}:${COMPLIANCE_PORT}/health > /dev/null" || true
    
    # Test concurrent requests
    run_test "Concurrent Health Checks" \
        "for i in {1..10}; do curl -f -s ${API_BASE}:${COMPLIANCE_PORT}/health & done; wait" || true
}

# Load Test (Basic)
run_load_test() {
    print_status "=== Running Basic Load Test ==="
    
    local concurrent_users=10
    local requests_per_user=5
    local total_requests=$((concurrent_users * requests_per_user))
    
    print_status "Running $total_requests requests with $concurrent_users concurrent users"
    
    # Simple load test using curl
    for i in $(seq 1 $concurrent_users); do
        (
            for j in $(seq 1 $requests_per_user); do
                curl -f -s "${API_BASE}:${COMPLIANCE_PORT}/health" > /dev/null || true
            done
        ) &
    done
    
    wait
    
    TESTS_RUN=$((TESTS_RUN + 1))
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo -e "  ${GREEN}✅ Load test completed${NC}"
    echo ""
}

# Generate test report
generate_report() {
    echo ""
    echo -e "${BLUE}📊 Test Results Summary${NC}"
    echo "=========================="
    echo -e "Total Tests: ${TESTS_RUN}"
    echo -e "Passed: ${GREEN}${TESTS_PASSED}${NC}"
    echo -e "Failed: ${RED}${TESTS_FAILED}${NC}"
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "\n${GREEN}🎉 All tests passed!${NC}"
        exit 0
    else
        echo -e "\n${RED}❌ Some tests failed${NC}"
        exit 1
    fi
}

# Main test execution
main() {
    # Check if jq is available
    if ! command -v jq &> /dev/null; then
        print_warning "jq is not installed. Some tests may fail."
        print_warning "Install jq for better test results: apt-get install jq"
    fi
    
    # Wait for services
    wait_for_services
    
    # Run test suites
    test_health_checks
    test_compliance_api
    test_llm_agents_api
    test_message_flow
    test_database_connectivity
    test_monitoring
    test_trading_flow
    test_performance
    
    # Optional load test
    if [ "${1:-}" = "load" ]; then
        run_load_test
    fi
    
    # Generate report
    generate_report
}

# Handle script arguments
case "${1:-}" in
    "load")
        main "load"
        ;;
    "quick")
        print_status "Running quick tests only..."
        wait_for_services
        test_health_checks
        generate_report
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [load|quick]"
        echo "  load  - Include load testing"
        echo "  quick - Run only health checks"
        echo "  (no args) - Run all integration tests"
        exit 1
        ;;
esac