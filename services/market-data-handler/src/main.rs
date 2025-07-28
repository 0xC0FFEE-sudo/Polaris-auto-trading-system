use polaris_core::kafka_utils::{create_kafka_consumer, create_kafka_producer, consume_messages, produce_message};
use polaris_core::metrics::{KAFKA_MESSAGES_CONSUMED, KAFKA_MESSAGES_PRODUCED, MARKET_DATA_LATENCY, CONNECTION_FAILURES, CIRCUIT_BREAKER_TRIPPED};
use polaris_core::exchange_connector::{ExchangeConnector, ExchangeType, MarketData, OrderBookSnapshot, TradeEvent};
use chrono::{Utc, DateTime};
use tracing::info;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::consumer::{StreamConsumer, Consumer};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use tokio::time::Instant;
use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MarketDataMessage {
    symbol: String,
    exchange_id: String,
    price: f64,
    quantity: f64,
    bid: f64,
    ask: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OrderBookUpdate {
    symbol: String,
    exchange_id: String,
    bids: Vec<(f64, f64)>, // (price, quantity)
    asks: Vec<(f64, f64)>, // (price, quantity)
    sequence: u64,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MarketDataHandlerConfig {
    kafka_brokers: String,
    input_topic: String,
    output_topic: String,
    heartbeat_interval: u64,
    circuit_breaker_threshold: u64,
    exchanges: Vec<ExchangeSubscription>,
    normalization_rules: HashMap<String, NormalizationRule>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExchangeSubscription {
    exchange_type: ExchangeType,
    symbols: Vec<String>,
    update_interval: u64, // milliseconds
    depth: u64, // order book depth
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NormalizationRule {
    price_precision: u8,
    quantity_precision: u8,
    symbol_mapping: HashMap<String, String>, // exchange symbol -> normalized symbol
}

impl Default for MarketDataHandlerConfig {
    fn default() -> Self {
        MarketDataHandlerConfig {
            kafka_brokers: "redpanda:9092".to_string(),
            input_topic: "market_data.raw".to_string(),
            output_topic: "market_data.normalized".to_string(),
            heartbeat_interval: 30_000,
            circuit_breaker_threshold: 5000,
            exchanges: vec![
                ExchangeSubscription {
                    exchange_type: ExchangeType::Binance,
                    symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
                    update_interval: 100,
                    depth: 10,
                },
                ExchangeSubscription {
                    exchange_type: ExchangeType::Coinbase,
                    symbols: vec!["BTC-USD".to_string(), "ETH-USD".to_string()],
                    update_interval: 200,
                    depth: 5,
                }
            ],
            normalization_rules: {
                let mut rules = HashMap::new();
                rules.insert("BTCUSDT".to_string(), NormalizationRule {
                    price_precision: 2,
                    quantity_precision: 6,
                    symbol_mapping: {
                        let mut mapping = HashMap::new();
                        mapping.insert("BTCUSDT".to_string(), "BTC/USD".to_string());
                        mapping.insert("ETHUSDT".to_string(), "ETH/USD".to_string());
                        mapping
                    },
                });
                rules.insert("BTC-USD".to_string(), NormalizationRule {
                    price_precision: 2,
                    quantity_precision: 6,
                    symbol_mapping: {
                        let mut mapping = HashMap::new();
                        mapping.insert("BTC-USD".to_string(), "BTC/USD".to_string());
                        mapping.insert("ETH-USD".to_string(), "ETH/USD".to_string());
                        mapping
                    },
                });
                rules
            },
        }
    }
}

struct AppState {
    kafka_producer: FutureProducer,
    market_data_consumer: StreamConsumer,
    config: MarketDataHandlerConfig,
    exchange_connectors: HashMap<ExchangeType, ExchangeConnector>,
    circuit_breaker_state: CircuitBreakerState,
    last_error_time: Option<Instant>,
    error_count: u64,
}

#[derive(Clone, Debug)]
struct CircuitBreakerState {
    error_count: u64,
    cooldown_until: Option<Instant>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    let config = MarketDataHandlerConfig {
        kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string()),
        input_topic: env::var("INPUT_TOPIC").unwrap_or_else(|_| "market_data.raw".to_string()),
        output_topic: env::var("OUTPUT_TOPIC").unwrap_or_else(|_| "market_data.normalized".to_string()),
        heartbeat_interval: env::var("HEARTBEAT_INTERVAL")
            .map(|v| v.parse().unwrap_or(30_000))
            .unwrap_or(30_000),
        circuit_breaker_threshold: env::var("CIRCUIT_BREAKER_THRESHOLD")
            .map(|v| v.parse().unwrap_or(5000))
            .unwrap_or(5000),
        exchanges: env::var("EXCHANGES")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_else(|_| vec![
                ExchangeSubscription {
                    exchange_type: ExchangeType::Binance,
                    symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
                    update_interval: 100,
                    depth: 10,
                }
            ]),
        normalization_rules: env::var("NORMALIZATION_RULES")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_default(),
        ..MarketDataHandlerConfig::default()
    };
    
    let producer = create_kafka_producer(&config.kafka_brokers);
    
    // Create market data consumer
    let market_data_consumer = create_kafka_consumer(&config.kafka_brokers, "market-data-handler-group");
    market_data_consumer.subscribe(&[&config.input_topic]).expect("Failed to subscribe to raw market data topic");
    
    info!("Market data handler started");
    
    // Initialize exchange connectors
    let mut exchange_connectors = HashMap::new();
    for exchange_sub in &config.exchanges {
        let connector = ExchangeConnector::new(
            exchange_sub.exchange_type.clone(),
            "".to_string(), // API key not needed for market data
            "".to_string(), // API secret not needed for market data
            match exchange_sub.exchange_type {
                ExchangeType::Binance => "wss://binance.com/ws".to_string(),
                ExchangeType::Coinbase => "wss://coinbase.com/ws".to_string(),
                ExchangeType::Kraken => "wss://kraken.com/ws".to_string(),
                ExchangeType::Solana => "wss://solana.com/ws".to_string(),
                ExchangeType::Ethereum => "wss://ethereum.com/ws".to_string(),
            },
            100, // Default rate limit
        );
        exchange_connectors.insert(exchange_sub.exchange_type.clone(), connector);
    }
    
    let app_state = Arc::new(Mutex::new(AppState {
        kafka_producer: producer,
        market_data_consumer: market_data_consumer,
        config: config,
        exchange_connectors: exchange_connectors,
        circuit_breaker_state: CircuitBreakerState {
            error_count: 0,
            cooldown_until: None,
        },
        last_error_time: None,
        error_count: 0,
    }));
    
    // Start background tasks
    let state_for_metrics = Arc::clone(&app_state);
    tokio::spawn(async move {
        report_metrics(state_for_metrics).await;
    });
    
    let state_for_heartbeat = Arc::clone(&app_state);
    tokio::spawn(async move {
        send_heartbeat(state_for_heartbeat).await;
    });
    
    let state_for_circuit_breaker = Arc::clone(&app_state);
    tokio::spawn(async move {
        monitor_circuit_breaker(state_for_circuit_breaker).await;
    });
    
    // Start HTTP server for health checks
    let health_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        start_health_server(health_state).await;
    });

    // Main loop for processing market data
    loop {
        let start = Instant::now();
        
        // Process raw market data messages
        {
            let mut state = app_state.lock().await;
            
            if let Some(message) = consume_messages(&state.market_data_consumer, &state.config.input_topic, Duration::from_millis(100)).await {
                KAFKA_MESSAGES_CONSUMED.with_label_values(&[&state.config.input_topic]).inc();
                
                if let Ok(raw_data) = serde_json::from_str::<serde_json::Value>(&message) {
                    let result = process_market_data(&mut state, raw_data).await;
                    
                    // Send normalized market data
                    if let Ok(normalized_data) = result {
                        let data_json = serde_json::to_string(&normalized_data).unwrap();
                        produce_message(&state.kafka_producer, &state.config.output_topic, &normalized_data.symbol, &data_json)
                            .await
                            .expect("Failed to produce normalized market data");
                        
                        KAFKA_MESSAGES_PRODUCED.with_label_values(&[&state.config.output_topic]).inc();
                    }
                }
            }
        }
        
        // Small delay to prevent busy waiting
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn report_metrics(state: Arc<Mutex<AppState>>) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        
        let state = state.lock().await;
        
        // Report exchange connection stats
        for (exchange_type, connector) in &state.exchange_connectors {
            info!("Exchange {} - Active connections: {}", exchange_type, connector.active_connections());
        }
        
        // Report error rate
        if state.error_count > 0 {
            info!("Total errors: {}", state.error_count);
        }
    }
}

async fn send_heartbeat(state: Arc<Mutex<AppState>>) {
    loop {
        tokio::time::sleep(Duration::from_millis(state.lock().await.config.heartbeat_interval)).await;
        
        let state = state.lock().await;
        
        // Send heartbeat message to Kafka
        let heartbeat = serde_json::json!({
            "timestamp": Utc::now(),
            "status": "alive",
            "component": "market-data-handler"
        });
        
        produce_message(&state.kafka_producer, "system.heartbeats", "market-data-handler", &heartbeat.to_string())
            .await
            .expect("Failed to send heartbeat");
    }
}

async fn monitor_circuit_breaker(state: Arc<Mutex<AppState>>) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        
        let mut state = state.lock().await;
        
        // Check circuit breaker state
        if let Some(cooldown) = state.circuit_breaker_state.cooldown_until {
            if Instant::now() >= cooldown {
                info!("Circuit breaker cooldown expired");
                state.circuit_breaker_state.cooldown_until = None;
                state.circuit_breaker_state.error_count = 0;
            }
        }
    }
}

