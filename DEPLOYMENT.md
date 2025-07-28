# Polaris Synapse Deployment Guide

**Tony Stark's Jarvis for Crypto Trading - Production Deployment**

This guide covers deploying Polaris Synapse in production environments with high availability, security, and performance.

## 🚀 Quick Start

### Development Deployment
```bash
# 1. Setup development environment
./scripts/setup-dev.sh

# 2. Configure environment
cp .env.example .env
# Edit .env with your API keys

# 3. Start all services
docker-compose up -d

# 4. Run integration tests
./scripts/test-integration.sh
```

### Production Deployment
```bash
# 1. Build production images
./scripts/build.sh

# 2. Deploy with production configuration
docker-compose -f docker-compose.yml -f deployments/docker-compose.prod.yml up -d

# 3. Verify deployment
./scripts/test-integration.sh quick
```

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Load Balancer (Nginx)                        │
└─────────────────────────────────────────────────────────────────┘
                                │
                ┌───────────────┼───────────────┐
                │               │               │
        ┌───────▼──────┐ ┌──────▼──────┐ ┌─────▼──────┐
        │ Order Gateway│ │Compliance   │ │LLM Agents  │
        │   (Port 8082)│ │Gateway      │ │(Port 8000) │
        │              │ │(Port 8080)  │ │            │
        └──────────────┘ └─────────────┘ └────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
┌───────▼──────┐ ┌──────────────▼──────────────┐ ┌─────▼──────┐
│Market Data   │ │     Message Bus (Kafka)     │ │Risk Manager│
│Handler       │ │                             │ │            │
└──────────────┘ └─────────────────────────────┘ └────────────┘
        │                       │                       │
┌───────▼──────┐ ┌──────────────▼──────────────┐ ┌─────▼──────┐
│Order Matching│ │        Databases            │ │Execution   │
│Engine        │ │  ┌─────────┐ ┌─────────────┐│ │Engine      │
│              │ │  │Orders DB│ │Compliance DB││ │            │
└──────────────┘ │  └─────────┘ └─────────────┘│ └────────────┘
                 │  ┌─────────┐                │
                 │  │Redis    │                │
                 │  └─────────┘                │
                 └─────────────────────────────┘
```

## 📋 Prerequisites

### System Requirements
- **CPU**: 16+ cores recommended
- **RAM**: 32GB+ recommended
- **Storage**: 500GB+ SSD
- **Network**: 1Gbps+ connection
- **OS**: Ubuntu 20.04+ or CentOS 8+

### Software Dependencies
- Docker 24.0+
- Docker Compose 2.0+
- Kubernetes 1.28+ (for K8s deployment)
- Nginx (for load balancing)

### External Services
- Exchange API accounts (Binance, Coinbase)
- LLM API keys (OpenAI, Anthropic)
- Monitoring services (optional)

## 🔧 Configuration

### Environment Variables

Create `.env` file from template:
```bash
cp .env.example .env
```

**Critical Variables:**
```bash
# Trading
BINANCE_PROD_API_KEY=your_key
COINBASE_PROD_API_KEY=your_key
ENABLE_REAL_TRADING=false  # Set to true for live trading

# AI
OPENAI_PROD_API_KEY=your_key
ANTHROPIC_PROD_API_KEY=your_key

# Security
DB_PASSWORD=secure_password
REDIS_PASSWORD=secure_password
JWT_SECRET=your_jwt_secret

