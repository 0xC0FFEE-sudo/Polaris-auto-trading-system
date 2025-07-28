"""
Data processing utilities for LLM Agents Service
"""

import json
import numpy as np
from datetime import datetime, timezone, timedelta
from typing import Dict, List, Any, Optional
import structlog

logger = structlog.get_logger()

class DataProcessor:
    """Utility class for processing and normalizing data"""
    
    def __init__(self, config):
        self.config = config
        self.data_cache = {}
        self.normalization_params = {}
    
    def normalize_market_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """Normalize market data from different exchanges"""
        
        normalized = {
            'symbol': self.normalize_symbol(raw_data.get('symbol', '')),
            'price': float(raw_data.get('price', 0)),
            'volume': float(raw_data.get('volume', 0)),
            'timestamp': self.normalize_timestamp(raw_data.get('timestamp')),
            'exchange': raw_data.get('exchange', 'unknown'),
            'bid': float(raw_data.get('bid', raw_data.get('price', 0))),
            'ask': float(raw_data.get('ask', raw_data.get('price', 0)))
        }
        
        # Validate data
        if not self.validate_market_data(normalized):
            logger.warning("Invalid market data", data=normalized)
            return None
        
        return normalized
    
    def normalize_symbol(self, symbol: str) -> str:
        """Normalize symbol format across exchanges"""
        
        # Common symbol mappings
        symbol_mappings = {
            'BTCUSDT': 'BTC/USD',
            'BTC-USD': 'BTC/USD',
            'ETHUSDT': 'ETH/USD',
            'ETH-USD': 'ETH/USD',
            'SOLUSDT': 'SOL/USD',
            'SOL-USD': 'SOL/USD'
        }
        
        return symbol_mappings.get(symbol.upper(), symbol.upper())
    
    def normalize_timestamp(self, timestamp: Any) -> datetime:
        """Normalize timestamp to UTC datetime"""
        
        if isinstance(timestamp, str):
            try:
                # Try ISO format first
                if 'T' in timestamp:
                    return datetime.fromisoformat(timestamp.replace('Z', '+00:00'))
                else:
                    # Try parsing as timestamp
                    return datetime.fromtimestamp(float(timestamp), tz=timezone.utc)
            except:
                return datetime.now(timezone.utc)
        
        elif isinstance(timestamp, (int, float)):
            # Unix timestamp
            if timestamp > 1e10:  # Milliseconds
                timestamp = timestamp / 1000
            return datetime.fromtimestamp(timestamp, tz=timezone.utc)
        
        elif isinstance(timestamp, datetime):
            if timestamp.tzinfo is None:
                return timestamp.replace(tzinfo=timezone.utc)
            return timestamp
        
        else:
            return datetime.now(timezone.utc)
    
    def validate_market_data(self, data: Dict[str, Any]) -> bool:
        """Validate market data quality"""
        
        # Check required fields
        required_fields = ['symbol', 'price', 'timestamp']
        for field in required_fields:
            if field not in data or data[field] is None:
                return False
        
        # Check price validity
        if data['price'] <= 0:
            return False
        
        # Check timestamp validity (not too old or in future)
        now = datetime.now(timezone.utc)
        timestamp = data['timestamp']
        
        if isinstance(timestamp, datetime):
            age = now - timestamp
            if age > timedelta(hours=1) or age < timedelta(seconds=-60):
                return False
        
        return True
    
    def aggregate_data(self, data_points: List[Dict[str, Any]], window_seconds: int = 60) -> Dict[str, Any]:
        """Aggregate data points over a time window"""
        
        if not data_points:
            return {}
        
        # Sort by timestamp
        sorted_data = sorted(data_points, key=lambda x: x['timestamp'])
        
        # Calculate aggregates
        prices = [d['price'] for d in sorted_data]
        volumes = [d.get('volume', 0) for d in sorted_data]
        
        aggregated = {
            'symbol': sorted_data[0]['symbol'],
            'open': prices[0],
            'high': max(prices),
            'low': min(prices),
            'close': prices[-1],
            'volume': sum(volumes),
            'count': len(data_points),
            'vwap': self.calculate_vwap(sorted_data),
            'timestamp_start': sorted_data[0]['timestamp'],
            'timestamp_end': sorted_data[-1]['timestamp']
        }
        
        return aggregated
    
    def calculate_vwap(self, data_points: List[Dict[str, Any]]) -> float:
        """Calculate Volume Weighted Average Price"""
        
        total_volume = 0
        total_value = 0
        
        for point in data_points:
            price = point['price']
            volume = point.get('volume', 0)
            
            total_value += price * volume
            total_volume += volume
        
        return total_value / total_volume if total_volume > 0 else 0
    
    def calculate_returns(self, prices: List[float]) -> List[float]:
        """Calculate returns from price series"""
        
        if len(prices) < 2:
            return []
        
        returns = []
        for i in range(1, len(prices)):
            if prices[i-1] > 0:
                ret = (prices[i] - prices[i-1]) / prices[i-1]
                returns.append(ret)
        
        return returns
    
    def calculate_volatility(self, returns: List[float], annualize: bool = True) -> float:
        """Calculate volatility from returns"""
        
        if len(returns) < 2:
            return 0.0
        
        volatility = np.std(returns)
        
        if annualize:
            # Assume daily returns, annualize
            volatility *= np.sqrt(365)
        
        return volatility
    
    def detect_outliers(self, values: List[float], method: str = 'iqr') -> List[bool]:
        """Detect outliers in data"""
        
        if len(values) < 4:
            return [False] * len(values)
        
        values_array = np.array(values)
        
        if method == 'iqr':
            q1 = np.percentile(values_array, 25)
            q3 = np.percentile(values_array, 75)
            iqr = q3 - q1
            
            lower_bound = q1 - 1.5 * iqr
            upper_bound = q3 + 1.5 * iqr
            
            outliers = (values_array < lower_bound) | (values_array > upper_bound)
            
        elif method == 'zscore':
            mean = np.mean(values_array)
            std = np.std(values_array)
            
            z_scores = np.abs((values_array - mean) / std) if std > 0 else np.zeros_like(values_array)
            outliers = z_scores > 3
        
        else:
            outliers = np.zeros_like(values_array, dtype=bool)
        
        return outliers.tolist()
    
    def smooth_data(self, values: List[float], method: str = 'ema', alpha: float = 0.1) -> List[float]:
        """Smooth data using various methods"""
        
        if not values:
            return []
        
        if method == 'ema':
            # Exponential Moving Average
            smoothed = [values[0]]
            for i in range(1, len(values)):
                ema = alpha * values[i] + (1 - alpha) * smoothed[-1]
                smoothed.append(ema)
            return smoothed
        
        elif method == 'sma':
            # Simple Moving Average
            window = max(1, int(1 / alpha))  # Convert alpha to window size
            smoothed = []
            for i in range(len(values)):
                start_idx = max(0, i - window + 1)
                window_values = values[start_idx:i+1]
                smoothed.append(sum(window_values) / len(window_values))
            return smoothed
        
        else:
            return values
    
    def resample_data(self, data_points: List[Dict[str, Any]], target_interval_seconds: int) -> List[Dict[str, Any]]:
        """Resample data to target interval"""
        
        if not data_points:
            return []
        
        # Sort by timestamp
        sorted_data = sorted(data_points, key=lambda x: x['timestamp'])
        
        resampled = []
        current_bucket = []
        bucket_start = sorted_data[0]['timestamp']
        
        for point in sorted_data:
            # Check if point belongs to current bucket
            time_diff = (point['timestamp'] - bucket_start).total_seconds()
            
            if time_diff < target_interval_seconds:
                current_bucket.append(point)
            else:
                # Process current bucket
                if current_bucket:
                    aggregated = self.aggregate_data(current_bucket, target_interval_seconds)
                    resampled.append(aggregated)
                
                # Start new bucket
                current_bucket = [point]
                bucket_start = point['timestamp']
        
        # Process final bucket
        if current_bucket:
            aggregated = self.aggregate_data(current_bucket, target_interval_seconds)
            resampled.append(aggregated)
        
        return resampled
    
    def calculate_correlation(self, series1: List[float], series2: List[float]) -> float:
        """Calculate correlation between two series"""
        
        if len(series1) != len(series2) or len(series1) < 2:
            return 0.0
        
        try:
            correlation = np.corrcoef(series1, series2)[0, 1]
            return correlation if not np.isnan(correlation) else 0.0
        except:
            return 0.0
    
    def normalize_features(self, features: Dict[str, float]) -> Dict[str, float]:
        """Normalize features for ML models"""
        
        normalized = {}
        
        for key, value in features.items():
            if key not in self.normalization_params:
                # Initialize normalization parameters
                self.normalization_params[key] = {
                    'mean': value,
                    'std': 1.0,
                    'min': value,
                    'max': value,
                    'count': 1
                }
                normalized[key] = 0.0
            else:
                params = self.normalization_params[key]
                
                # Update running statistics
                params['count'] += 1
                delta = value - params['mean']
                params['mean'] += delta / params['count']
                params['std'] = np.sqrt(((params['count'] - 1) * params['std']**2 + delta**2) / params['count'])
                params['min'] = min(params['min'], value)
                params['max'] = max(params['max'], value)
                
                # Z-score normalization
                if params['std'] > 0:
                    normalized[key] = (value - params['mean']) / params['std']
                else:
                    normalized[key] = 0.0
        
        return normalized