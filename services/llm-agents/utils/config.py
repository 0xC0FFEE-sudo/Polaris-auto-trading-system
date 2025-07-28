"""
Configuration management for LLM Agents Service
"""

import os
from typing import Dict, List, Any
from dataclasses import dataclass

@dataclass
class Config:
    """Configuration for LLM Agents Service"""
    
    # Kafka configuration
    kafka_brokers: str = "redpanda:9092"
    
    # Redis configuration
    redis_host: str = "localhost"
    redis_port: int = 6379
    
    # OpenAI configuration
    openai_api_key: str = ""
    
    # Model configuration
    model_name: str = "gpt-3.5-turbo"
    max_tokens: int = 2000
    temperature: float = 0.7
    
    # Trading configuration
    risk_tolerance: float = 0.02  # 2% max risk per trade
    max_position_size: float = 10000  # $10k max position
    position_limits: Dict[str, float] = None
    
    # Agent weights
    fact_agent_weight: float = 0.6
    subjectivity_agent_weight: float = 0.4
    
    # Risk parameters
    sharpe_threshold: float = 1.2
    max_drawdown: float = 0.05
    correlation_threshold: float = 0.7
    
    # Processing parameters
    decision_cycle_ms: int = 100  # 100ms decision cycle
    reflection_interval_minutes: int = 5
    health_check_interval_seconds: int = 30
    
    def __post_init__(self):
        """Initialize configuration from environment variables"""
        
        # Kafka
        self.kafka_brokers = os.getenv("KAFKA_BROKERS", self.kafka_brokers)
        
        # Redis
        self.redis_host = os.getenv("REDIS_HOST", self.redis_host)
        self.redis_port = int(os.getenv("REDIS_PORT", str(self.redis_port)))
        
        # OpenAI
        self.openai_api_key = os.getenv("OPENAI_API_KEY", self.openai_api_key)
        
        # Model
        self.model_name = os.getenv("MODEL_NAME", self.model_name)
        self.max_tokens = int(os.getenv("MAX_TOKENS", str(self.max_tokens)))
        self.temperature = float(os.getenv("TEMPERATURE", str(self.temperature)))
        
        # Trading
        self.risk_tolerance = float(os.getenv("RISK_TOLERANCE", str(self.risk_tolerance)))
        self.max_position_size = float(os.getenv("MAX_POSITION_SIZE", str(self.max_position_size)))
        
        # Position limits
        if self.position_limits is None:
            self.position_limits = {
                "BTC/USD": 50000,
                "ETH/USD": 25000,
                "SOL/USD": 10000
            }
        
        # Agent weights
        self.fact_agent_weight = float(os.getenv("FACT_AGENT_WEIGHT", str(self.fact_agent_weight)))
        self.subjectivity_agent_weight = float(os.getenv("SUBJECTIVITY_AGENT_WEIGHT", str(self.subjectivity_agent_weight)))
        
        # Risk parameters
        self.sharpe_threshold = float(os.getenv("SHARPE_THRESHOLD", str(self.sharpe_threshold)))
        self.max_drawdown = float(os.getenv("MAX_DRAWDOWN", str(self.max_drawdown)))
        
        # Processing
        self.decision_cycle_ms = int(os.getenv("DECISION_CYCLE_MS", str(self.decision_cycle_ms)))
        self.reflection_interval_minutes = int(os.getenv("REFLECTION_INTERVAL_MINUTES", str(self.reflection_interval_minutes)))
        self.health_check_interval_seconds = int(os.getenv("HEALTH_CHECK_INTERVAL_SECONDS", str(self.health_check_interval_seconds)))
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert config to dictionary"""
        return {
            'kafka_brokers': self.kafka_brokers,
            'redis_host': self.redis_host,
            'redis_port': self.redis_port,
            'model_name': self.model_name,
            'max_tokens': self.max_tokens,
            'temperature': self.temperature,
            'risk_tolerance': self.risk_tolerance,
            'max_position_size': self.max_position_size,
            'position_limits': self.position_limits,
            'fact_agent_weight': self.fact_agent_weight,
            'subjectivity_agent_weight': self.subjectivity_agent_weight,
            'sharpe_threshold': self.sharpe_threshold,
            'max_drawdown': self.max_drawdown,
            'decision_cycle_ms': self.decision_cycle_ms,
            'reflection_interval_minutes': self.reflection_interval_minutes,
            'health_check_interval_seconds': self.health_check_interval_seconds
        }