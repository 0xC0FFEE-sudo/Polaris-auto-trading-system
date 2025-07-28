use polaris_core::kafka_utils::{create_kafka_consumer, create_kafka_producer, consume_messages, produce_message};
use polaris_core::metrics::{KAFKA_MESSAGES_CONSUMED, KAFKA_MESSAGES_PRODUCED};
use chrono::{Utc, DateTime, Timelike};
use tracing::info;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use rdkafka::producer::FutureProducer;
use rdkafka::consumer::{Consumer, StreamConsumer};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use tokio::time::Instant;
use std::env;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use sqlx::{PgPool, Row};
use sha2::{Sha256, Digest};
use regex::Regex;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Transaction {
    transaction_id: String,
    user_id: String,
    symbol: String,
    amount: f64,
    transaction_type: String, // "buy", "sell", "deposit", "withdrawal"
    wallet_address: Option<String>,
    counterparty: Option<String>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ComplianceCheck {
    check_id: String,
    transaction_id: String,
    rule_name: String,
    status: String, // "passed", "failed", "flagged"
    risk_score: f64, // 0-100
    details: String,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AMLAlert {
    alert_id: String,
    transaction_id: String,
    alert_type: String,
    severity: String, // "low", "medium", "high", "critical"
    description: String,
    risk_indicators: Vec<String>,
    recommended_action: String,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct KYCRecord {
    user_id: String,
    verification_level: String, // "basic", "enhanced", "institutional"
    status: String, // "pending", "approved", "rejected", "expired"
    documents: Vec<String>,
    risk_rating: String, // "low", "medium", "high"
    last_updated: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RiskProfile {
    entity_id: String,
    entity_type: String, // "user", "wallet", "exchange"
    risk_score: f64,
    risk_factors: Vec<String>,
    sanctions_check: bool,
    pep_check: bool, // Politically Exposed Person
    adverse_media: bool,
    last_updated: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ComplianceConfig {
    kafka_brokers: String,
    input_topic: String,
    alert_topic: String,
    database_url: String,
    heartbeat_interval: u64,
    risk_thresholds: RiskThresholds,
    aml_rules: Vec<AMLRule>,
    sanctions_api_url: String,
    kyc_provider_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RiskThresholds {
    transaction_amount_high: f64,
    transaction_amount_critical: f64,
    daily_volume_limit: f64,
    risk_score_threshold: f64,
    velocity_threshold: u32, // transactions per hour
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AMLRule {
    rule_id: String,
    name: String,
    description: String,
    rule_type: String, // "amount", "velocity", "pattern", "geography"
    parameters: HashMap<String, serde_json::Value>,
    enabled: bool,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        ComplianceConfig {
            kafka_brokers: "redpanda:9092".to_string(),
            input_topic: "transactions.all".to_string(),
            alert_topic: "compliance.alerts".to_string(),
            database_url: "postgresql://compliance:compliancepass@localhost/compliancedb".to_string(),
            heartbeat_interval: 30_000,
            risk_thresholds: RiskThresholds {
                transaction_amount_high: 10000.0,
                transaction_amount_critical: 50000.0,
                daily_volume_limit: 100000.0,
                risk_score_threshold: 75.0,
                velocity_threshold: 10,
            },
            aml_rules: vec![
                AMLRule {
                    rule_id: "LARGE_TRANSACTION".to_string(),
                    name: "Large Transaction Monitoring".to_string(),
                    description: "Monitor transactions above threshold".to_string(),
                    rule_type: "amount".to_string(),
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("threshold".to_string(), serde_json::Value::Number(serde_json::Number::from(10000)));
                        params
                    },
                    enabled: true,
                },
                AMLRule {
                    rule_id: "VELOCITY_CHECK".to_string(),
                    name: "Transaction Velocity Check".to_string(),
                    description: "Monitor high frequency transactions".to_string(),
                    rule_type: "velocity".to_string(),
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("max_per_hour".to_string(), serde_json::Value::Number(serde_json::Number::from(10)));
                        params
                    },
                    enabled: true,
                }
            ],
            sanctions_api_url: "https://api.sanctions-check.com/v1/check".to_string(),
            kyc_provider_url: "https://api.kyc-provider.com/v1/verify".to_string(),
        }
    }
}

struct AppState {
    kafka_producer: FutureProducer,
    transaction_consumer: StreamConsumer,
    config: ComplianceConfig,
    db_pool: PgPool,
    user_cache: Arc<Mutex<HashMap<String, KYCRecord>>>,
    risk_cache: Arc<Mutex<HashMap<String, RiskProfile>>>,
    transaction_history: Arc<Mutex<HashMap<String, Vec<Transaction>>>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    let config = ComplianceConfig {
        kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string()),
        input_topic: env::var("INPUT_TOPIC").unwrap_or_else(|_| "transactions.all".to_string()),
        alert_topic: env::var("ALERT_TOPIC").unwrap_or_else(|_| "compliance.alerts".to_string()),
        database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://compliance:compliancepass@localhost/compliancedb".to_string()),
        heartbeat_interval: env::var("HEARTBEAT_INTERVAL")
            .map(|v| v.parse().unwrap_or(30_000))
            .unwrap_or(30_000),
        ..ComplianceConfig::default()
    };
    
    // Initialize database connection
    let db_pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");
    
    // Run database migrations (simplified for demo)
    // In production, use proper migration tools
    run_migrations(&db_pool).await.expect("Failed to run migrations");
    
    let producer = create_kafka_producer(&config.kafka_brokers);
    
    // Create transaction consumer
    let transaction_consumer = create_kafka_consumer(&config.kafka_brokers, "compliance-gateway-group");
    transaction_consumer.subscribe(&[&config.input_topic]).expect("Failed to subscribe to transactions topic");
    
    info!("Compliance gateway started");
    
    let app_state = Arc::new(AppState {
        kafka_producer: producer,
        transaction_consumer: transaction_consumer,
        config: config,
        db_pool: db_pool,
        user_cache: Arc::new(Mutex::new(HashMap::new())),
        risk_cache: Arc::new(Mutex::new(HashMap::new())),
        transaction_history: Arc::new(Mutex::new(HashMap::new())),
    });
    
    // Start background tasks
    let state_for_processing = Arc::clone(&app_state);
    tokio::spawn(async move {
        process_transactions(state_for_processing).await;
    });
    
    let state_for_heartbeat = Arc::clone(&app_state);
    tokio::spawn(async move {
        send_heartbeat(state_for_heartbeat).await;
    });
    
    // Start REST API server
    let api_state = Arc::clone(&app_state);
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/compliance/check", post(compliance_check))
        .route("/compliance/alerts", get(get_alerts))
        .route("/kyc/status", get(get_kyc_status))
        .route("/risk/profile", get(get_risk_profile))
        .layer(CorsLayer::permissive())
        .with_state(api_state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");
    
    info!("Compliance API server listening on port 8080");
    
    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

async fn process_transactions(state: Arc<AppState>) {
    info!("Starting transaction processing loop");
    
    loop {
        let start = Instant::now();
        
        if let Some(message) = consume_messages(&state.transaction_consumer, &state.config.input_topic, Duration::from_millis(100)).await {
            KAFKA_MESSAGES_CONSUMED.with_label_values(&[&state.config.input_topic]).inc();
            
            if let Ok(transaction) = serde_json::from_str::<Transaction>(&message) {
                let result = handle_transaction_compliance(&state, transaction).await;
                
                if let Ok(alerts) = result {
                    for alert in alerts {
                        let alert_json = serde_json::to_string(&alert).unwrap();
                        produce_message(&state.kafka_producer, &state.config.alert_topic, &alert.alert_id, &alert_json)
                            .await
                            .expect("Failed to produce compliance alert");
                        
                        KAFKA_MESSAGES_PRODUCED.with_label_values(&[&state.config.alert_topic]).inc();
                    }
                }
            }
        }
        
        // Small delay to prevent busy waiting
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn handle_transaction_compliance(state: &Arc<AppState>, transaction: Transaction) -> Result<Vec<AMLAlert>, String> {
    let mut alerts = Vec::new();
    
    // Store transaction in history
    {
        let mut history = state.transaction_history.lock().await;
        history.entry(transaction.user_id.clone())
            .or_insert_with(Vec::new)
            .push(transaction.clone());
        
        // Keep only last 1000 transactions per user
        if let Some(user_history) = history.get_mut(&transaction.user_id) {
            if user_history.len() > 1000 {
                user_history.drain(0..user_history.len() - 1000);
            }
        }
    }
    
    // Store transaction in database
    store_transaction(&state.db_pool, &transaction).await?;
    
    // Run AML checks
    for rule in &state.config.aml_rules {
        if rule.enabled {
            if let Some(alert) = check_aml_rule(&state, &transaction, rule).await {
                alerts.push(alert);
            }
        }
    }
    
    // KYC verification check
    if let Some(alert) = check_kyc_compliance(&state, &transaction).await {
        alerts.push(alert);
    }
    
    // Sanctions screening
    if let Some(alert) = check_sanctions(&state, &transaction).await {
        alerts.push(alert);
    }
    
    // Risk scoring
    let risk_score = calculate_risk_score(&state, &transaction).await;
    if risk_score > state.config.risk_thresholds.risk_score_threshold {
        alerts.push(AMLAlert {
            alert_id: Uuid::new_v4().to_string(),
            transaction_id: transaction.transaction_id.clone(),
            alert_type: "HIGH_RISK_SCORE".to_string(),
            severity: "high".to_string(),
            description: format!("Transaction has high risk score: {:.2}", risk_score),
            risk_indicators: vec!["high_risk_score".to_string()],
            recommended_action: "manual_review".to_string(),
            timestamp: Utc::now(),
        });
    }
    
    // Store compliance checks
    for alert in &alerts {
        store_compliance_check(&state.db_pool, &transaction, alert).await?;
    }
    
    Ok(alerts)
}

async fn check_aml_rule(state: &Arc<AppState>, transaction: &Transaction, rule: &AMLRule) -> Option<AMLAlert> {
    match rule.rule_type.as_str() {
        "amount" => {
            let threshold = rule.parameters.get("threshold")?.as_f64()?;
            if transaction.amount > threshold {
                return Some(AMLAlert {
                    alert_id: Uuid::new_v4().to_string(),
                    transaction_id: transaction.transaction_id.clone(),
                    alert_type: rule.rule_id.clone(),
                    severity: if transaction.amount > state.config.risk_thresholds.transaction_amount_critical {
                        "critical".to_string()
                    } else {
                        "high".to_string()
                    },
                    description: format!("Large transaction: ${:.2}", transaction.amount),
                    risk_indicators: vec!["large_amount".to_string()],
                    recommended_action: "enhanced_due_diligence".to_string(),
                    timestamp: Utc::now(),
                });
            }
        },
        "velocity" => {
            let max_per_hour = rule.parameters.get("max_per_hour")?.as_u64()? as usize;
            
            // Check transaction velocity
            let history = state.transaction_history.lock().await;
            if let Some(user_transactions) = history.get(&transaction.user_id) {
                let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
                let recent_count = user_transactions.iter()
                    .filter(|t| t.timestamp > one_hour_ago)
                    .count();
                
                if recent_count > max_per_hour {
                    return Some(AMLAlert {
                        alert_id: Uuid::new_v4().to_string(),
                        transaction_id: transaction.transaction_id.clone(),
                        alert_type: rule.rule_id.clone(),
                        severity: "medium".to_string(),
                        description: format!("High transaction velocity: {} transactions in last hour", recent_count),
                        risk_indicators: vec!["high_velocity".to_string()],
                        recommended_action: "temporary_limit".to_string(),
                        timestamp: Utc::now(),
                    });
                }
            }
        },
        _ => {}
    }
    
    None
}

async fn check_kyc_compliance(state: &Arc<AppState>, transaction: &Transaction) -> Option<AMLAlert> {
    // Check KYC status from cache or database
    let kyc_record = get_kyc_record(&state, &transaction.user_id).await;
    
    match kyc_record {
        Some(record) => {
            if record.status != "approved" {
                return Some(AMLAlert {
                    alert_id: Uuid::new_v4().to_string(),
                    transaction_id: transaction.transaction_id.clone(),
                    alert_type: "KYC_NOT_APPROVED".to_string(),
                    severity: "high".to_string(),
                    description: format!("User KYC status: {}", record.status),
                    risk_indicators: vec!["kyc_not_approved".to_string()],
                    recommended_action: "block_transaction".to_string(),
                    timestamp: Utc::now(),
                });
            }
            
            // Check if enhanced KYC is required for large transactions
            if transaction.amount > state.config.risk_thresholds.transaction_amount_high 
                && record.verification_level == "basic" {
                return Some(AMLAlert {
                    alert_id: Uuid::new_v4().to_string(),
                    transaction_id: transaction.transaction_id.clone(),
                    alert_type: "ENHANCED_KYC_REQUIRED".to_string(),
                    severity: "medium".to_string(),
                    description: "Enhanced KYC required for large transaction".to_string(),
                    risk_indicators: vec!["large_amount_basic_kyc".to_string()],
                    recommended_action: "request_enhanced_kyc".to_string(),
                    timestamp: Utc::now(),
                });
            }
        },
        None => {
            return Some(AMLAlert {
                alert_id: Uuid::new_v4().to_string(),
                transaction_id: transaction.transaction_id.clone(),
                alert_type: "NO_KYC_RECORD".to_string(),
                severity: "critical".to_string(),
                description: "No KYC record found for user".to_string(),
                risk_indicators: vec!["no_kyc".to_string()],
                recommended_action: "block_transaction".to_string(),
                timestamp: Utc::now(),
            });
        }
    }
    
    None
}

async fn check_sanctions(state: &Arc<AppState>, transaction: &Transaction) -> Option<AMLAlert> {
    // Check wallet address against sanctions lists
    if let Some(wallet_address) = &transaction.wallet_address {
        // Simplified sanctions check - in production would call external API
        let sanctioned_addresses = vec![
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", // Example sanctioned address
        ];
        
        if sanctioned_addresses.contains(&wallet_address.as_str()) {
            return Some(AMLAlert {
                alert_id: Uuid::new_v4().to_string(),
                transaction_id: transaction.transaction_id.clone(),
                alert_type: "SANCTIONS_HIT".to_string(),
                severity: "critical".to_string(),
                description: "Transaction involves sanctioned wallet address".to_string(),
                risk_indicators: vec!["sanctioned_address".to_string()],
                recommended_action: "block_transaction".to_string(),
                timestamp: Utc::now(),
            });
        }
    }
    
    None
}

async fn calculate_risk_score(state: &Arc<AppState>, transaction: &Transaction) -> f64 {
    let mut risk_score: f64 = 0.0;
    
    // Amount-based risk
    if transaction.amount > state.config.risk_thresholds.transaction_amount_high {
        risk_score += 30.0;
    }
    if transaction.amount > state.config.risk_thresholds.transaction_amount_critical {
        risk_score += 40.0;
    }
    
    // Velocity-based risk
    let history = state.transaction_history.lock().await;
    if let Some(user_transactions) = history.get(&transaction.user_id) {
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
        let recent_count = user_transactions.iter()
            .filter(|t| t.timestamp > one_hour_ago)
            .count();
        
        if recent_count > 5 {
            risk_score += 20.0;
        }
        if recent_count > 10 {
            risk_score += 30.0;
        }
    }
    
    // Time-based risk (unusual hours)
    let hour = transaction.timestamp.hour();
    if hour < 6 || hour > 22 {
        risk_score += 10.0;
    }
    
    // KYC-based risk
    if let Some(kyc_record) = get_kyc_record(state, &transaction.user_id).await {
        match kyc_record.risk_rating.as_str() {
            "high" => risk_score += 25.0,
            "medium" => risk_score += 10.0,
            _ => {}
        }
    } else {
        risk_score += 50.0; // No KYC record
    }
    
    risk_score.min(100.0)
}

async fn get_kyc_record(state: &Arc<AppState>, user_id: &str) -> Option<KYCRecord> {
    // Check cache first
    {
        let cache = state.user_cache.lock().await;
        if let Some(record) = cache.get(user_id) {
            return Some(record.clone());
        }
    }
    
    // Query database
    let result = sqlx::query("SELECT * FROM kyc_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.db_pool)
        .await;
    
    if let Ok(Some(row)) = result {
        let record = KYCRecord {
            user_id: row.get("user_id"),
            verification_level: row.get("verification_level"),
            status: row.get("status"),
            documents: serde_json::from_str(row.get("documents")).unwrap_or_default(),
            risk_rating: row.get("risk_rating"),
            last_updated: row.get("last_updated"),
        };
        
        // Update cache
        {
            let mut cache = state.user_cache.lock().await;
            cache.insert(user_id.to_string(), record.clone());
        }
        
        return Some(record);
    }
    
    None
}

async fn store_transaction(db_pool: &PgPool, transaction: &Transaction) -> Result<(), String> {
    let result = sqlx::query(
        "INSERT INTO transactions (transaction_id, user_id, symbol, amount, transaction_type, wallet_address, counterparty, timestamp) 
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    )
    .bind(&transaction.transaction_id)
    .bind(&transaction.user_id)
    .bind(&transaction.symbol)
    .bind(transaction.amount)
    .bind(&transaction.transaction_type)
    .bind(&transaction.wallet_address)
    .bind(&transaction.counterparty)
    .bind(transaction.timestamp)
    .execute(db_pool)
    .await;
    
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

async fn store_compliance_check(db_pool: &PgPool, transaction: &Transaction, alert: &AMLAlert) -> Result<(), String> {
    let check = ComplianceCheck {
        check_id: Uuid::new_v4().to_string(),
        transaction_id: transaction.transaction_id.clone(),
        rule_name: alert.alert_type.clone(),
        status: "flagged".to_string(),
        risk_score: 75.0, // Simplified
        details: alert.description.clone(),
        timestamp: Utc::now(),
    };
    
    let result = sqlx::query(
        "INSERT INTO compliance_checks (check_id, transaction_id, rule_name, status, risk_score, details, timestamp) 
         VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(&check.check_id)
    .bind(&check.transaction_id)
    .bind(&check.rule_name)
    .bind(&check.status)
    .bind(check.risk_score)
    .bind(&check.details)
    .bind(check.timestamp)
    .execute(db_pool)
    .await;
    
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

async fn send_heartbeat(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(Duration::from_millis(state.config.heartbeat_interval)).await;
        
        let heartbeat = serde_json::json!({
            "timestamp": Utc::now(),
            "status": "alive",
            "component": "compliance-gateway"
        });
        
        produce_message(&state.kafka_producer, "system.heartbeats", "compliance-gateway", &heartbeat.to_string())
            .await
            .expect("Failed to send heartbeat");
    }
}

// REST API handlers
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now(),
        "service": "compliance-gateway"
    }))
}

async fn compliance_check(
    State(state): State<Arc<AppState>>,
    Json(transaction): Json<Transaction>,
) -> Result<Json<Vec<AMLAlert>>, StatusCode> {
    match handle_transaction_compliance(&state, transaction).await {
        Ok(alerts) => Ok(Json(alerts)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<AMLAlert>>, StatusCode> {
    // Simplified - in production would query database
    Ok(Json(vec![]))
}

async fn get_kyc_status(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Option<KYCRecord>>, StatusCode> {
    if let Some(user_id) = params.get("user_id") {
        let record = get_kyc_record(&state, user_id).await;
        Ok(Json(record))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn get_risk_profile(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Option<RiskProfile>>, StatusCode> {
    // Simplified implementation
    Ok(Json(None))
}

async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // Create tables if they don't exist
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS transactions (
            transaction_id VARCHAR(255) PRIMARY KEY,
            user_id VARCHAR(255) NOT NULL,
            symbol VARCHAR(50) NOT NULL,
            amount DECIMAL(20, 8) NOT NULL,
            transaction_type VARCHAR(50) NOT NULL,
            wallet_address VARCHAR(255),
            counterparty VARCHAR(255),
            timestamp TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW()
        )
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS kyc_records (
            user_id VARCHAR(255) PRIMARY KEY,
            verification_level VARCHAR(50) NOT NULL DEFAULT 'basic',
            status VARCHAR(50) NOT NULL DEFAULT 'pending',
            documents JSONB DEFAULT '[]',
            risk_rating VARCHAR(50) NOT NULL DEFAULT 'medium',
            last_updated TIMESTAMPTZ DEFAULT NOW(),
            created_at TIMESTAMPTZ DEFAULT NOW()
        )
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS compliance_checks (
            check_id VARCHAR(255) PRIMARY KEY,
            transaction_id VARCHAR(255) NOT NULL,
            rule_name VARCHAR(255) NOT NULL,
            status VARCHAR(50) NOT NULL,
            risk_score DECIMAL(5, 2) NOT NULL,
            details TEXT,
            timestamp TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW()
        )
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS aml_alerts (
            alert_id VARCHAR(255) PRIMARY KEY,
            transaction_id VARCHAR(255) NOT NULL,
            alert_type VARCHAR(255) NOT NULL,
            severity VARCHAR(50) NOT NULL,
            description TEXT NOT NULL,
            risk_indicators JSONB DEFAULT '[]',
            recommended_action VARCHAR(255),
            status VARCHAR(50) DEFAULT 'open',
            assigned_to VARCHAR(255),
            resolved_at TIMESTAMPTZ,
            timestamp TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW()
        )
    "#).execute(pool).await?;

    // Insert sample KYC record for testing
    sqlx::query(r#"
        INSERT INTO kyc_records (user_id, verification_level, status, risk_rating)
        VALUES ('user1', 'basic', 'approved', 'low')
        ON CONFLICT (user_id) DO NOTHING
    "#).execute(pool).await?;

    Ok(())
}