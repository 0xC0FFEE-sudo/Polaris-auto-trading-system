use polaris_core::kafka_utils::{create_kafka_consumer, create_kafka_producer, consume_messages, produce_message};
use polaris_core::metrics::{KAFKA_MESSAGES_CONSUMED, KAFKA_MESSAGES_PRODUCED, EXECUTION_LATENCY, CONNECTION_FAILURES, CIRCUIT_BREAKER_TRIPPED};
use polaris_core::exchange_connector::{ExchangeConnector, ExchangeType, OrderExecution, FillReport, OrderStatus};
use polaris_core::order_validator::{validate_order, OrderValidationError};
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
struct Order {
    order_id: String,
    client_order_id: String,
    symbol: String,
    price: f64,
    quantity: f64,
    side: String, // "buy" or "sell"
    order_type: String, // "limit", "market", etc.
    time_in_force: String, // "GTC", "IOC", "FOK"
    user_id: String,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Fill {
    fill_id: String,
    order_id: String,
    client_order_id: String,
    symbol: String,
    price: f64,
    quantity: f64,
    side: String,
    exchange_id: String,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExecutionReport {
    order_id: String,
    client_order_id: String,
    symbol: String,
    status: String, // "filled", "partial", "rejected", etc.
    filled_quantity: f64,
    remaining_quantity: f64,
    avg_price: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ExecutionEngineConfig {
    kafka_brokers: String,
    input_topic: String,
    output_topic: String,
    heartbeat_interval: u64,
    circuit_breaker_threshold: u64,
    exchanges: Vec<ExchangeSubscription>,
    execution_timeout: u64, // milliseconds
    max_retries: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExchangeSubscription {
    exchange_type: ExchangeType,
    symbols: Vec<String>,
    api_key: String,
    api_secret: String,
    endpoint: String,
    rate_limit: u64,
}

impl Default for ExecutionEngineConfig {
    fn default() -> Self {
        ExecutionEngineConfig {
            kafka_brokers: "redpanda:9092".to_string(),
            input_topic: "orders.executable".to_string(),
            output_topic: "fills".to_string(),
            heartbeat_interval: 30_000,
            circuit_breaker_threshold: 5000,
            execution_timeout: 5000,
            max_retries: 3,
            exchanges: vec![
                ExchangeSubscription {
                    exchange_type: ExchangeType::Binance,
                    symbols: vec!["BTC/USD".to_string(), "ETH/USD".to_string()],
                    api_key: "binance_key".to_string(),
                    api_secret: "binance_secret".to_string(),
                    endpoint: "wss://binance.com/ws".to_string(),
                    rate_limit: 100,
                },
                ExchangeSubscription {
                    exchange_type: ExchangeType::Coinbase,
                    symbols: vec!["BTC/USD".to_string(), "ETH/USD".to_string()],
                    api_key: "coinbase_key".to_string(),
                    api_secret: "coinbase_secret".to_string(),
                    endpoint: "wss://coinbase.com/ws".to_string(),
                    rate_limit: 50,
                }
            ],
        }
    }
}

struct AppState {
    kafka_producer: FutureProducer,
    order_consumer: StreamConsumer,
    config: ExecutionEngineConfig,
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
    
    let config = ExecutionEngineConfig {
        kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string()),
        input_topic: env::var("INPUT_TOPIC").unwrap_or_else(|_| "orders.executable".to_string()),
        output_topic: env::var("OUTPUT_TOPIC").unwrap_or_else(|_| "fills".to_string()),
        heartbeat_interval: env::var("HEARTBEAT_INTERVAL")
            .map(|v| v.parse().unwrap_or(30_000))
            .unwrap_or(30_000),
        circuit_breaker_threshold: env::var("CIRCUIT_BREAKER_THRESHOLD")
            .map(|v| v.parse().unwrap_or(5000))
            .unwrap_or(5000),
        execution_timeout: env::var("EXECUTION_TIMEOUT")
            .map(|v| v.parse().unwrap_or(5000))
            .unwrap_or(5000),
        max_retries: env::var("MAX_RETRIES")
            .map(|v| v.parse().unwrap_or(3))
            .unwrap_or(3),
        exchanges: env::var("EXCHANGES")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_default(),
        ..ExecutionEngineConfig::default()
    };
    
    let producer = create_kafka_producer(&config.kafka_brokers);
    
    // Create order consumer
    let order_consumer = create_kafka_consumer(&config.kafka_brokers, "execution-engine-group");
    order_consumer.subscribe(&[&config.input_topic]).expect("Failed to subscribe to executable orders topic");
    
    info!("Execution engine started");
    
    // Initialize exchange connectors
    let mut exchange_connectors = HashMap::new();
    for exchange_sub in &config.exchanges {
        let connector = ExchangeConnector::new(
            exchange_sub.exchange_type.clone(),
            exchange_sub.api_key.clone(),
            exchange_sub.api_secret.clone(),
            exchange_sub.endpoint.clone(),
            exchange_sub.rate_limit,
        );
        exchange_connectors.insert(exchange_sub.exchange_type.clone(), connector);
    }
    
    let app_state = Arc::new(Mutex::new(AppState {
        kafka_producer: producer,
        order_consumer: order_consumer,
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
    
    // Main loop for processing orders
    loop {
        let start = Instant::now();
        
        // Process order messages
        {
            let mut state = app_state.lock().await;
            
            if let Some(message) = consume_messages(&state.order_consumer, &state.config.input_topic, Duration::from_millis(100)).await {
                KAFKA_MESSAGES_CONSUMED.with_label_values(&[&state.config.input_topic]).inc();
                
                if let Ok(order) = serde_json::from_str::<Order>(&message) {
                    let result = handle_execution(&mut state, order).await;
                    
                    // Send execution report
                    let output_topic = &state.config.output_topic;
                    
                    let result_json = serde_json::to_string(&result).unwrap();
                    produce_message(&state.kafka_producer, output_topic, &result.order_id, &result_json)
                        .await
                        .expect("Failed to produce execution report");
                    
                    KAFKA_MESSAGES_PRODUCED.with_label_values(&[output_topic]).inc();
                }
            }
        }
        
        // Report latency metrics
        let latency = start.elapsed().as_millis() as u64;
        // (Not implemented here but could be added)
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
            "component": "execution-engine"
        });
        
        produce_message(&state.kafka_producer, "system.heartbeats", "execution-engine", &heartbeat.to_string())
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

async fn handle_execution(state: &mut AppState, order: Order) -> ExecutionReport {
    let start = Instant::now();
    
    // Check circuit breaker
    if let Some(cooldown) = state.circuit_breaker_state.cooldown_until {
        if Instant::now() < cooldown {
            return create_execution_report(
                &order,
                "rejected",
                0.0,
                order.quantity,
                0.0,
                Utc::now(),
                "circuit_breaker_active"
            );
        }
    }
    
    // Find appropriate exchange for the symbol
    let exchange_type = match find_exchange_for_symbol(state, &order.symbol) {
        Some(exchange) => exchange,
        None => {
            return create_execution_report(
                &order,
                "rejected",
                0.0,
                order.quantity,
                0.0,
                Utc::now(),
                "no_exchange_for_symbol"
            );
        }
    };
    
    // Get exchange connector
    let connector = match state.exchange_connectors.get_mut(&exchange_type) {
        Some(connector) => connector,
        None => {
            return create_execution_report(
                &order,
                "rejected",
                0.0,
                order.quantity,
                0.0,
                Utc::now(),
                "exchange_connector_not_found"
            );
        }
    };
    
    // Execute order with retries
    let mut retries = 0;
    let execution_result = loop {
        match connector.execute_order(&order).await {
            Ok(execution) => break execution,
            Err(e) => {
                retries += 1;
                if retries > state.config.max_retries {
                    return create_execution_report(
                        &order,
                        "rejected",
                        0.0,
                        order.quantity,
                        0.0,
                        Utc::now(),
                        &format!("execution_failed: {}", e)
                    );
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    };
    
    // Create fill report
    let fill = Fill {
        fill_id: Uuid::new_v4().to_string(),
        order_id: order.order_id.clone(),
        client_order_id: order.client_order_id.clone(),
        symbol: order.symbol.clone(),
        price: execution_result.price,
        quantity: execution_result.quantity,
        side: order.side.clone(),
        exchange_id: exchange_type.to_string(),
        timestamp: Utc::now(),
    };
    
    // Update latency metrics
    let latency = (Utc::now().timestamp_nanos_opt().unwrap() - order.timestamp.timestamp_nanos_opt().unwrap()) as u64;
    EXECUTION_LATENCY.with_label_values(&[&order.symbol]).observe(latency as f64);
    
    create_execution_report(
        &order,
        match execution_result.status {
            OrderStatus::Filled => "filled",
            OrderStatus::Partial => "partial",
            _ => "rejected",
        },
        execution_result.quantity,
        order.quantity - execution_result.quantity,
        execution_result.price,
        Utc::now(),
        "execution_complete"
    )
}

fn create_execution_report(order: &Order, status: &str, filled_quantity: f64, remaining_quantity: f64, avg_price: f64, timestamp: DateTime<Utc>, reason: &str) -> ExecutionReport {
    ExecutionReport {
        order_id: order.order_id.clone(),
        client_order_id: order.client_order_id.clone(),
        symbol: order.symbol.clone(),
        status: status.to_string(),
        filled_quantity: filled_quantity,
        remaining_quantity: remaining_quantity,
        avg_price: avg_price,
        timestamp: timestamp,
    }
}

fn find_exchange_for_symbol(state: &AppState, symbol: &str) -> Option<ExchangeType> {
    for exchange_sub in &state.config.exchanges {
        if exchange_sub.symbols.contains(&symbol.to_string()) {
            return Some(exchange_sub.exchange_type.clone());
        }
    }
    None
}
