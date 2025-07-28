use polaris_core::kafka_utils::{create_kafka_consumer, create_kafka_producer, consume_messages, produce_message};
use polaris_core::metrics::{KAFKA_MESSAGES_CONSUMED, KAFKA_MESSAGES_PRODUCED, RISK_VIOLATIONS, CIRCUIT_BREAKER_TRIPPED, POSITION_EXPOSURE};
use polaris_core::risk_engine::{RiskEngine, PositionLimit, RiskRule, ComplianceCheck};
use polaris_core::Order;
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

// Using Order from polaris_core

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RiskValidation {
    order_id: String,
    status: String, // "approved", "rejected", "modified"
    violations: Vec<String>,
    position_status: PositionStatus,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PositionStatus {
    symbol: String,
    current_position: f64,
    exposure: f64,
    pnl: f64,
    last_updated: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RiskManagerConfig {
    kafka_brokers: String,
    input_topic: String,
    approved_topic: String,
    rejected_topic: String,
    heartbeat_interval: u64,
    circuit_breaker_threshold: u64,
    position_limits: HashMap<String, PositionLimit>,
    risk_rules: Vec<RiskRule>,
    compliance_rules: Vec<ComplianceCheck>,
}

impl Default for RiskManagerConfig {
    fn default() -> Self {
        RiskManagerConfig {
            kafka_brokers: "redpanda:9092".to_string(),
            input_topic: "orders.incoming".to_string(),
            approved_topic: "orders.validated".to_string(),
            rejected_topic: "orders.rejected".to_string(),
            heartbeat_interval: 30_000,
            circuit_breaker_threshold: 5000,
            position_limits: {
                let mut limits = HashMap::new();
                limits.insert("BTC/USD".to_string(), PositionLimit {
                    max_long: 100.0,
                    max_short: 100.0,
                    max_exposure: 1_000_000.0,
                    max_daily_pnl: 50_000.0,
                    max_drawdown: 20_000.0,
                });
                limits.insert("ETH/USD".to_string(), PositionLimit {
                    max_long: 500.0,
                    max_short: 500.0,
                    max_exposure: 500_000.0,
                    max_daily_pnl: 25_000.0,
                    max_drawdown: 10_000.0,
                });
                limits
            },
            risk_rules: vec![
                RiskRule {
                    name: "max_order_size".to_string(),
                    description: "Maximum order size relative to market depth".to_string(),
                    condition: "order.quantity < market_depth.bid_size * 0.1".to_string(),
                },
                RiskRule {
                    name: "min_tick_size".to_string(),
                    description: "Order price must respect minimum tick size".to_string(),
                    condition: "order.price % tick_size == 0".to_string(),
                }
            ],
            compliance_rules: vec![
                ComplianceCheck {
                    name: "SEC Rule 611".to_string(),
                    description: "Order must comply with SEC Rule 611 (trade-through rule)".to_string(),
                    validation: "order.price must not trade through best bid/offer".to_string(),
                },
                ComplianceCheck {
                    name: "FINRA 4524".to_string(),
                    description: "Cryptocurrency transaction monitoring".to_string(),
                    validation: "wallet address must pass KYC/AML checks".to_string(),
                }
            ],
        }
    }
}

struct AppState {
    kafka_producer: FutureProducer,
    order_consumer: StreamConsumer,
    config: RiskManagerConfig,
    risk_engine: RiskEngine,
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
    
    let config = RiskManagerConfig {
        kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string()),
        input_topic: env::var("INPUT_TOPIC").unwrap_or_else(|_| "orders.incoming".to_string()),
        approved_topic: env::var("APPROVED_TOPIC").unwrap_or_else(|_| "orders.validated".to_string()),
        rejected_topic: env::var("REJECTED_TOPIC").unwrap_or_else(|_| "orders.rejected".to_string()),
        heartbeat_interval: env::var("HEARTBEAT_INTERVAL")
            .map(|v| v.parse().unwrap_or(30_000))
            .unwrap_or(30_000),
        circuit_breaker_threshold: env::var("CIRCUIT_BREAKER_THRESHOLD")
            .map(|v| v.parse().unwrap_or(5000))
            .unwrap_or(5000),
        position_limits: env::var("POSITION_LIMITS")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_default(),
        risk_rules: env::var("RISK_RULES")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_default(),
        compliance_rules: env::var("COMPLIANCE_RULES")
            .map(|v| serde_json::from_str(&v).unwrap_or_default())
            .unwrap_or_default(),
        ..RiskManagerConfig::default()
    };
    
    let producer = create_kafka_producer(&config.kafka_brokers);
    
    // Create order consumer
    let order_consumer = create_kafka_consumer(&config.kafka_brokers, "risk-manager-group");
    order_consumer.subscribe(&[&config.input_topic]).expect("Failed to subscribe to incoming orders topic");
    
    info!("Risk manager started");
    
    let app_state = Arc::new(Mutex::new(AppState {
        kafka_producer: producer,
        order_consumer: order_consumer,
        config: config,
        risk_engine: RiskEngine::new(),
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
                    let result = handle_risk_check(&mut state, order).await;
                    
                    // Send validation result to appropriate topic
                    let output_topic = match result.status.as_str() {
                        "approved" => &state.config.approved_topic,
                        _ => &state.config.rejected_topic,
                    };
                    
                    let result_json = serde_json::to_string(&result).unwrap();
                    produce_message(&state.kafka_producer, output_topic, &result.order_id, &result_json)
                        .await
                        .expect("Failed to produce risk validation result");
                    
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
        
        // Report position exposure metrics
        for (symbol, limit) in &state.config.position_limits {
            let exposure = state.risk_engine.get_position_exposure(symbol);
            POSITION_EXPOSURE.with_label_values(&[symbol]).set(exposure as f64);
            
            info!("Position - {} | Exposure: ${:.2}, PnL: ${:.2}", 
                symbol, exposure, state.risk_engine.get_position_pnl(symbol));
        }
        
        // Report error rate
        if state.error_count > 0 {
            info!("Total risk violations: {}", state.error_count);
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
            "component": "risk-manager"
        });
        
        produce_message(&state.kafka_producer, "system.heartbeats", "risk-manager", &heartbeat.to_string())
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

async fn handle_risk_check(state: &mut AppState, order: Order) -> RiskValidation {
    let start = Instant::now();
    
    // Check circuit breaker
    if let Some(cooldown) = state.circuit_breaker_state.cooldown_until {
        if Instant::now() < cooldown {
            return create_risk_validation(
                &order,
                "rejected",
                vec!["circuit_breaker_active".to_string()],
                Utc::now()
            );
        }
    }
    
    // Initialize risk engine with current config
    state.risk_engine.update_position_limits(state.config.position_limits.clone());
    state.risk_engine.add_risk_rules(state.config.risk_rules.clone());
    state.risk_engine.add_compliance_rules(state.config.compliance_rules.clone());
    
    // Validate order against all risk checks
    let mut violations = Vec::new();
    
    // Position limit checks
    if let Some(limit_violations) = state.risk_engine.check_position_limits(&order) {
        violations.extend(limit_violations);
    }
    
    // Risk rule checks
    if let Some(rule_violations) = state.risk_engine.check_risk_rules(&order) {
        violations.extend(rule_violations);
    }
    
    // Compliance checks
    if let Some(compliance_violations) = state.risk_engine.check_compliance(&order) {
        violations.extend(compliance_violations);
    }
    
    // Circuit breaker check
    if violations.len() >= state.config.circuit_breaker_threshold as usize {
        state.circuit_breaker_state = CircuitBreakerState {
            error_count: violations.len() as u64,
            cooldown_until: Some(Instant::now() + Duration::from_secs(300)),
        };
        CIRCUIT_BREAKER_TRIPPED.inc();
        
        return create_risk_validation(
            &order,
            "rejected",
            vec!["circuit_breaker_tripped".to_string()],
            Utc::now()
        );
    }
    
    // If no violations, approve order
    if violations.is_empty() {
        create_risk_validation(
            &order,
            "approved",
            vec![],
            Utc::now()
        )
    } else {
        // Record violations
        RISK_VIOLATIONS.inc_by((violations.len() as u64) as f64);
        
        create_risk_validation(
            &order,
            "rejected",
            violations,
            Utc::now()
        )
    }
}

fn create_risk_validation(order: &Order, status: &str, violations: Vec<String>, timestamp: DateTime<Utc>) -> RiskValidation {
    RiskValidation {
        order_id: order.order_id.clone(),
        status: status.to_string(),
        violations: violations,
        position_status: PositionStatus {
            symbol: order.symbol.clone(),
            current_position: 0.0, // Would be populated from risk engine
            exposure: 0.0, // Would be populated from risk engine
            pnl: 0.0, // Would be populated from risk engine
            last_updated: timestamp,
        },
        timestamp: timestamp,
    }
}