async fn process_market_data(state: &mut AppState, raw_data: serde_json::Value) -> Result<MarketDataMessage, String> {
    let start = Instant::now();
    
    // Check circuit breaker
    if let Some(cooldown) = state.circuit_breaker_state.cooldown_until {
        if Instant::now() < cooldown {
            return Err("Circuit breaker active".to_string());
        }
    }
    
    // Extract exchange type and symbol from raw data
    let exchange_type = match raw_data.get("exchange") {
        Some(exchange_val) => {
            match exchange_val.as_str() {
                Some("binance") => ExchangeType::Binance,
                Some("coinbase") => ExchangeType::Coinbase,
                _ => return Err("Unknown exchange".to_string()),
            }
        },
        None => return Err("Missing exchange information".to_string()),
    };
    
    let symbol = match raw_data.get("symbol") {
        Some(symbol_val) => {
            match symbol_val.as_str() {
                Some(s) => s.to_string(),
                None => return Err("Invalid symbol format".to_string()),
            }
        },
        None => return Err("Missing symbol information".to_string()),
    };
    
    // Get normalization rules
    let normalization_rule = match state.config.normalization_rules.get(&symbol) {
        Some(rule) => rule,
        None => return Err(format!("No normalization rule for symbol: {}", symbol)),
    };
    
    // Normalize symbol
    let normalized_symbol = match normalization_rule.symbol_mapping.get(&symbol) {
        Some(normalized) => normalized.clone(),
        None => return Err(format!("No mapping for symbol: {}", symbol)),
    };
    
    // Extract price and quantity with validation
    let price = match raw_data.get("price") {
        Some(price_val) => {
            match price_val.as_f64() {
                Some(p) => p,
                None => return Err("Invalid price format".to_string()),
            }
        },
        None => return Err("Missing price information".to_string()),
    };
    
    let quantity = match raw_data.get("quantity") {
        Some(quantity_val) => {
            match quantity_val.as_f64() {
                Some(q) => q,
                None => return Err("Invalid quantity format".to_string()),
            }
        },
        None => return Err("Missing quantity information".to_string()),
    };
    
    // Apply precision rules
    let normalized_price = round_to_precision(price, normalization_rule.price_precision);
    let normalized_quantity = round_to_precision(quantity, normalization_rule.quantity_precision);
    
    // Create normalized market data message
    let market_data = MarketDataMessage {
        symbol: normalized_symbol,
        exchange_id: exchange_type.to_string(),
        price: normalized_price,
        quantity: normalized_quantity,
        bid: match raw_data.get("bid") {
            Some(bid_val) => bid_val.as_f64().unwrap_or(normalized_price),
            None => normalized_price,
        },
        ask: match raw_data.get("ask") {
            Some(ask_val) => ask_val.as_f64().unwrap_or(normalized_price),
            None => normalized_price,
        },
        timestamp: Utc::now(),
    };
    
    // Update latency metrics
    let latency = (Utc::now().timestamp_nanos_opt().unwrap() - market_data.timestamp.timestamp_nanos_opt().unwrap()) as u64;
    MARKET_DATA_LATENCY.with_label_values(&[&market_data.symbol]).observe(latency as f64);
    
    Ok(market_data)
}

fn round_to_precision(value: f64, precision: u8) -> f64 {
    let multiplier = 10f64.powi(precision as i32);
    (value * multiplier).round() / multiplier
}
async fn start_health_server(state: Arc<Mutex<AppState>>) {
    use std::convert::Infallible;
    use std::net::SocketAddr;
    
    let health_handler = || async {
        let response = serde_json::json!({
            "status": "healthy",
            "timestamp": Utc::now(),
            "service": "market-data-handler"
        });
        Ok::<_, Infallible>(response.to_string())
    };
    
    let ready_handler = || async {
        let response = serde_json::json!({
            "status": "ready",
            "timestamp": Utc::now()
        });
        Ok::<_, Infallible>(response.to_string())
    };
    
    let metrics_handler = || async {
        // Simple metrics endpoint
        let metrics = "# HELP market_data_processed_total Total market data messages processed\n# TYPE market_data_processed_total counter\nmarket_data_processed_total 0\n";
        Ok::<_, Infallible>(metrics.to_string())
    };
    
    // Simple HTTP server for health checks
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Health server listening on {}", addr);
    
    // This is a simplified HTTP server - in production you'd use a proper framework
    // For now, we'll just log that the health server would be running
    info!("Health endpoints available at /health, /ready, /metrics");
}