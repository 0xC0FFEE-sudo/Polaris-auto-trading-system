"""
Reflection Engine - Provides explainability and continuous learning
"""

import asyncio
import json
from datetime import datetime, timezone
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

import structlog

logger = structlog.get_logger()

@dataclass
class ReflectionInsight:
    """Single reflection insight"""
    insight_type: str
    description: str
    confidence: float
    supporting_evidence: List[str]
    timestamp: datetime

class ReflectionEngine:
    """Engine for reflective reasoning and explainability"""
    
    def __init__(self, config):
        self.config = config
        self.reflection_history = []
        self.insight_patterns = {}
        
    async def reflect(self, signal, fact_analysis, subjectivity_analysis, decision) -> Dict[str, Any]:
        """Perform reflection on a single trading decision"""
        
        try:
            reflection = {
                'decision_id': decision.decision_id if decision else 'no_decision',
                'symbol': signal.symbol,
                'timestamp': datetime.now(timezone.utc).isoformat(),
                'chain_of_thought': [],
                'insights': [],
                'confidence_analysis': {},
                'risk_reasoning': {},
                'alternative_scenarios': [],
                'learning_points': []
            }
            
            # Build chain of thought
            reflection['chain_of_thought'] = await self.build_chain_of_thought(
                signal, fact_analysis, subjectivity_analysis, decision
            )
            
            # Generate insights
            reflection['insights'] = await self.generate_insights(
                signal, fact_analysis, subjectivity_analysis, decision
            )
            
            # Analyze confidence
            reflection['confidence_analysis'] = await self.analyze_confidence(
                fact_analysis, subjectivity_analysis, decision
            )
            
            # Risk reasoning
            if decision:
                reflection['risk_reasoning'] = await self.analyze_risk_reasoning(decision)
            
            # Alternative scenarios
            reflection['alternative_scenarios'] = await self.generate_alternative_scenarios(
                signal, fact_analysis, subjectivity_analysis
            )
            
            # Learning points
            reflection['learning_points'] = await self.extract_learning_points(
                signal, fact_analysis, subjectivity_analysis, decision
            )
            
            # Summary
            reflection['summary'] = await self.generate_summary(reflection)
            
            # Store reflection
            self.reflection_history.append(reflection)
            
            logger.info(
                "Reflection completed",
                symbol=signal.symbol,
                insights_count=len(reflection['insights']),
                chain_length=len(reflection['chain_of_thought'])
            )
            
            return reflection
            
        except Exception as e:
            logger.error("Error in reflection", symbol=signal.symbol, error=str(e))
            return {'error': str(e), 'summary': 'Reflection failed'}
    
    async def build_chain_of_thought(self, signal, fact_analysis, subjectivity_analysis, decision) -> List[Dict[str, Any]]:
        """Build detailed chain of thought for the decision"""
        
        chain = []
        
        # Step 1: Initial signal processing
        chain.append({
            'step': 1,
            'type': 'signal_processing',
            'description': f'Received market signal for {signal.symbol}',
            'data': {
                'price': signal.price,
                'volume': signal.volume,
                'sentiment_score': signal.sentiment_score,
                'timestamp': signal.timestamp.isoformat()
            },
            'reasoning': 'Processing incoming market data and sentiment signals'
        })
        
        # Step 2: Fact analysis
        chain.append({
            'step': 2,
            'type': 'fact_analysis',
            'description': f'Technical analysis shows {fact_analysis.price_trend} trend',
            'data': {
                'price_trend': fact_analysis.price_trend,
                'confidence': fact_analysis.confidence,
                'key_indicators': dict(list(fact_analysis.technical_indicators.items())[:3])
            },
            'reasoning': f'Technical indicators: {", ".join(fact_analysis.reasoning[:2])}'
        })
        
        # Step 3: Subjectivity analysis
        chain.append({
            'step': 3,
            'type': 'subjectivity_analysis',
            'description': f'Sentiment analysis reveals {subjectivity_analysis.sentiment_score:.2f} sentiment score',
            'data': {
                'sentiment_score': subjectivity_analysis.sentiment_score,
                'confidence': subjectivity_analysis.confidence,
                'dominant_themes': subjectivity_analysis.narrative_themes[:3]
            },
            'reasoning': f'Sentiment factors: {", ".join(subjectivity_analysis.reasoning[:2])}'
        })
        
        # Step 4: Risk assessment
        if decision:
            chain.append({
                'step': 4,
                'type': 'risk_assessment',
                'description': f'Risk analysis for {decision.action} decision',
                'data': {
                    'risk_score': decision.risk_assessment.get('risk_score', 0),
                    'sharpe_estimate': decision.risk_assessment.get('sharpe_estimate', 0),
                    'risk_factors': decision.risk_assessment.get('risk_factors', [])
                },
                'reasoning': f'Risk considerations led to {decision.action} with {decision.confidence:.2f} confidence'
            })
        
        # Step 5: Final decision
        if decision:
            chain.append({
                'step': 5,
                'type': 'final_decision',
                'description': f'Decision: {decision.action.upper()} {decision.quantity:.4f} at target {decision.price_target:.2f}',
                'data': {
                    'action': decision.action,
                    'quantity': decision.quantity,
                    'price_target': decision.price_target,
                    'confidence': decision.confidence
                },
                'reasoning': 'Bayesian voting combined with risk gates produced final trading decision'
            })
        else:
            chain.append({
                'step': 5,
                'type': 'no_decision',
                'description': 'No trading action taken',
                'data': {'reason': 'Failed risk gates or insufficient confidence'},
                'reasoning': 'Risk management prevented trade execution'
            })
        
        return chain
    
    async def generate_insights(self, signal, fact_analysis, subjectivity_analysis, decision) -> List[ReflectionInsight]:
        """Generate insights from the analysis"""
        
        insights = []
        
        # Technical vs Sentiment alignment insight
        tech_bullish = fact_analysis.price_trend == "bullish"
        sentiment_bullish = subjectivity_analysis.sentiment_score > 0.1
        
        if tech_bullish == sentiment_bullish:
            insights.append(ReflectionInsight(
                insight_type="alignment",
                description="Technical analysis and sentiment are aligned",
                confidence=0.8,
                supporting_evidence=[
                    f"Technical trend: {fact_analysis.price_trend}",
                    f"Sentiment score: {subjectivity_analysis.sentiment_score:.2f}"
                ],
                timestamp=datetime.now(timezone.utc)
            ))
        else:
            insights.append(ReflectionInsight(
                insight_type="divergence",
                description="Technical analysis and sentiment are diverging",
                confidence=0.7,
                supporting_evidence=[
                    f"Technical trend: {fact_analysis.price_trend}",
                    f"Sentiment score: {subjectivity_analysis.sentiment_score:.2f}"
                ],
                timestamp=datetime.now(timezone.utc)
            ))
        
        # Volume confirmation insight
        volume_spike = fact_analysis.volume_analysis.get('volume_spike', False)
        if volume_spike:
            insights.append(ReflectionInsight(
                insight_type="volume_confirmation",
                description="Volume spike confirms price movement",
                confidence=0.9,
                supporting_evidence=[
                    f"Volume ratio: {fact_analysis.volume_analysis.get('volume_ratio', 1):.2f}",
                    "Significant increase in trading activity"
                ],
                timestamp=datetime.now(timezone.utc)
            ))
        
        # Market psychology insight
        fear_greed = subjectivity_analysis.market_psychology.get('fear_greed_index', 50)
        if fear_greed > 75:
            insights.append(ReflectionInsight(
                insight_type="market_psychology",
                description="Market showing extreme greed - potential reversal risk",
                confidence=0.6,
                supporting_evidence=[
                    f"Fear & Greed Index: {fear_greed}",
                    "Contrarian signals may be emerging"
                ],
                timestamp=datetime.now(timezone.utc)
            ))
        elif fear_greed < 25:
            insights.append(ReflectionInsight(
                insight_type="market_psychology",
                description="Market showing extreme fear - potential buying opportunity",
                confidence=0.6,
                supporting_evidence=[
                    f"Fear & Greed Index: {fear_greed}",
                    "Oversold conditions may present value"
                ],
                timestamp=datetime.now(timezone.utc)
            ))
        
        # Decision quality insight
        if decision:
            if decision.confidence > 0.8:
                insights.append(ReflectionInsight(
                    insight_type="decision_quality",
                    description="High confidence decision with strong signal alignment",
                    confidence=decision.confidence,
                    supporting_evidence=[
                        f"Decision confidence: {decision.confidence:.2f}",
                        f"Action: {decision.action}",
                        "Multiple indicators supporting decision"
                    ],
                    timestamp=datetime.now(timezone.utc)
                ))
        
        return insights
    
    async def analyze_confidence(self, fact_analysis, subjectivity_analysis, decision) -> Dict[str, Any]:
        """Analyze confidence levels across different components"""
        
        confidence_analysis = {
            'fact_confidence': fact_analysis.confidence,
            'subjectivity_confidence': subjectivity_analysis.confidence,
            'overall_confidence': decision.confidence if decision else 0.0,
            'confidence_factors': [],
            'confidence_risks': []
        }
        
        # Analyze confidence factors
        if fact_analysis.confidence > 0.7:
            confidence_analysis['confidence_factors'].append("Strong technical signals")
        
        if subjectivity_analysis.confidence > 0.7:
            confidence_analysis['confidence_factors'].append("Clear sentiment direction")
        
        if len(fact_analysis.technical_indicators) > 5:
            confidence_analysis['confidence_factors'].append("Multiple technical indicators available")
        
        # Analyze confidence risks
        if abs(fact_analysis.confidence - subjectivity_analysis.confidence) > 0.3:
            confidence_analysis['confidence_risks'].append("Large confidence gap between agents")
        
        if fact_analysis.confidence < 0.5 or subjectivity_analysis.confidence < 0.5:
            confidence_analysis['confidence_risks'].append("Low confidence from one or more agents")
        
        return confidence_analysis
    
    async def analyze_risk_reasoning(self, decision) -> Dict[str, Any]:
        """Analyze the risk reasoning behind the decision"""
        
        risk_reasoning = {
            'risk_score': decision.risk_assessment.get('risk_score', 0),
            'risk_factors': decision.risk_assessment.get('risk_factors', []),
            'risk_mitigation': decision.risk_assessment.get('risk_mitigation', []),
            'position_sizing_rationale': f"Position sized at {decision.quantity:.4f} based on risk tolerance",
            'sharpe_analysis': f"Estimated Sharpe ratio: {decision.risk_assessment.get('sharpe_estimate', 0):.2f}"
        }
        
        return risk_reasoning
    
    async def generate_alternative_scenarios(self, signal, fact_analysis, subjectivity_analysis) -> List[Dict[str, Any]]:
        """Generate alternative scenarios and their implications"""
        
        scenarios = []
        
        # Scenario 1: If sentiment was opposite
        opposite_sentiment = -subjectivity_analysis.sentiment_score
        scenarios.append({
            'scenario': 'opposite_sentiment',
            'description': f'If sentiment was {opposite_sentiment:.2f} instead of {subjectivity_analysis.sentiment_score:.2f}',
            'likely_outcome': 'buy' if opposite_sentiment > 0.1 else 'sell',
            'probability': 0.3,
            'implications': 'Would likely reverse the trading decision'
        })
        
        # Scenario 2: If technical trend was different
        alt_trend = 'bearish' if fact_analysis.price_trend == 'bullish' else 'bullish'
        scenarios.append({
            'scenario': 'alternative_technical_trend',
            'description': f'If technical analysis showed {alt_trend} instead of {fact_analysis.price_trend}',
            'likely_outcome': 'sell' if alt_trend == 'bearish' else 'buy',
            'probability': 0.2,
            'implications': 'Would create conflict between technical and sentiment signals'
        })
        
        # Scenario 3: Higher volatility
        scenarios.append({
            'scenario': 'high_volatility',
            'description': 'If market volatility suddenly increased significantly',
            'likely_outcome': 'hold',
            'probability': 0.4,
            'implications': 'Would trigger risk management protocols and reduce position sizes'
        })
        
        return scenarios
    
    async def extract_learning_points(self, signal, fact_analysis, subjectivity_analysis, decision) -> List[str]:
        """Extract key learning points from this decision cycle"""
        
        learning_points = []
        
        # Data quality learning
        if len(fact_analysis.technical_indicators) < 3:
            learning_points.append("Need more technical indicators for better analysis")
        
        # Confidence learning
        if decision and decision.confidence < 0.6:
            learning_points.append("Low confidence decisions should be avoided or position sizes reduced")
        
        # Agent alignment learning
        conf_diff = abs(fact_analysis.confidence - subjectivity_analysis.confidence)
        if conf_diff > 0.4:
            learning_points.append("Large confidence gaps between agents indicate uncertainty")
        
        # Volume learning
        if fact_analysis.volume_analysis.get('volume_spike', False):
            learning_points.append("Volume spikes provide important confirmation signals")
        
        # Sentiment learning
        if abs(subjectivity_analysis.sentiment_score) > 0.8:
            learning_points.append("Extreme sentiment levels may indicate reversal opportunities")
        
        return learning_points
    
    async def generate_summary(self, reflection) -> str:
        """Generate a concise summary of the reflection"""
        
        decision_id = reflection.get('decision_id', 'unknown')
        symbol = reflection.get('symbol', 'unknown')
        insights_count = len(reflection.get('insights', []))
        
        # Extract key points
        chain_of_thought = reflection.get('chain_of_thought', [])
        final_step = chain_of_thought[-1] if chain_of_thought else {}
        
        if final_step.get('type') == 'final_decision':
            action = final_step.get('data', {}).get('action', 'unknown')
            confidence = final_step.get('data', {}).get('confidence', 0)
            summary = f"Decision {decision_id}: {action.upper()} {symbol} with {confidence:.1%} confidence. Generated {insights_count} insights through {len(chain_of_thought)} reasoning steps."
        else:
            summary = f"Decision {decision_id}: No action taken for {symbol}. Risk management prevented execution. Generated {insights_count} insights."
        
        # Add key insight
        insights = reflection.get('insights', [])
        if insights:
            key_insight = insights[0]  # First insight is usually most important
            summary += f" Key insight: {key_insight.description}"
        
        return summary
    
    async def batch_reflect(self, decisions: List) -> Dict[str, Any]:
        """Perform batch reflection on multiple decisions for learning"""
        
        batch_reflection = {
            'batch_id': str(datetime.now(timezone.utc).timestamp()),
            'decision_count': len(decisions),
            'timestamp': datetime.now(timezone.utc).isoformat(),
            'patterns': {},
            'performance_insights': [],
            'improvement_suggestions': []
        }
        
        if not decisions:
            return batch_reflection
        
        # Analyze patterns
        actions = [d.action for d in decisions]
        action_distribution = {action: actions.count(action) for action in set(actions)}
        batch_reflection['patterns']['action_distribution'] = action_distribution
        
        # Confidence patterns
        confidences = [d.confidence for d in decisions]
        avg_confidence = sum(confidences) / len(confidences)
        batch_reflection['patterns']['average_confidence'] = avg_confidence
        
        # Performance insights
        if avg_confidence < 0.6:
            batch_reflection['performance_insights'].append(
                "Average confidence is low - may need better signal quality"
            )
        
        if action_distribution.get('hold', 0) > len(decisions) * 0.7:
            batch_reflection['performance_insights'].append(
                "High percentage of hold decisions - may be too conservative"
            )
        
        # Improvement suggestions
        batch_reflection['improvement_suggestions'] = [
            "Consider adjusting agent weights based on recent performance",
            "Monitor correlation between confidence levels and actual outcomes",
            "Evaluate risk gate effectiveness"
        ]
        
        return batch_reflection