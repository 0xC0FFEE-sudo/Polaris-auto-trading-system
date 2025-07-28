use polaris_core::kafka_utils::{create_kafka_consumer, create_kafka_producer, consume_messages, produce_message};
use polaris_core::metrics::{KAFKA_MESSAGES_CONSUMED, KAFKA_MESSAGES_PRODUCED};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeMap};
use std::time::Duration;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::FutureProducer;
use tokio::sync::Mutex;
use std::sync::Arc;
use uuid::Uuid;
use std::cmp::Ordering;

// Wrapper for f64 to implement Ord
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct OrderedFloat(f64);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

impl From<f64> for OrderedFloat {
    fn from(f: f64) -> Self {
        OrderedFloat(f)
    }
}

impl From<OrderedFloat> for f64 {
    fn from(of: OrderedFloat) -> Self {
        of.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Order {
    order_id: String,
    client_order_id: String,
    symbol: String,
    price: f64,
    quantity: f64,
    side: OrderSide,
    order_type: OrderType,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
enum OrderSide {
    Buy,
    Sell,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
enum OrderType {
    Market,
    Limit,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Trade {
    trade_id: String,
    symbol: String,
    price: f64,
    quantity: f64,
    buyer_id: String,
    seller_id: String,
    timestamp: DateTime<Utc>,
}

// Simplified order book using Vec for easier management
struct MatchingEngine {
    buy_orders: HashMap<String, BTreeMap<OrderedFloat, Vec<Order>>>,
    sell_orders: HashMap<String, BTreeMap<OrderedFloat, Vec<Order>>>,
}

impl MatchingEngine {
    fn new() -> Self {
        MatchingEngine {
            buy_orders: HashMap::new(),
            sell_orders: HashMap::new(),
        }
    }

    fn process_order(&mut self, order: Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        
        // Try to match the order first
        trades.extend(self.match_order(&order));
        
        // Add remaining quantity to order book
        if order.quantity > 0.0 {
            self.add_to_order_book(order);
        }
        
        trades
    }

    fn match_order(&mut self, incoming_order: &Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut remaining_quantity = incoming_order.quantity;
        
        let opposing_orders = match incoming_order.side {
            OrderSide::Buy => &mut self.sell_orders,
            OrderSide::Sell => &mut self.buy_orders,
        };
        
        if let Some(symbol_book) = opposing_orders.get_mut(&incoming_order.symbol) {
            let price_levels: Vec<OrderedFloat> = symbol_book.keys().cloned().collect();
            
            for price_level in price_levels {
                if remaining_quantity <= 0.0 {
                    break;
                }
                
                // Check if price matches
                let price_matches = match incoming_order.side {
                    OrderSide::Buy => price_level.0 <= incoming_order.price,
                    OrderSide::Sell => price_level.0 >= incoming_order.price,
                };
                
                if !price_matches {
                    continue;
                }
                
                if let Some(orders_at_price) = symbol_book.get_mut(&price_level) {
                    let mut orders_to_remove = Vec::new();
                    
                    for (i, existing_order) in orders_at_price.iter_mut().enumerate() {
                        if remaining_quantity <= 0.0 {
                            break;
                        }
                        
                        let trade_quantity = remaining_quantity.min(existing_order.quantity);
                        
                        // Create trade
                        let trade = Trade {
                            trade_id: Uuid::new_v4().to_string(),
                            symbol: incoming_order.symbol.clone(),
                            price: existing_order.price,
                            quantity: trade_quantity,
                            buyer_id: if incoming_order.side == OrderSide::Buy {
                                incoming_order.client_order_id.clone()
                            } else {
                                existing_order.client_order_id.clone()
                            },
                            seller_id: if incoming_order.side == OrderSide::Sell {
                                incoming_order.client_order_id.clone()
                            } else {
                                existing_order.client_order_id.clone()
                            },
                            timestamp: Utc::now(),
                        };
                        
                        trades.push(trade);
                        remaining_quantity -= trade_quantity;
                        existing_order.quantity -= trade_quantity;
                        
                        if existing_order.quantity <= 0.0 {
                            orders_to_remove.push(i);
                        }
                    }
                    
                    // Remove filled orders
                    for &i in orders_to_remove.iter().rev() {
                        orders_at_price.remove(i);
                    }
                    
                    // Remove empty price level
                    if orders_at_price.is_empty() {
                        symbol_book.remove(&price_level);
                    }
                }
            }
        }
        
        trades
    }

    fn add_to_order_book(&mut self, order: Order) {
        let book = match order.side {
            OrderSide::Buy => &mut self.buy_orders,
            OrderSide::Sell => &mut self.sell_orders,
        };
        
        let symbol_book = book.entry(order.symbol.clone())
            .or_insert_with(BTreeMap::new);
        
        let price_level = symbol_book.entry(OrderedFloat::from(order.price))
            .or_insert_with(Vec::new);
        
        price_level.push(order);
    }
}

struct AppState {
    kafka_producer: FutureProducer,
    matching_engine: Arc<Mutex<MatchingEngine>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    let kafka_brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    
    // Create Kafka consumer and producer
    let consumer: StreamConsumer = create_kafka_consumer(&kafka_brokers, "order-matching-group");
    let producer = create_kafka_producer(&kafka_brokers);
    
    consumer.subscribe(&["orders.validated"]).expect("Failed to subscribe to topic");
    
    let matching_engine = Arc::new(Mutex::new(MatchingEngine::new()));
    
    let state = Arc::new(AppState {
        kafka_producer: producer,
        matching_engine: matching_engine.clone(),
    });
    
    println!("Order matching engine started, consuming from orders.validated");
    
    // Main processing loop
    loop {
        if let Some(message) = consume_messages(&consumer, "orders.validated", Duration::from_millis(1000)).await {
            // KAFKA_MESSAGES_CONSUMED.inc();
            
            match serde_json::from_str::<Order>(&message) {
                Ok(order) => {
                    let mut engine = matching_engine.lock().await;
                    let trades = engine.process_order(order);
                    drop(engine);
                    
                    // Publish trades
                    for trade in trades {
                        let trade_json = serde_json::to_string(&trade)?;
                        produce_message(&state.kafka_producer, "trades.executed", &trade_json, "").await.ok();
                        // KAFKA_MESSAGES_PRODUCED.inc();
                    }
                }
                Err(e) => {
                    eprintln!("Failed to parse order: {}", e);
                }
            }
        }
        
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}