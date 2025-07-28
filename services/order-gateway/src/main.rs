use polaris_core::kafka_utils::{create_kafka_consumer, create_kafka_producer, consume_messages, produce_message};
use polaris_core::metrics::{KAFKA_MESSAGES_CONSUMED, KAFKA_MESSAGES_PRODUCED, ORDER_LATENCY, CONNECTION_FAILURES, CIRCUIT_BREAKER_TRIPPED};
use polaris_core::auth::{authenticate, authorize, ApiKeyAuth};
use polaris_core::rate_limiter::{RateLimiter, RateLimitExceeded};
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
struct OrderRequest {
    client_order_id: String,
    symbol: String,
    price: f64,
    quantity: f64,
    side: String,
    order_type: String,
    time_in_force: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OrderResponse {
    order_id: String,
    client_order_id: String,
    status: String,
    reason: String,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OrderGatewayConfig {
    kafka_brokers: String,
    input_topic: String,
    output_topic: String,
    heartbeat_interval: u64,
    circuit_breaker_threshold: u64,
    rate_limit: u64,
    auth_type: String,
    api_keys: HashMap<String, ApiKeyAuth>,
}

impl Default for OrderGatewayConfig {
    fn default() -> Self {
        OrderGatewayConfig {
            kafka_brokers: "redpanda:9092".to_string(),
            input_topic: "orders.client".to_string(),
            output_topic: "orders.incoming".to_string(),
            heartbeat_interval: 30_000,
            circuit_breaker_threshold: 5000,
            rate_limit: 1000,
            auth_type: "api_key".to_string(),
            api_keys: {
                let mut keys = HashMap::new();
                keys.insert("user1".to_string(), ApiKeyAuth {
                    key: "user1".to_string(),
                    secret: "secret1".to_string(),
                    permissions: vec!["trade".to_string()],
                });
                keys
            },
        }
    }
}

struct AppState {
    kafka_producer: FutureProducer,
    order_consumer: StreamConsumer,
    config: OrderGatewayConfig,
    rate_limiter: RateLimiter,
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
    
    let config = OrderGatewayConfig {
        kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string()),
        input_topic: env::var("INPUT_TOPIC").unwrap_or_else(|_| "orders.client".to_string()),
        output_topic: env::var("OUTPUT_TOPIC").unwrap_or_else(|_| "orders.incoming".to_string()),
        heartbeat_interval: env::var("HEARTBEAT_INTERVAL")
            .map(|v| v.parse().unwrap_or(30_000))
            .unwrap_or(30_000),
        circuit_breaker_threshold: env::var("CIRCUIT_BREAKER_THRESHOLD")
            .map(|v| v.parse().unwrap_or(5000))
            .unwrap_or(5000),
        rate_limit: env::var("RATE_LIMIT")
            .map(|v| v.parse().unwrap_or(1000))
            .unwrap_or(1000),
        auth_type: env::var("AUTH_TYPE")
            .unwrap_or_else(|_| "api_key".to_string()),
        api_keys: env::var("API_KEYS")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_else(|_| {
                let mut keys = HashMap::new();
                keys.insert("user1".to_string(), ApiKeyAuth {
                    key: "user1".to_string(),
                    secret: "secret1".to_string(),
                    permissions: vec!["trade".to_string()],
                });
                keys
            }),
        ..OrderGatewayConfig::default()
    };
    
    let producer = create_kafka_producer(&config.kafka_brokers);
    
    // Create order consumer
    let order_consumer = create_kafka_consumer(&config.kafka_brokers, "order-gateway-group");
    order_consumer.subscribe(&[&config.input_topic]).expect("Failed to subscribe to client orders topic");
    
    info!("Order gateway started");
    
    let app_state = Arc::new(Mutex::new(AppState {
        kafka_producer: producer,
        order_consumer: order_consumer,
        config: config.clone(),
        rate_limiter: RateLimiter::new(config.rate_limit),
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
                
                if let Ok(order_request) = serde_json::from_str::<OrderRequest>(&message) {
                    let result = handle_order(&mut state, order_request).await;
                    
                    // Send result to appropriate topic
                    let output_topic = &state.config.output_topic;
                    
                    let result_json = serde_json::to_string(&result).unwrap();
                    produce_message(&state.kafka_producer, output_topic, &result.order_id, &result_json)
                        .await
                        .expect("Failed to produce order response");
                    
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
        
        // Report rate limiting stats
        info!("Rate limit - Current QPS: {}", state.rate_limiter.get_current_qps());
        
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
            "component": "order-gateway"
        });
        
        produce_message(&state.kafka_producer, "system.heartbeats", "order-gateway", &heartbeat.to_string())
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

async fn handle_order(state: &mut AppState, order_request: OrderRequest) -> OrderResponse {
    let start = Instant::now();
    
    // Check circuit breaker
    if let Some(cooldown) = state.circuit_breaker_state.cooldown_until {
        if Instant::now() < cooldown {
            return create_order_response(
                "",
                "rejected",
                "circuit_breaker_active",
                Utc::now()
            );
        }
    }
    
    // Convert OrderRequest to Order for validation
    let order_for_validation = polaris_core::Order {
        order_id: Uuid::new_v4().to_string(),
        client_order_id: order_request.client_order_id.clone(),
        symbol: order_request.symbol.clone(),
        price: order_request.price,
        quantity: order_request.quantity,
        side: order_request.side.clone(),
        order_type: order_request.order_type.clone(),
        time_in_force: order_request.time_in_force.clone(),
        user_id: "".to_string(),
        timestamp: Utc::now(),
    };
    
    // Validate request
    let validation_result = validate_order(&order_for_validation);
    if let Err(e) = validation_result {
        return create_order_response(
            "",
            "rejected",
            &format!("validation_error: {:?}", e),
            Utc::now()
        );
    }
    
    // Check rate limiting
    if let Err(RateLimitExceeded) = state.rate_limiter.check() {
        return create_order_response(
            "",
            "rejected",
            "rate_limit_exceeded",
            Utc::now()
        );
    }
    
    // Create order with unique ID
    let order_id = Uuid::new_v4().to_string();
    let client_order_id = order_request.client_order_id.clone();
    
    let order = Order {
        order_id: order_id.clone(),
        client_order_id: client_order_id.clone(),
        symbol: order_request.symbol,
        price: order_request.price,
        quantity: order_request.quantity,
        side: order_request.side,
        order_type: order_request.order_type,
        time_in_force: order_request.time_in_force,
        user_id: "user1".to_string(), // In production, this would come from authentication
        timestamp: Utc::now(),
    };
    
    // Update latency metrics
    let latency = (Utc::now().timestamp_nanos_opt().unwrap() - order.timestamp.timestamp_nanos_opt().unwrap()) as u64;
    ORDER_LATENCY.with_label_values(&[&order.symbol]).observe(latency as f64);
    
    create_order_response(
        &order_id,
        "accepted",
        "order_received",
        Utc::now()
    )
}

fn create_order_response(order_id: &str, status: &str, reason: &str, timestamp: DateTime<Utc>) -> OrderResponse {
    OrderResponse {
        order_id: order_id.to_string(),
        client_order_id: Uuid::new_v4().to_string(),
        status: status.to_string(),
        reason: reason.to_string(),
        timestamp: timestamp,
    }
}
