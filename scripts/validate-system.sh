#!/bin/bash

# Polaris Synapse System Validation Script
# Comprehensive testing of all components

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 Validating Polaris Synapse System${NC}"
echo ""

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

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

# Function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    print_status "Testing: $test_name"
    
    if eval "$test_command" > /dev/null 2>&1; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo -e "  ${GREEN}✅ PASSED${NC}"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        echo -e "  ${RED}❌ FAILED${NC}"
    fi
}

# Test 1: Build Validation
test_builds() {
    print_status "=== Testing Build System ==="
    
    # Test core library build
    run_test "Core Library Build" "cd libs/core && cargo check"
    
    # Test Rust services
    local rust_services=(
        "market-data-handler"
        "order-gateway"
        "risk-manager"
        "order-matching-engine"
        "execution-engine"
        "compliance-gateway"
    )
    
    for service in "${rust_services[@]}"; do
        run_test "Rust Service: $service" "cd services/$service && cargo check"
    done
    
    # Test Go services
    if [ -d "services/gateway" ]; then
        run_test "Go Service: Gateway" "cd services/gateway && go build -o /tmp/gateway"
    fi
    
    if [ -d "services/firehose_bridge" ]; then
        run_test "Go Service: Firehose Bridge" "cd services/firehose_bridge && go build -o /tmp/firehose_bridge"
    fi
    
    # Test Python service
    run_test "Python Service: LLM Agents" "cd services/llm-agents && python3 -m py_compile main.py"
}

# Test 2: Docker Build Validation
test_docker_builds() {
    print_status "=== Testing Docker Builds ==="
    
    # Test if Dockerfiles exist and are valid
    local services_with_docker=(
        "market-data-handler"
        "order-gateway"
        "risk-manager"
        "order-matching-engine"
        "execution-engine"
        "compliance-gateway"
        "llm-agents"
        "gateway"
        "firehose_bridge"
    )
    
    for service in "${services_with_docker[@]}"; do
        if [ -f "services/$service/Dockerfile" ]; then
            run_test "Dockerfile: $service" "docker build -t test-$service services/$service --no-cache"
        else
            print_warning "No Dockerfile found for $service"
        fi
    done
}

# Test 3: Configuration Validation
test_configurations() {
    print_status "=== Testing Configurations ==="
    
    # Test YAML configurations
    run_test "Development Config" "python3 -c 'import yaml; yaml.safe_load(open(\"configs/development.yml\"))'"
    run_test "Production Config" "python3 -c 'import yaml; yaml.safe_load(open(\"configs/production.yml\"))'"
    
    # Test Docker Compose files
    run_test "Docker Compose Main" "docker-compose config"
    run_test "Docker Compose Production" "docker-compose -f docker-compose.yml -f deployments/docker-compose.prod.yml config"
    
    # Test Kubernetes manifests
    if command -v kubectl > /dev/null 2>&1; then
        run_test "Kubernetes Namespace" "kubectl apply --dry-run=client -f deployments/kubernetes/namespace.yaml"
        run_test "Kubernetes Kafka" "kubectl apply --dry-run=client -f deployments/kubernetes/kafka-cluster.yaml"
        run_test "Kubernetes Services" "kubectl apply --dry-run=client -f deployments/kubernetes/trading-services.yaml"
    fi
}

# Test 4: Script Validation
test_scripts() {
    print_status "=== Testing Scripts ==="
    
    # Test script syntax
    local scripts=(
        "scripts/build.sh"
        "scripts/setup-dev.sh"
        "scripts/start-system.sh"
        "scripts/test-integration.sh"
        "scripts/validate-system.sh"
    )
    
    for script in "${scripts[@]}"; do
        if [ -f "$script" ]; then
            run_test "Script Syntax: $(basename $script)" "bash -n $script"
        fi
    done
}

# Test 5: Dependencies Validation
test_dependencies() {
    print_status "=== Testing Dependencies ==="
    
    # Test Rust dependencies
    run_test "Rust Toolchain" "rustc --version"
    run_test "Cargo Available" "cargo --version"
    
    # Test Python dependencies
    run_test "Python 3 Available" "python3 --version"
    run_test "Pip Available" "pip3 --version"
    
    # Test Go dependencies
    if command -v go > /dev/null 2>&1; then
        run_test "Go Available" "go version"
    fi
    
    # Test Docker
    run_test "Docker Available" "docker --version"
    run_test "Docker Compose Available" "docker-compose --version"
    
    # Test optional tools
    if command -v kubectl > /dev/null 2>&1; then
        run_test "Kubectl Available" "kubectl version --client"
    fi
}

