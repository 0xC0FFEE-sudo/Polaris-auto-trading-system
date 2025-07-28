pub mod block_processor {
    use arrow::array::{Array, StringArray, TimestampMillisecondArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use chrono::{DateTime, Utc};
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    pub struct BlockEvent {
        pub block_number: String,
        pub kafka_write_time: DateTime<Utc>,
    }

    impl BlockEvent {
        pub fn new(block_number: String, kafka_write_time: DateTime<Utc>) -> Self {
            Self {
                block_number,
                kafka_write_time,
            }
        }

        pub fn to_arrow_record_batch(&self) -> RecordBatch {
            let block_numbers = StringArray::from(vec![self.block_number.clone()]);
            let timestamps = TimestampMillisecondArray::from(vec![self.kafka_write_time.timestamp_millis()]);
            
            let schema = Arc::new(Schema::new(vec![
                Field::new("block_number", DataType::Utf8, false),
                Field::new("kafka_write_time", DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, None), false),
            ]));
            
            RecordBatch::try_new(schema, vec![Arc::new(block_numbers), Arc::new(timestamps)]).unwrap()
        }
    }
}

pub mod kafka_utils {
    use rdkafka::config::ClientConfig;
    use rdkafka::consumer::{Consumer, StreamConsumer};
    use rdkafka::producer::{FutureProducer, FutureRecord};
    use rdkafka::message::Message;
    use std::time::Duration;
    use tracing::info;

    pub fn create_kafka_consumer(brokers: &str, group_id: &str) -> StreamConsumer {
        info!("Creating Kafka consumer for brokers: {}, group: {}", brokers, group_id);
        ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "false")
            .create()
            .expect("Failed to create Kafka consumer")
    }

    pub fn create_kafka_producer(brokers: &str) -> FutureProducer {
        info!("Creating Kafka producer for brokers: {}", brokers);
        ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .expect("Failed to create Kafka producer")
    }

    pub async fn consume_messages(consumer: &StreamConsumer, topic: &str, timeout: Duration) -> Option<String> {
        match consumer.recv().await {
            Err(e) => {
                tracing::error!("Kafka consume error: {:?}", e);
                None
            }
            Ok(m) => {
                if let Some(payload) = m.payload() {
                    let message = String::from_utf8_lossy(payload).to_string();
                    tracing::debug!("Consumed message from {}: {}", topic, message);
                    Some(message)
                } else {
                    None
                }
            }
        }
    }

    pub async fn produce_message(
        producer: &FutureProducer,
        topic: &str,
        key: &str,
        payload: &str,
    ) -> Result<(), rdkafka::error::KafkaError> {
        let record = FutureRecord::to(topic)
            .key(key)
            .payload(payload);
        
        match producer.send(record, Duration::from_secs(1)).await {
            Ok(_) => {
                tracing::debug!("Message sent to topic {}: {}", topic, payload);
                Ok(())
            }
            Err((e, _)) => {
                tracing::error!("Failed to send message to topic {}: {:?}", topic, e);
                Err(e)
            }
        }
    }
}

pub mod metrics {
    use prometheus::{register_counter_vec, register_histogram_vec, CounterVec, HistogramVec, register_counter, Counter, register_gauge, Gauge};
    use once_cell::sync::Lazy;

    pub static KAFKA_MESSAGES_CONSUMED: Lazy<CounterVec> = Lazy::new(|| {
        register_counter_vec!("kafka_messages_consumed_total", "Number of Kafka messages consumed", &["topic"]).unwrap()
    });

    pub static KAFKA_MESSAGES_PRODUCED: Lazy<CounterVec> = Lazy::new(|| {
        register_counter_vec!("kafka_messages_produced_total", "Number of Kafka messages produced", &["topic"]).unwrap()
    });

    pub static ARROW_RECORD_BATCHES_PRODUCED: Lazy<CounterVec> = Lazy::new(|| {
        register_counter_vec!("arrow_record_batches_produced_total", "Number of Arrow record batches produced", &["stream"]).unwrap()
    });

    pub static LATENCY_HISTOGRAM: Lazy<HistogramVec> = Lazy::new(|| {
        register_histogram_vec!("processing_latency_seconds", "Processing latency in seconds", &["component"]).unwrap()
    });

    pub static ORDER_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
        register_histogram_vec!("order_latency_nanoseconds", "Order processing latency in nanoseconds", &["symbol"]).unwrap()
    });

    pub static EXECUTION_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
        register_histogram_vec!("execution_latency_nanoseconds", "Order execution latency in nanoseconds", &["symbol"]).unwrap()
    });

    pub static MARKET_DATA_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
        register_histogram_vec!("market_data_latency_nanoseconds", "Market data processing latency in nanoseconds", &["symbol"]).unwrap()
    });

    pub static CONNECTION_FAILURES: Lazy<Counter> = Lazy::new(|| {
        register_counter!("connection_failures_total", "Total number of connection failures").unwrap()
    });

    pub static CIRCUIT_BREAKER_TRIPPED: Lazy<Counter> = Lazy::new(|| {
        register_counter!("circuit_breaker_tripped_total", "Total number of circuit breaker trips").unwrap()
    });

    pub static ORDERS_MATCHED: Lazy<Counter> = Lazy::new(|| {
        register_counter!("orders_matched_total", "Total number of orders matched").unwrap()
    });

    pub static RISK_VIOLATIONS: Lazy<Counter> = Lazy::new(|| {
        register_counter!("risk_violations_total", "Total number of risk violations").unwrap()
    });

    pub static POSITION_EXPOSURE: Lazy<prometheus::GaugeVec> = Lazy::new(|| {
        prometheus::register_gauge_vec!("position_exposure_usd", "Current position exposure in USD", &["symbol"]).unwrap()
    });
}

