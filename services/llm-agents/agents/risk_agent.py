"""
Risk Agent - Makes final trading decisions using Bayesian voting and risk assessment
"""

import asyncio
import json
import uuid
from datetime import datetime, timezone
from typing import Dict, List, Any, Optional
from dataclasses import dataclass
import numpy as np

import structlog

logger = structlog.get_logger()

@dataclass
class TradingDecision:
    """Final trading decision"""
    decision_id: str
    symbol: str
    action: str  # "buy", "sell", "hold"
    confidence: float
    quantity: float
    price_target: float
    reasoning: Dict[str, Any]
    risk_assessment: Dict[str, Any]
    timestamp: datetime

class RiskAgent:
    """Agent responsible for final trading decisions and risk management"""
    
    def __init__(self, config):
        self.config = config
        self.position_limits = getattr(config, 'position_limits', {})
        self.risk_tolerance = getattr(config, 'risk_tolerance', 0.02)  # 2% max risk per trade
        self.max_position_size = getattr(config, 'max_position_size', 10000)  # $10k max position
        
        # Bayesian voting weights
        self.fact_weight = 0.6
        self.subjectivity_weight = 0.4
        
        # Risk parameters
        self.sharpe_threshold = 1.2  # Minimum Sharpe ratio for trade execution
        self.max_drawdown = 0.05  # 5% max drawdown
        self.correlation_threshold = 0.7  # Max correlation with existing positions
    
    async def decide(self, signal, fact_analysis, subjectivity_analysis) -> Optional[TradingDecision]:
        """Make final trading decision using Bayesian voting"""
        start_time = datetime.now()
        
        try:
            # Step 1: Bayesian voting to combine agent outputs
            combined_signal = self.bayesian_vote(fact_analysis, subjectivity_analysis)
            
            # Step 2: Risk assessment
            risk_assessment = await self.assess_risk(signal, combined_signal)
            
            # Step 3: Position sizing
            position_size = self.calculate_position_size(signal, risk_assessment)
            
            # Step 4: Final decision logic
            decision = await self.make_final_decision(
                signal, combined_signal, risk_assessment, position_size
            )
            
            if decision:
                processing_time = (datetime.now() - start_time).total_seconds()
                logger.info(
                    "Risk agent decision made",
                    symbol=signal.symbol,
                    action=decision.action,
                    confidence=decision.confidence,
                    quantity=decision.quantity,
                    processing_time=processing_time
                )
            
            return decision
            
        except Exception as e:
            logger.error("Error in risk agent decision", symbol=signal.symbol, error=str(e))
            return None
    
    def bayesian_vote(self, fact_analysis, subjectivity_analysis) -> Dict[str, Any]:
        """Combine agent outputs using Bayesian voting"""
        
        # Extract signals from each agent
        fact_signal = self.extract_fact_signal(fact_analysis)
        subjectivity_signal = self.extract_subjectivity_signal(subjectivity_analysis)
        
        # Bayesian combination
        combined_confidence = (
            fact_analysis.confidence * self.fact_weight +
            subjectivity_analysis.confidence * self.subjectivity_weight
        )
        
        # Combine action probabilities
        fact_probs = fact_signal['action_probabilities']
        subj_probs = subjectivity_signal['action_probabilities']
        
        combined_probs = {}
        for action in ['buy', 'sell', 'hold']:
            # Bayesian update
            prior = 1/3  # Uniform prior
            fact_likelihood = fact_probs.get(action, prior)
            subj_likelihood = subj_probs.get(action, prior)
            
            # Weighted combination
            combined_likelihood = (
                fact_likelihood * self.fact_weight +
                subj_likelihood * self.subjectivity_weight
            )
            
            combined_probs[action] = combined_likelihood
        
        # Normalize probabilities
        total_prob = sum(combined_probs.values())
        if total_prob > 0:
            combined_probs = {k: v/total_prob for k, v in combined_probs.items()}
        
        # Determine dominant action
        dominant_action = max(combined_probs.items(), key=lambda x: x[1])
        
        return {
            'action': dominant_action[0],
            'action_probabilities': combined_probs,
            'confidence': combined_confidence,
            'fact_signal': fact_signal,
            'subjectivity_signal': subjectivity_signal,
            'reasoning': {
                'fact_reasoning': fact_analysis.reasoning,
                'subjectivity_reasoning': subjectivity_analysis.reasoning
            }
        }
    
    def extract_fact_signal(self, fact_analysis) -> Dict[str, Any]:
        """Extract trading signal from fact analysis"""
        
        # Convert price trend to action probabilities
        if fact_analysis.price_trend == "bullish":
            action_probs = {"buy": 0.7, "hold": 0.2, "sell": 0.1}
        elif fact_analysis.price_trend == "bearish":
            action_probs = {"buy": 0.1, "hold": 0.2, "sell": 0.7}
        else:
            action_probs = {"buy": 0.3, "hold": 0.4, "sell": 0.3}
        
        # Adjust based on technical indicators
        rsi = fact_analysis.technical_indicators.get('rsi', 50)
        if rsi > 70:  # Overbought
            action_probs['sell'] += 0.1
            action_probs['buy'] -= 0.1
        elif rsi < 30:  # Oversold
            action_probs['buy'] += 0.1
            action_probs['sell'] -= 0.1
        
        # Adjust based on volume
        if fact_analysis.volume_analysis.get('volume_spike', False):
            # Volume confirms trend
            if fact_analysis.price_trend == "bullish":
                action_probs['buy'] += 0.1
            elif fact_analysis.price_trend == "bearish":
                action_probs['sell'] += 0.1
        
        # Normalize
        total = sum(action_probs.values())
        if total > 0:
            action_probs = {k: v/total for k, v in action_probs.items()}
        
        return {
            'action_probabilities': action_probs,
            'strength': fact_analysis.confidence,
            'technical_score': self.calculate_technical_score(fact_analysis)
        }
    
    def extract_subjectivity_signal(self, subjectivity_analysis) -> Dict[str, Any]:
        """Extract trading signal from subjectivity analysis"""
        
        sentiment = subjectivity_analysis.sentiment_score
        
        # Convert sentiment to action probabilities
        if sentiment > 0.3:
            action_probs = {"buy": 0.6, "hold": 0.3, "sell": 0.1}
        elif sentiment < -0.3:
            action_probs = {"buy": 0.1, "hold": 0.3, "sell": 0.6}
        else:
            action_probs = {"buy": 0.3, "hold": 0.4, "sell": 0.3}
        
        # Adjust based on market psychology
        fear_greed = subjectivity_analysis.market_psychology.get('fear_greed_index', 50)
        if fear_greed > 75:  # Extreme greed
            action_probs['sell'] += 0.2  # Contrarian
            action_probs['buy'] -= 0.2
        elif fear_greed < 25:  # Extreme fear
            action_probs['buy'] += 0.2  # Contrarian
            action_probs['sell'] -= 0.2
        
        # Adjust based on dominant emotions
        emotions = subjectivity_analysis.emotion_analysis
        if emotions.get('fear', 0) > 0.5:
            action_probs['sell'] += 0.1
        elif emotions.get('greed', 0) > 0.5:
            action_probs['buy'] += 0.1
        
        # Normalize
        total = sum(action_probs.values())
        if total > 0:
            action_probs = {k: v/total for k, v in action_probs.items()}
        
        return {
            'action_probabilities': action_probs,
            'strength': subjectivity_analysis.confidence,
            'sentiment_score': sentiment
        }
    
    def calculate_technical_score(self, fact_analysis) -> float:
        """Calculate overall technical analysis score"""
        score = 0.0
        
        # Price trend score
        if fact_analysis.price_trend == "bullish":
            score += 0.3
        elif fact_analysis.price_trend == "bearish":
            score -= 0.3
        
        # RSI score
        rsi = fact_analysis.technical_indicators.get('rsi', 50)
        if 30 < rsi < 70:  # Healthy range
            score += 0.1
        elif rsi < 30:  # Oversold (potential buy)
            score += 0.2
        elif rsi > 70:  # Overbought (potential sell)
            score -= 0.2
        
        # Volume confirmation
        if fact_analysis.volume_analysis.get('volume_spike', False):
            score += 0.1
        
        # Bollinger Bands position
        bb_position = fact_analysis.technical_indicators.get('bb_position', 0.5)
        if bb_position < 0.2:  # Near lower band
            score += 0.1
        elif bb_position > 0.8:  # Near upper band
            score -= 0.1
        
        return max(-1.0, min(1.0, score))
    
    async def assess_risk(self, signal, combined_signal) -> Dict[str, Any]:
        """Comprehensive risk assessment"""
        
        risk_assessment = {
            'overall_risk': 'medium',
            'risk_score': 0.5,  # 0-1 scale
            'risk_factors': [],
            'risk_mitigation': [],
            'sharpe_estimate': 0.0,
            'var_estimate': 0.0,  # Value at Risk
            'max_loss_estimate': 0.0
        }
        
        # Market risk assessment
        volatility = signal.on_chain_activity.get('volatility', 0.0)
        if volatility > 0.5:
            risk_assessment['risk_factors'].append('High market volatility')
            risk_assessment['risk_score'] += 0.2
        
        # Liquidity risk
        if signal.volume < 1000:
            risk_assessment['risk_factors'].append('Low liquidity')
            risk_assessment['risk_score'] += 0.1
        
        # Sentiment risk
        sentiment_score = combined_signal['subjectivity_signal']['sentiment_score']
        if abs(sentiment_score) > 0.8:
            risk_assessment['risk_factors'].append('Extreme sentiment levels')
            risk_assessment['risk_score'] += 0.1
        
        # Technical risk
        technical_score = combined_signal['fact_signal']['technical_score']
        if abs(technical_score) < 0.1:
            risk_assessment['risk_factors'].append('Weak technical signals')
            risk_assessment['risk_score'] += 0.1
        
        # Confidence risk
        if combined_signal['confidence'] < 0.5:
            risk_assessment['risk_factors'].append('Low confidence in analysis')
            risk_assessment['risk_score'] += 0.2
        
        # Calculate Sharpe ratio estimate
        expected_return = abs(technical_score) * 0.1  # Simplified
        risk_free_rate = 0.02  # 2% risk-free rate
        sharpe_estimate = (expected_return - risk_free_rate) / max(volatility, 0.01)
        risk_assessment['sharpe_estimate'] = sharpe_estimate
        
        # Overall risk classification
        if risk_assessment['risk_score'] > 0.7:
            risk_assessment['overall_risk'] = 'high'
        elif risk_assessment['risk_score'] < 0.3:
            risk_assessment['overall_risk'] = 'low'
        
        # Risk mitigation strategies
        if volatility > 0.5:
            risk_assessment['risk_mitigation'].append('Reduce position size due to volatility')
        if signal.volume < 1000:
            risk_assessment['risk_mitigation'].append('Use limit orders for better execution')
        
        return risk_assessment
    
    def calculate_position_size(self, signal, risk_assessment) -> float:
        """Calculate optimal position size using Kelly Criterion and risk management"""
        
        # Base position size
        base_size = min(self.max_position_size, signal.price * 0.1)  # 10% of price as base
        
        # Kelly Criterion adjustment
        win_prob = risk_assessment.get('win_probability', 0.5)
        avg_win = risk_assessment.get('avg_win', 0.05)
        avg_loss = risk_assessment.get('avg_loss', 0.03)
        
        if avg_loss > 0:
            kelly_fraction = (win_prob * avg_win - (1 - win_prob) * avg_loss) / avg_win
            kelly_fraction = max(0, min(0.25, kelly_fraction))  # Cap at 25%
        else:
            kelly_fraction = 0.1
        
        # Risk-adjusted position size
        risk_multiplier = 1.0 - risk_assessment['risk_score']
        position_size = base_size * kelly_fraction * risk_multiplier
        
        # Apply position limits
        symbol_limit = self.position_limits.get(signal.symbol, self.max_position_size)
        position_size = min(position_size, symbol_limit)
        
        # Minimum position size
        min_size = signal.price * 0.001  # 0.1% of price
        position_size = max(position_size, min_size)
        
        return position_size
    
    async def make_final_decision(self, signal, combined_signal, risk_assessment, position_size) -> Optional[TradingDecision]:
        """Make final trading decision with all risk checks"""
        
        action = combined_signal['action']
        confidence = combined_signal['confidence']
        
        # Risk gates
        if not self.pass_risk_gates(risk_assessment, confidence):
            logger.info(
                "Trade rejected by risk gates",
                symbol=signal.symbol,
                risk_score=risk_assessment['risk_score'],
                confidence=confidence
            )
            return None
        
        # Sharpe ratio check
        if risk_assessment['sharpe_estimate'] < self.sharpe_threshold:
            logger.info(
                "Trade rejected due to low Sharpe ratio",
                symbol=signal.symbol,
                sharpe=risk_assessment['sharpe_estimate'],
                threshold=self.sharpe_threshold
            )
            return None
        
        # Don't trade on hold signals with low confidence
        if action == 'hold' or confidence < 0.6:
            return None
        
        # Calculate price target
        price_target = self.calculate_price_target(signal, combined_signal, action)
        
        # Create decision
        decision = TradingDecision(
            decision_id=str(uuid.uuid4()),
            symbol=signal.symbol,
            action=action,
            confidence=confidence,
            quantity=position_size / signal.price,  # Convert to quantity
            price_target=price_target,
            reasoning=combined_signal['reasoning'],
            risk_assessment=risk_assessment,
            timestamp=datetime.now(timezone.utc)
        )
        
        return decision
    
    def pass_risk_gates(self, risk_assessment, confidence) -> bool:
        """Check if trade passes all risk gates"""
        
        # Risk score gate
        if risk_assessment['risk_score'] > 0.8:
            return False
        
        # Confidence gate
        if confidence < 0.5:
            return False
        
        # Sharpe ratio gate
        if risk_assessment['sharpe_estimate'] < 0.5:
            return False
        
        return True
    
    def calculate_price_target(self, signal, combined_signal, action) -> float:
        """Calculate price target based on analysis"""
        current_price = signal.price
        
        # Base target calculation
        technical_score = combined_signal['fact_signal']['technical_score']
        sentiment_score = combined_signal['subjectivity_signal']['sentiment_score']
        
        # Expected move percentage
        expected_move = (abs(technical_score) + abs(sentiment_score)) / 2 * 0.05  # 5% max move
        
        if action == 'buy':
            price_target = current_price * (1 + expected_move)
        elif action == 'sell':
            price_target = current_price * (1 - expected_move)
        else:
            price_target = current_price
        
        return price_target