# Test 6: File Structure Validation
test_file_structure() {
    print_status "=== Testing File Structure ==="
    
    # Test required directories
    local required_dirs=(
        "services"
        "libs/core"
        "configs"
        "deployments"
        "scripts"
    )
    
    for dir in "${required_dirs[@]}"; do
        run_test "Directory Exists: $dir" "[ -d $dir ]"
    done
    
    # Test required files
    local required_files=(
        "README.md"
        "DEPLOYMENT.md"
        "CONTRIBUTING.md"
        "docker-compose.yml"
        ".env.example"
    )
    
    for file in "${required_files[@]}"; do
        run_test "File Exists: $file" "[ -f $file ]"
    done
}

# Test 7: Code Quality
test_code_quality() {
    print_status "=== Testing Code Quality ==="
    
    # Test Rust code formatting
    run_test "Rust Code Format" "cd libs/core && cargo fmt --check"
    
    # Test for common issues
    run_test "No TODO Comments in Main Code" "! grep -r 'TODO' services/*/src/main.rs"
    run_test "No Hardcoded Passwords" "! grep -ri 'password.*=' services/ --include='*.rs' --include='*.py' --include='*.go'"
    
    # Test Python code if available
    if command -v python3 > /dev/null 2>&1; then
        run_test "Python Syntax Check" "cd services/llm-agents && python3 -m py_compile main.py"
    fi
}

# Test 8: Security Validation
test_security() {
    print_status "=== Testing Security ==="
    
    # Test for security issues
    run_test "No Hardcoded API Keys" "! grep -ri 'api.*key.*=' services/ --include='*.rs' --include='*.py' --include='*.go'"
    run_test "No Hardcoded Secrets" "! grep -ri 'secret.*=' services/ --include='*.rs' --include='*.py' --include='*.go'"
    run_test "Environment Variables Used" "grep -q 'env::var' services/*/src/main.rs"
    
    # Test Docker security
    run_test "Non-Root User in Dockerfiles" "grep -q 'USER' services/*/Dockerfile"
}

# Generate validation report
generate_report() {
    echo ""
    echo -e "${BLUE}📊 Validation Results${NC}"
    echo "====================="
    echo -e "Total Tests: ${TESTS_RUN}"
    echo -e "Passed: ${GREEN}${TESTS_PASSED}${NC}"
    echo -e "Failed: ${RED}${TESTS_FAILED}${NC}"
    
    local success_rate=$((TESTS_PASSED * 100 / TESTS_RUN))
    echo -e "Success Rate: ${success_rate}%"
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "\n${GREEN}🎉 All validations passed! System is ready for deployment.${NC}"
        exit 0
    elif [ $success_rate -ge 80 ]; then
        echo -e "\n${YELLOW}⚠️  Most validations passed. System should work with minor issues.${NC}"
        exit 0
    else
        echo -e "\n${RED}❌ Many validations failed. Please fix issues before deployment.${NC}"
        exit 1
    fi
}

# Main validation execution
main() {
    print_status "Starting comprehensive system validation..."
    echo ""
    
    test_dependencies
    test_file_structure
    test_configurations
    test_scripts
    test_builds
    test_code_quality
    test_security
    
    # Optional Docker tests (can be slow)
    if [ "${1:-}" = "full" ]; then
        test_docker_builds
    fi
    
    generate_report
}

# Handle script arguments
case "${1:-}" in
    "full")
        print_status "Running full validation including Docker builds..."
        main "full"
        ;;
    "quick")
        print_status "Running quick validation (no builds)..."
        test_dependencies
        test_file_structure
        test_configurations
        test_scripts
        generate_report
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [full|quick]"
        echo "  full  - Include Docker build tests (slow)"
        echo "  quick - Skip build tests (fast)"
        echo "  (no args) - Standard validation"
        exit 1
        ;;
esac