pub mod exchange_connector {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use chrono::{DateTime, Utc};
    use tokio::sync::RwLock;
    use std::sync::Arc;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub enum ExchangeType {
        Binance,
        Coinbase,
        Kraken,
        Solana,
        Ethereum,
    }

    impl std::fmt::Display for ExchangeType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ExchangeType::Binance => write!(f, "binance"),
                ExchangeType::Coinbase => write!(f, "coinbase"),
                ExchangeType::Kraken => write!(f, "kraken"),
                ExchangeType::Solana => write!(f, "solana"),
                ExchangeType::Ethereum => write!(f, "ethereum"),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarketData {
        pub symbol: String,
        pub price: f64,
        pub volume: f64,
        pub timestamp: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OrderBookSnapshot {
        pub symbol: String,
        pub bids: Vec<(f64, f64)>,
        pub asks: Vec<(f64, f64)>,
        pub timestamp: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TradeEvent {
        pub symbol: String,
        pub price: f64,
        pub quantity: f64,
        pub side: String,
        pub timestamp: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OrderExecution {
        pub price: f64,
        pub quantity: f64,
        pub status: OrderStatus,
        pub timestamp: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum OrderStatus {
        Filled,
        Partial,
        Rejected,
        Cancelled,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FillReport {
        pub fill_id: String,
        pub order_id: String,
        pub price: f64,
        pub quantity: f64,
        pub timestamp: DateTime<Utc>,
    }

    pub struct ExchangeConnector {
        exchange_type: ExchangeType,
        api_key: String,
        api_secret: String,
        endpoint: String,
        rate_limit: u64,
        connections: Arc<RwLock<u32>>,
    }

    impl ExchangeConnector {
        pub fn new(
            exchange_type: ExchangeType,
            api_key: String,
            api_secret: String,
            endpoint: String,
            rate_limit: u64,
        ) -> Self {
            Self {
                exchange_type,
                api_key,
                api_secret,
                endpoint,
                rate_limit,
                connections: Arc::new(RwLock::new(0)),
            }
        }

        pub async fn execute_order(&self, order: &crate::Order) -> Result<OrderExecution, String> {
            // Simulate order execution
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            
            Ok(OrderExecution {
                price: order.price,
                quantity: order.quantity,
                status: OrderStatus::Filled,
                timestamp: Utc::now(),
            })
        }

        pub fn active_connections(&self) -> u32 {
            // This would be implemented with actual connection tracking
            1
        }

        pub async fn get_market_data(&self, symbol: &str) -> Result<MarketData, String> {
            // Simulate market data retrieval
            Ok(MarketData {
                symbol: symbol.to_string(),
                price: 50000.0,
                volume: 1000.0,
                timestamp: Utc::now(),
            })
        }
    }

    // Common order structure used across services
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Order {
        pub order_id: String,
        pub client_order_id: String,
        pub symbol: String,
        pub price: f64,
        pub quantity: f64,
        pub side: String,
        pub order_type: String,
        pub time_in_force: String,
        pub user_id: String,
        pub timestamp: DateTime<Utc>,
    }
}

pub mod auth {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ApiKeyAuth {
        pub key: String,
        pub secret: String,
        pub permissions: Vec<String>,
    }

    pub fn authenticate(api_key: &str, api_secret: &str) -> Result<String, String> {
        // Simplified authentication - in production this would validate against a database
        if api_key == "user1" && api_secret == "secret1" {
            Ok("user1".to_string())
        } else {
            Err("Invalid credentials".to_string())
        }
    }

    pub fn authorize(user_id: &str, permission: &str) -> bool {
        // Simplified authorization - in production this would check user permissions
        match user_id {
            "user1" => permission == "trade",
            _ => false,
        }
    }
}

pub mod rate_limiter {
    use std::time::{Duration, Instant};
    use std::collections::VecDeque;

    #[derive(Debug)]
    pub struct RateLimitExceeded;

    pub struct RateLimiter {
        max_requests: u64,
        window: Duration,
        requests: VecDeque<Instant>,
    }

    impl RateLimiter {
        pub fn new(max_requests: u64) -> Self {
            Self {
                max_requests,
                window: Duration::from_secs(60), // 1 minute window
                requests: VecDeque::new(),
            }
        }

        pub fn check(&mut self) -> Result<(), RateLimitExceeded> {
            let now = Instant::now();
            
            // Remove old requests outside the window
            while let Some(&front) = self.requests.front() {
                if now.duration_since(front) > self.window {
                    self.requests.pop_front();
                } else {
                    break;
                }
            }

            // Check if we're at the limit
            if self.requests.len() >= self.max_requests as usize {
                return Err(RateLimitExceeded);
            }

            // Add current request
            self.requests.push_back(now);
            Ok(())
        }

        pub fn get_current_qps(&self) -> f64 {
            self.requests.len() as f64 / self.window.as_secs_f64()
        }
    }
}

pub mod order_validator {
    use crate::exchange_connector::Order;

    #[derive(Debug)]
    pub enum OrderValidationError {
        InvalidPrice,
        InvalidQuantity,
        InvalidSymbol,
        InvalidSide,
        InvalidOrderType,
    }

    impl std::fmt::Display for OrderValidationError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                OrderValidationError::InvalidPrice => write!(f, "Invalid price"),
                OrderValidationError::InvalidQuantity => write!(f, "Invalid quantity"),
                OrderValidationError::InvalidSymbol => write!(f, "Invalid symbol"),
                OrderValidationError::InvalidSide => write!(f, "Invalid side"),
                OrderValidationError::InvalidOrderType => write!(f, "Invalid order type"),
            }
        }
    }

    pub fn validate_order(order: &Order) -> Result<(), OrderValidationError> {
        // Price validation
        if order.price <= 0.0 {
            return Err(OrderValidationError::InvalidPrice);
        }

        // Quantity validation
        if order.quantity <= 0.0 {
            return Err(OrderValidationError::InvalidQuantity);
        }

        // Symbol validation
        if order.symbol.is_empty() {
            return Err(OrderValidationError::InvalidSymbol);
        }

        // Side validation
        if !matches!(order.side.as_str(), "buy" | "sell") {
            return Err(OrderValidationError::InvalidSide);
        }

        // Order type validation
        if !matches!(order.order_type.as_str(), "limit" | "market" | "stop" | "stop_limit") {
            return Err(OrderValidationError::InvalidOrderType);
        }

        Ok(())
    }
}

pub mod risk_engine {
    use crate::exchange_connector::Order;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PositionLimit {
        pub max_long: f64,
        pub max_short: f64,
        pub max_exposure: f64,
        pub max_daily_pnl: f64,
        pub max_drawdown: f64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RiskRule {
        pub name: String,
        pub description: String,
        pub condition: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ComplianceCheck {
        pub name: String,
        pub description: String,
        pub validation: String,
    }

    pub struct RiskEngine {
        position_limits: HashMap<String, PositionLimit>,
        risk_rules: Vec<RiskRule>,
        compliance_rules: Vec<ComplianceCheck>,
        positions: HashMap<String, f64>,
        pnl: HashMap<String, f64>,
    }

    impl RiskEngine {
        pub fn new() -> Self {
            Self {
                position_limits: HashMap::new(),
                risk_rules: Vec::new(),
                compliance_rules: Vec::new(),
                positions: HashMap::new(),
                pnl: HashMap::new(),
            }
        }

        pub fn update_position_limits(&mut self, limits: HashMap<String, PositionLimit>) {
            self.position_limits = limits;
        }

        pub fn add_risk_rules(&mut self, rules: Vec<RiskRule>) {
            self.risk_rules = rules;
        }

        pub fn add_compliance_rules(&mut self, rules: Vec<ComplianceCheck>) {
            self.compliance_rules = rules;
        }

        pub fn check_position_limits(&self, order: &Order) -> Option<Vec<String>> {
            let mut violations = Vec::new();
            
            if let Some(limit) = self.position_limits.get(&order.symbol) {
                let current_position = self.positions.get(&order.symbol).unwrap_or(&0.0);
                let new_position = match order.side.as_str() {
                    "buy" => current_position + order.quantity,
                    "sell" => current_position - order.quantity,
                    _ => *current_position,
                };

                if new_position > limit.max_long {
                    violations.push(format!("Position would exceed max long limit: {} > {}", new_position, limit.max_long));
                }

                if new_position < -limit.max_short {
                    violations.push(format!("Position would exceed max short limit: {} < -{}", new_position, limit.max_short));
                }

                let exposure = new_position.abs() * order.price;
                if exposure > limit.max_exposure {
                    violations.push(format!("Exposure would exceed limit: {} > {}", exposure, limit.max_exposure));
                }
            }

            if violations.is_empty() { None } else { Some(violations) }
        }

        pub fn check_risk_rules(&self, _order: &Order) -> Option<Vec<String>> {
            // Simplified risk rule checking
            // In production, this would evaluate the rule conditions
            None
        }

        pub fn check_compliance(&self, _order: &Order) -> Option<Vec<String>> {
            // Simplified compliance checking
            // In production, this would validate against compliance rules
            None
        }

        pub fn get_position_exposure(&self, symbol: &str) -> f64 {
            self.positions.get(symbol).unwrap_or(&0.0).abs() * 50000.0 // Simplified
        }

        pub fn get_position_pnl(&self, symbol: &str) -> f64 {
            self.pnl.get(symbol).unwrap_or(&0.0).clone()
        }
    }
}

// Re-export common types
pub use exchange_connector::Order;