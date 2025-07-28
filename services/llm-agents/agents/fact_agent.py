"""
Fact Agent - Processes quantitative/numerical data for trading decisions
"""

import asyncio
import json
import numpy as np
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

import structlog
from transformers import pipeline

logger = structlog.get_logger()

@dataclass
class FactAnalysis:
    """Result of fact-based analysis"""
    price_trend: str  # "bullish", "bearish", "neutral"
    volume_analysis: Dict[str, float]
    technical_indicators: Dict[str, float]
    market_structure: Dict[str, Any]
    confidence: float
    reasoning: List[str]

class FactAgent:
    """Agent responsible for analyzing quantitative market data"""
    
    def __init__(self, config):
        self.config = config
        self.price_history = {}  # Symbol -> price history
        self.volume_history = {}  # Symbol -> volume history
        
        # Initialize technical analysis tools
        self.setup_technical_analysis()
    
    def setup_technical_analysis(self):
        """Setup technical analysis indicators"""
        self.indicators = {
            'sma_periods': [5, 10, 20, 50],
            'ema_periods': [12, 26],
            'rsi_period': 14,
            'bollinger_period': 20,
            'macd_fast': 12,
            'macd_slow': 26,
            'macd_signal': 9
        }
    
    async def analyze(self, signal) -> FactAnalysis:
        """Analyze market signal using quantitative methods"""
        start_time = datetime.now()
        
        try:
            # Update price and volume history
            self.update_history(signal)
            
            # Perform technical analysis
            price_trend = self.analyze_price_trend(signal.symbol)
            volume_analysis = self.analyze_volume(signal.symbol)
            technical_indicators = self.calculate_technical_indicators(signal.symbol)
            market_structure = self.analyze_market_structure(signal)
            
            # Calculate overall confidence
            confidence = self.calculate_confidence(
                price_trend, volume_analysis, technical_indicators
            )
            
            # Generate reasoning
            reasoning = self.generate_reasoning(
                signal, price_trend, volume_analysis, technical_indicators
            )
            
            analysis = FactAnalysis(
                price_trend=price_trend,
                volume_analysis=volume_analysis,
                technical_indicators=technical_indicators,
                market_structure=market_structure,
                confidence=confidence,
                reasoning=reasoning
            )
            
            processing_time = (datetime.now() - start_time).total_seconds()
            logger.info(
                "Fact analysis completed",
                symbol=signal.symbol,
                price_trend=price_trend,
                confidence=confidence,
                processing_time=processing_time
            )
            
            return analysis
            
        except Exception as e:
            logger.error("Error in fact analysis", symbol=signal.symbol, error=str(e))
            return self.create_default_analysis()
    
    def update_history(self, signal):
        """Update price and volume history"""
        symbol = signal.symbol
        
        if symbol not in self.price_history:
            self.price_history[symbol] = []
            self.volume_history[symbol] = []
        
        # Add new data point
        self.price_history[symbol].append({
            'price': signal.price,
            'timestamp': signal.timestamp
        })
        
        self.volume_history[symbol].append({
            'volume': signal.volume,
            'timestamp': signal.timestamp
        })
        
        # Keep only last 200 data points
        self.price_history[symbol] = self.price_history[symbol][-200:]
        self.volume_history[symbol] = self.volume_history[symbol][-200:]
    
    def analyze_price_trend(self, symbol: str) -> str:
        """Analyze price trend using multiple timeframes"""
        if symbol not in self.price_history or len(self.price_history[symbol]) < 10:
            return "neutral"
        
        prices = [p['price'] for p in self.price_history[symbol]]
        
        # Short-term trend (last 5 periods)
        short_trend = self.calculate_trend(prices[-5:])
        
        # Medium-term trend (last 20 periods)
        medium_trend = self.calculate_trend(prices[-20:]) if len(prices) >= 20 else 0
        
        # Long-term trend (last 50 periods)
        long_trend = self.calculate_trend(prices[-50:]) if len(prices) >= 50 else 0
        
        # Weighted trend analysis
        trend_score = (short_trend * 0.5) + (medium_trend * 0.3) + (long_trend * 0.2)
        
        if trend_score > 0.02:
            return "bullish"
        elif trend_score < -0.02:
            return "bearish"
        else:
            return "neutral"
    
    def calculate_trend(self, prices: List[float]) -> float:
        """Calculate trend using linear regression"""
        if len(prices) < 2:
            return 0.0
        
        x = np.arange(len(prices))
        y = np.array(prices)
        
        # Linear regression
        slope, _ = np.polyfit(x, y, 1)
        
        # Normalize by average price
        avg_price = np.mean(y)
        return slope / avg_price if avg_price > 0 else 0.0
    
    def analyze_volume(self, symbol: str) -> Dict[str, float]:
        """Analyze volume patterns"""
        if symbol not in self.volume_history or len(self.volume_history[symbol]) < 5:
            return {"average_volume": 0.0, "volume_trend": 0.0, "volume_spike": False}
        
        volumes = [v['volume'] for v in self.volume_history[symbol]]
        
        # Calculate volume metrics
        avg_volume = np.mean(volumes)
        recent_volume = volumes[-1]
        volume_trend = self.calculate_trend(volumes[-10:]) if len(volumes) >= 10 else 0.0
        
        # Detect volume spikes
        volume_spike = recent_volume > (avg_volume * 2) if avg_volume > 0 else False
        
        return {
            "average_volume": avg_volume,
            "current_volume": recent_volume,
            "volume_trend": volume_trend,
            "volume_spike": volume_spike,
            "volume_ratio": recent_volume / avg_volume if avg_volume > 0 else 1.0
        }
    
    def calculate_technical_indicators(self, symbol: str) -> Dict[str, float]:
        """Calculate technical indicators"""
        if symbol not in self.price_history or len(self.price_history[symbol]) < 20:
            return {}
        
        prices = [p['price'] for p in self.price_history[symbol]]
        indicators = {}
        
        # Simple Moving Averages
        for period in self.indicators['sma_periods']:
            if len(prices) >= period:
                sma = np.mean(prices[-period:])
                indicators[f'sma_{period}'] = sma
        
        # RSI
        if len(prices) >= self.indicators['rsi_period']:
            rsi = self.calculate_rsi(prices, self.indicators['rsi_period'])
            indicators['rsi'] = rsi
        
        # Bollinger Bands
        if len(prices) >= self.indicators['bollinger_period']:
            bb_upper, bb_middle, bb_lower = self.calculate_bollinger_bands(
                prices, self.indicators['bollinger_period']
            )
            indicators['bb_upper'] = bb_upper
            indicators['bb_middle'] = bb_middle
            indicators['bb_lower'] = bb_lower
            indicators['bb_position'] = (prices[-1] - bb_lower) / (bb_upper - bb_lower) if bb_upper != bb_lower else 0.5
        
        # MACD
        if len(prices) >= max(self.indicators['macd_fast'], self.indicators['macd_slow']):
            macd_line, signal_line = self.calculate_macd(prices)
            indicators['macd'] = macd_line
            indicators['macd_signal'] = signal_line
            indicators['macd_histogram'] = macd_line - signal_line
        
        return indicators
    
    def calculate_rsi(self, prices: List[float], period: int) -> float:
        """Calculate Relative Strength Index"""
        if len(prices) < period + 1:
            return 50.0
        
        deltas = np.diff(prices)
        gains = np.where(deltas > 0, deltas, 0)
        losses = np.where(deltas < 0, -deltas, 0)
        
        avg_gain = np.mean(gains[-period:])
        avg_loss = np.mean(losses[-period:])
        
        if avg_loss == 0:
            return 100.0
        
        rs = avg_gain / avg_loss
        rsi = 100 - (100 / (1 + rs))
        
        return rsi
    
    def calculate_bollinger_bands(self, prices: List[float], period: int, std_dev: float = 2.0):
        """Calculate Bollinger Bands"""
        if len(prices) < period:
            return prices[-1], prices[-1], prices[-1]
        
        sma = np.mean(prices[-period:])
        std = np.std(prices[-period:])
        
        upper = sma + (std_dev * std)
        lower = sma - (std_dev * std)
        
        return upper, sma, lower
    
    def calculate_macd(self, prices: List[float]):
        """Calculate MACD"""
        if len(prices) < self.indicators['macd_slow']:
            return 0.0, 0.0
        
        # Calculate EMAs
        ema_fast = self.calculate_ema(prices, self.indicators['macd_fast'])
        ema_slow = self.calculate_ema(prices, self.indicators['macd_slow'])
        
        macd_line = ema_fast - ema_slow
        
        # Signal line (EMA of MACD)
        # Simplified - in production would maintain MACD history
        signal_line = macd_line * 0.9  # Approximation
        
        return macd_line, signal_line
    
    def calculate_ema(self, prices: List[float], period: int) -> float:
        """Calculate Exponential Moving Average"""
        if len(prices) < period:
            return np.mean(prices)
        
        multiplier = 2 / (period + 1)
        ema = prices[0]
        
        for price in prices[1:]:
            ema = (price * multiplier) + (ema * (1 - multiplier))
        
        return ema
    
    def analyze_market_structure(self, signal) -> Dict[str, Any]:
        """Analyze market microstructure"""
        return {
            "bid_ask_spread": 0.001,  # Placeholder
            "market_depth": signal.volume,
            "price_impact": 0.0001,  # Placeholder
            "liquidity_score": min(signal.volume / 1000, 1.0),
            "volatility": self.calculate_volatility(signal.symbol)
        }
    
    def calculate_volatility(self, symbol: str) -> float:
        """Calculate price volatility"""
        if symbol not in self.price_history or len(self.price_history[symbol]) < 10:
            return 0.0
        
        prices = [p['price'] for p in self.price_history[symbol][-20:]]
        returns = np.diff(np.log(prices))
        
        return np.std(returns) * np.sqrt(24 * 365)  # Annualized volatility
    
    def calculate_confidence(self, price_trend: str, volume_analysis: Dict, indicators: Dict) -> float:
        """Calculate confidence in the analysis"""
        confidence = 0.5  # Base confidence
        
        # Adjust based on trend strength
        if price_trend in ["bullish", "bearish"]:
            confidence += 0.2
        
        # Adjust based on volume confirmation
        if volume_analysis.get("volume_spike", False):
            confidence += 0.1
        
        # Adjust based on technical indicators alignment
        rsi = indicators.get("rsi", 50)
        if (price_trend == "bullish" and rsi < 70) or (price_trend == "bearish" and rsi > 30):
            confidence += 0.1
        
        # Adjust based on data quality
        data_points = len(self.price_history.get(signal.symbol if hasattr(self, 'signal') else 'default', []))
        if data_points > 50:
            confidence += 0.1
        
        return min(confidence, 1.0)
    
    def generate_reasoning(self, signal, price_trend: str, volume_analysis: Dict, indicators: Dict) -> List[str]:
        """Generate human-readable reasoning"""
        reasoning = []
        
        # Price trend reasoning
        reasoning.append(f"Price trend analysis shows {price_trend} momentum")
        
        # Volume reasoning
        if volume_analysis.get("volume_spike", False):
            reasoning.append("Significant volume spike detected, indicating strong interest")
        
        # Technical indicator reasoning
        rsi = indicators.get("rsi")
        if rsi:
            if rsi > 70:
                reasoning.append(f"RSI at {rsi:.1f} indicates overbought conditions")
            elif rsi < 30:
                reasoning.append(f"RSI at {rsi:.1f} indicates oversold conditions")
            else:
                reasoning.append(f"RSI at {rsi:.1f} shows neutral momentum")
        
        # Bollinger Bands reasoning
        bb_position = indicators.get("bb_position")
        if bb_position:
            if bb_position > 0.8:
                reasoning.append("Price near upper Bollinger Band, potential resistance")
            elif bb_position < 0.2:
                reasoning.append("Price near lower Bollinger Band, potential support")
        
        return reasoning
    
    def create_default_analysis(self) -> FactAnalysis:
        """Create default analysis when errors occur"""
        return FactAnalysis(
            price_trend="neutral",
            volume_analysis={"average_volume": 0.0, "volume_trend": 0.0, "volume_spike": False},
            technical_indicators={},
            market_structure={},
            confidence=0.1,
            reasoning=["Analysis failed, using default neutral stance"]
        )