# Monitoring
GRAFANA_PROD_PASSWORD=secure_password
SLACK_WEBHOOK_URL=your_webhook
```

### Service Configuration

Edit configuration files in `configs/`:
- `development.yml` - Development settings
- `production.yml` - Production settings

## 🐳 Docker Deployment

### Single Node Deployment

1. **Build Services**
   ```bash
   ./scripts/build.sh
   ```

2. **Start Infrastructure**
   ```bash
   # Start databases and message queue first
   docker-compose up -d postgres postgres-compliance redis redpanda
   
   # Wait for services to be ready
   sleep 30
   ```

3. **Start Trading Services**
   ```bash
   # Start core trading services
   docker-compose up -d market-data-handler order-gateway risk-manager
   docker-compose up -d order-matching-engine execution-engine
   
   # Start AI and compliance services
   docker-compose up -d llm-agents compliance-gateway
   ```

4. **Start Monitoring**
   ```bash
   docker-compose up -d prometheus grafana
   ```

### Production Cluster Deployment

Use the production compose file:
```bash
docker-compose -f docker-compose.yml -f deployments/docker-compose.prod.yml up -d
```

This includes:
- Kafka cluster (3 nodes)
- Redis cluster (6 nodes)
- PostgreSQL with replication
- Load balancing
- Enhanced monitoring

## ☸️ Kubernetes Deployment

### Prerequisites
```bash
# Install kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Verify cluster access
kubectl cluster-info
```

### Deploy to Kubernetes

1. **Create Namespace**
   ```bash
   kubectl apply -f deployments/kubernetes/namespace.yaml
   ```

2. **Deploy Infrastructure**
   ```bash
   # Deploy Kafka cluster
   kubectl apply -f deployments/kubernetes/kafka-cluster.yaml
   
   # Wait for Kafka to be ready
   kubectl wait --for=condition=ready pod -l app=kafka -n polaris-synapse --timeout=300s
   ```

3. **Create Secrets**
   ```bash
   # Database secrets
   kubectl create secret generic database-secrets \
     --from-literal=orders-db-url="postgresql://matcher:${DB_PASSWORD}@orders-primary:5432/orderdb" \
     --from-literal=compliance-db-url="postgresql://compliance:${COMPLIANCE_DB_PASSWORD}@compliance-primary:5432/compliancedb" \
     -n polaris-synapse
   
   # Exchange API secrets
   kubectl create secret generic exchange-secrets \
     --from-literal=binance-api-key="${BINANCE_PROD_API_KEY}" \
     --from-literal=binance-api-secret="${BINANCE_PROD_API_SECRET}" \
     --from-literal=coinbase-api-key="${COINBASE_PROD_API_KEY}" \
     --from-literal=coinbase-api-secret="${COINBASE_PROD_API_SECRET}" \
     -n polaris-synapse
   
   # LLM API secrets
   kubectl create secret generic llm-secrets \
     --from-literal=openai-api-key="${OPENAI_PROD_API_KEY}" \
     --from-literal=anthropic-api-key="${ANTHROPIC_PROD_API_KEY}" \
     -n polaris-synapse
   ```

4. **Deploy Trading Services**
   ```bash
   kubectl apply -f deployments/kubernetes/trading-services.yaml
   ```

5. **Verify Deployment**
   ```bash
   kubectl get pods -n polaris-synapse
   kubectl get services -n polaris-synapse
   ```

## 🔒 Security Configuration

### SSL/TLS Setup

1. **Generate Certificates**
   ```bash
   # Self-signed for development
   openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
     -keyout deployments/nginx/ssl/polaris-synapse.key \
     -out deployments/nginx/ssl/polaris-synapse.crt
   ```

2. **Configure Nginx**
   ```nginx
   server {
       listen 443 ssl;
       server_name polaris-synapse.com;
       
       ssl_certificate /etc/nginx/ssl/polaris-synapse.crt;
       ssl_certificate_key /etc/nginx/ssl/polaris-synapse.key;
       
       location / {
           proxy_pass http://order-gateway:8080;
       }
   }
   ```

### Firewall Configuration

```bash
# Allow only necessary ports
ufw allow 22    # SSH
ufw allow 80    # HTTP
ufw allow 443   # HTTPS
ufw allow 3000  # Grafana (restrict to admin IPs)
ufw allow 9090  # Prometheus (restrict to admin IPs)
ufw enable
```

### API Key Security

1. **Rotate Keys Regularly**
   ```bash
   # Script to rotate API keys
   ./scripts/rotate-keys.sh
   ```

2. **Use Secrets Management**
   - HashiCorp Vault
   - AWS Secrets Manager
   - Azure Key Vault
   - Google Secret Manager

## 📊 Monitoring Setup

### Prometheus Configuration

Prometheus scrapes metrics from all services automatically. Key metrics:

- **Trading Metrics**: Order latency, execution time, fill rates
- **System Metrics**: CPU, memory, disk usage
- **Business Metrics**: P&L, position exposure, risk violations

### Grafana Dashboards

Access Grafana at `http://localhost:3000` (admin/admin)

Pre-configured dashboards:
- **Trading Overview**: Key trading metrics
- **System Health**: Infrastructure monitoring
- **Risk Management**: Position and risk metrics
- **Compliance**: AML/KYC monitoring

### Alerting

Alerts are configured in `deployments/monitoring/rules/`:
- **Critical**: Circuit breaker trips, compliance violations
- **Warning**: High latency, error rates
- **Info**: System status changes

## 🔄 Backup and Recovery

### Database Backups

