# 🤝 Contributing to Polaris Auto Trading System

Thank you for your interest in contributing to **Polaris Auto Trading System**! We're excited to have you join our community of developers building the future of AI-powered crypto trading.

## 🚀 **Quick Start for Contributors**

### 1. Fork & Clone
```bash
git clone https://github.com/yourusername/Polaris-auto-trading-system.git
cd Polaris-auto-trading-system
```

### 2. Development Setup
```bash
# Setup development environment
./scripts/setup-dev.sh

# Start infrastructure
docker-compose up -d zookeeper kafka postgres postgres-compliance redis

# Verify setup
./scripts/test-system.sh
```

### 3. Make Your Changes
```bash
# Create feature branch
git checkout -b feature/your-amazing-feature

# Make changes, add tests
# ...

# Test your changes
./scripts/test-integration.sh
```

## 🏗️ **Development Environment**

### Prerequisites
- **Docker & Docker Compose** (latest)
- **Rust 1.75+** with `cargo`, `rustfmt`, `clippy`
- **Python 3.12+** with `pip`, `black`, `pytest`
- **Go 1.21+** with `gofmt`, `golint`
- **8GB+ RAM** for full system testing

### Service Development
```bash
# Rust services (most core services)
cd services/compliance-gateway
cargo build
cargo test
cargo clippy

# Python services (LLM agents)
cd services/llm-agents
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
pytest

# Go services (gateway, bridges)
cd services/gateway
go build
go test ./...
```

## 📝 **Code Standards**

### 🦀 **Rust Guidelines**
```rust
// ✅ Good: Clear, documented, tested
/// Validates an order for compliance requirements
pub fn validate_order(order: &Order) -> Result<(), ValidationError> {
    // Implementation with proper error handling
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_order_success() {
        // Comprehensive test cases
    }
}
```

**Standards:**
- Use `rustfmt` for formatting
- Pass all `clippy` lints
- Write doc comments for public APIs
- Include comprehensive tests
- Handle errors explicitly (no `unwrap()` in production code)

### 🐍 **Python Guidelines**
```python
# ✅ Good: Type hints, docstrings, clean structure
from typing import Optional, Dict, Any
import asyncio

async def analyze_market_sentiment(
    data: Dict[str, Any], 
    confidence_threshold: float = 0.7
) -> Optional[float]:
    """
    Analyzes market sentiment from trading data.
    
    Args:
        data: Market data dictionary
        confidence_threshold: Minimum confidence for valid results
        
    Returns:
        Sentiment score between -1.0 and 1.0, or None if insufficient confidence
    """
    # Implementation
    pass
```

**Standards:**
- Follow PEP 8 (use `black` formatter)
- Include type hints for all functions
- Write comprehensive docstrings
- Use `pytest` for testing
- Handle async operations properly

### 🐹 **Go Guidelines**
```go
// ✅ Good: Clear naming, error handling, documented
package gateway

import (
    "context"
    "fmt"
)

// ProcessRequest handles incoming trading requests with proper error handling
func (s *GatewayServer) ProcessRequest(ctx context.Context, req *Request) (*Response, error) {
    if err := validateRequest(req); err != nil {
        return nil, fmt.Errorf("invalid request: %w", err)
    }
    
    // Implementation
    return &Response{}, nil
}
```

**Standards:**
- Use `gofmt` for formatting
- Follow Go naming conventions
- Handle errors explicitly
- Write table-driven tests
- Include package documentation

## 🧪 **Testing Requirements**

### Test Coverage Targets
- **Unit Tests**: >85% coverage
- **Integration Tests**: All service interactions
- **End-to-End Tests**: Critical trading flows

### Running Tests
```bash
# Full test suite
./scripts/test-all.sh

# Service-specific tests
cd services/compliance-gateway && cargo test
cd services/llm-agents && pytest
cd services/gateway && go test ./...

# Integration tests
./scripts/test-integration.sh

# System health test
./scripts/test-system.sh
```

## 📋 **Pull Request Process**

### 1. Pre-Submission Checklist
- [ ] Code follows style guidelines
- [ ] All tests pass locally
- [ ] Documentation updated
- [ ] Changelog entry added (if applicable)
- [ ] No security vulnerabilities introduced

### 2. PR Description Template
```markdown
## 🎯 **What This PR Does**
Brief description of changes

## 🔧 **Changes Made**
- [ ] Added new feature X
- [ ] Fixed bug Y
- [ ] Updated documentation Z

## 🧪 **Testing**
- [ ] Unit tests added/updated
- [ ] Integration tests pass
- [ ] Manual testing completed

## 📝 **Additional Notes**
Any special considerations or breaking changes
```

## 🐛 **Issue Reporting**

### Bug Reports
Use our bug report template:
```markdown
**🐛 Bug Description**
Clear description of the issue

**🔄 Steps to Reproduce**
1. Step one
2. Step two
3. Bug occurs

**✅ Expected Behavior**
What should happen

**❌ Actual Behavior**
What actually happens

**🖥️ Environment**
- OS: [e.g., macOS 14.0]
- Docker: [e.g., 24.0.0]
- Service: [e.g., compliance-gateway]

**📋 Additional Context**
Logs, screenshots, etc.
```

## 🌟 **Recognition**

Contributors are recognized in:
- **README.md** contributors section
- **Release notes** for significant contributions
- **GitHub discussions** for community highlights

## 📞 **Getting Help**

- **💬 Discussions**: [GitHub Discussions](https://github.com/0xC0FFEE-sudo/Polaris-auto-trading-system/discussions)
- **🐛 Issues**: [GitHub Issues](https://github.com/0xC0FFEE-sudo/Polaris-auto-trading-system/issues)
- **📖 Wiki**: [Project Wiki](https://github.com/0xC0FFEE-sudo/Polaris-auto-trading-system/wiki)

## 🤝 **Community Guidelines**

- **Be Respectful**: Treat everyone with kindness and respect
- **Be Inclusive**: Welcome contributors of all backgrounds and skill levels
- **Be Constructive**: Provide helpful feedback and suggestions
- **Be Patient**: Remember that everyone is learning
- **Have Fun**: Enjoy building amazing technology together!

---

**🚀 Ready to contribute? We can't wait to see what you'll build!**

*Thank you for helping make Polaris Auto Trading System the best AI-powered crypto trading platform! 🙏*