```bash
# Automated backup script
#!/bin/bash
BACKUP_DIR="/backups/$(date +%Y%m%d)"
mkdir -p $BACKUP_DIR

# Backup orders database
docker exec postgres pg_dump -U matcher orderdb > $BACKUP_DIR/orders.sql

# Backup compliance database
docker exec postgres-compliance pg_dump -U compliance compliancedb > $BACKUP_DIR/compliance.sql

# Compress and upload to S3
tar -czf $BACKUP_DIR.tar.gz $BACKUP_DIR
aws s3 cp $BACKUP_DIR.tar.gz s3://polaris-synapse-backups/
```

### Disaster Recovery

1. **Data Replication**
   - PostgreSQL streaming replication
   - Redis cluster with persistence
   - Kafka topic replication

2. **Service Recovery**
   ```bash
   # Restore from backup
   ./scripts/restore-backup.sh 20241201
   
   # Restart services
   docker-compose up -d
   ```

## 🚀 Scaling

### Horizontal Scaling

Scale individual services based on load:

```bash
# Scale order gateway for high traffic
docker-compose up -d --scale order-gateway=5

# Scale execution engine for multiple exchanges
docker-compose up -d --scale execution-engine=3

# Scale LLM agents for AI processing
docker-compose up -d --scale llm-agents=2
```

### Kubernetes Scaling

```bash
# Auto-scaling based on CPU
kubectl autoscale deployment order-gateway --cpu-percent=70 --min=3 --max=10 -n polaris-synapse

# Manual scaling
kubectl scale deployment llm-agents --replicas=3 -n polaris-synapse
```

### Performance Tuning

1. **Kafka Optimization**
   ```yaml
   # Increase batch size for throughput
   batch.size: 32768
   linger.ms: 1
   compression.type: lz4
   ```

2. **Database Optimization**
   ```sql
   -- Optimize PostgreSQL
   shared_buffers = 256MB
   effective_cache_size = 1GB
   work_mem = 4MB
   ```

3. **Redis Optimization**
   ```
   # Redis configuration
   maxmemory 2gb
   maxmemory-policy allkeys-lru
   ```

## 🧪 Testing in Production

### Health Checks

```bash
# Automated health monitoring
./scripts/test-integration.sh quick
```

### Load Testing

```bash
# Run load tests
./scripts/test-integration.sh load

# Stress testing with custom tools
./scripts/stress-test.sh --concurrent=100 --duration=300
```

### Canary Deployments

```bash
# Deploy new version to subset of traffic
kubectl set image deployment/order-gateway order-gateway=polaris-synapse/order-gateway:v2.0.0 -n polaris-synapse

# Monitor metrics and rollback if needed
kubectl rollout undo deployment/order-gateway -n polaris-synapse
```

## 🔧 Troubleshooting

### Common Issues

1. **High Latency**
   ```bash
   # Check Kafka lag
   docker exec redpanda rpk group describe order-gateway-group
   
   # Check database connections
   docker exec postgres psql -U matcher -c "SELECT * FROM pg_stat_activity;"
   ```

2. **Memory Issues**
   ```bash
   # Check container memory usage
   docker stats
   
   # Increase JVM heap for Kafka
   KAFKA_HEAP_OPTS="-Xmx2G -Xms2G"
   ```

3. **Service Discovery**
   ```bash
   # Check service connectivity
   docker exec order-gateway ping compliance-gateway
   
   # Check DNS resolution
   docker exec order-gateway nslookup kafka
   ```

### Log Analysis

```bash
# View service logs
docker-compose logs -f llm-agents

# Search for errors
docker-compose logs | grep ERROR

# Export logs for analysis
docker-compose logs > polaris-synapse.log
```

## 📞 Support

### Monitoring Dashboards
- **Grafana**: http://localhost:3000
- **Prometheus**: http://localhost:9090
- **Kafka UI**: http://localhost:8080 (if enabled)

### Log Locations
- **Application Logs**: `/var/log/polaris-synapse/`
- **System Logs**: `/var/log/syslog`
- **Audit Logs**: `/var/log/polaris-synapse/audit/`

### Emergency Procedures

1. **Circuit Breaker Activation**
   ```bash
   # Manually trigger circuit breaker
   curl -X POST http://localhost:8083/circuit-breaker/activate
   ```

2. **Emergency Shutdown**
   ```bash
   # Stop all trading immediately
   ./scripts/emergency-stop.sh
   ```

3. **Rollback Deployment**
   ```bash
   # Rollback to previous version
   docker-compose down
   docker-compose up -d --scale order-gateway=0
   # Deploy previous version
   ```

---

**⚠️ Important**: Always test deployments in a staging environment before production. Monitor system metrics closely during initial deployment and have rollback procedures ready.

**🔐 Security Note**: Never commit real API keys or passwords to version control. Use proper secrets management in production.