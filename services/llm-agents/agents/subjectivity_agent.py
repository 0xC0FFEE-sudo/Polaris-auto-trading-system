"""
Subjectivity Agent - Processes qualitative/sentiment data for trading decisions
"""

import asyncio
import json
import re
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

import structlog
import openai
from transformers import pipeline, AutoTokenizer, AutoModelForSequenceClassification
import numpy as np

logger = structlog.get_logger()

@dataclass
class SubjectivityAnalysis:
    """Result of subjectivity-based analysis"""
    sentiment_score: float  # -1 to 1
    emotion_analysis: Dict[str, float]
    narrative_themes: List[str]
    social_signals: Dict[str, Any]
    market_psychology: Dict[str, float]
    confidence: float
    reasoning: List[str]

class SubjectivityAgent:
    """Agent responsible for analyzing qualitative/sentiment data"""
    
    def __init__(self, config):
        self.config = config
        self.sentiment_history = {}  # Symbol -> sentiment history
        
        # Initialize NLP models
        self.setup_nlp_models()
        
        # Initialize OpenAI client if available
        if hasattr(config, 'openai_api_key') and config.openai_api_key:
            openai.api_key = config.openai_api_key
            self.use_openai = True
        else:
            self.use_openai = False
    
    def setup_nlp_models(self):
        """Setup NLP models for sentiment analysis"""
        try:
            # Crypto-specific sentiment model
            self.sentiment_analyzer = pipeline(
                "sentiment-analysis",
                model="ElKulako/cryptobert",
                tokenizer="ElKulako/cryptobert"
            )
            
            # Emotion analysis model
            self.emotion_analyzer = pipeline(
                "text-classification",
                model="j-hartmann/emotion-english-distilroberta-base"
            )
            
            # Financial news sentiment
            self.financial_sentiment = pipeline(
                "sentiment-analysis",
                model="ProsusAI/finbert"
            )
            
            logger.info("NLP models loaded successfully")
            
        except Exception as e:
            logger.error("Error loading NLP models", error=str(e))
            # Fallback to basic sentiment
            self.sentiment_analyzer = pipeline("sentiment-analysis")
            self.emotion_analyzer = None
            self.financial_sentiment = None
    
    async def analyze(self, signal) -> SubjectivityAnalysis:
        """Analyze market signal using qualitative methods"""
        start_time = datetime.now()
        
        try:
            # Update sentiment history
            self.update_sentiment_history(signal)
            
            # Analyze current sentiment
            sentiment_score = await self.analyze_sentiment(signal)
            
            # Analyze emotions
            emotion_analysis = await self.analyze_emotions(signal)
            
            # Extract narrative themes
            narrative_themes = await self.extract_narrative_themes(signal)
            
            # Analyze social signals
            social_signals = await self.analyze_social_signals(signal)
            
            # Assess market psychology
            market_psychology = await self.assess_market_psychology(signal)
            
            # Calculate confidence
            confidence = self.calculate_confidence(
                sentiment_score, emotion_analysis, social_signals
            )
            
            # Generate reasoning
            reasoning = await self.generate_reasoning(
                signal, sentiment_score, emotion_analysis, narrative_themes
            )
            
            analysis = SubjectivityAnalysis(
                sentiment_score=sentiment_score,
                emotion_analysis=emotion_analysis,
                narrative_themes=narrative_themes,
                social_signals=social_signals,
                market_psychology=market_psychology,
                confidence=confidence,
                reasoning=reasoning
            )
            
            processing_time = (datetime.now() - start_time).total_seconds()
            logger.info(
                "Subjectivity analysis completed",
                symbol=signal.symbol,
                sentiment_score=sentiment_score,
                confidence=confidence,
                processing_time=processing_time
            )
            
            return analysis
            
        except Exception as e:
            logger.error("Error in subjectivity analysis", symbol=signal.symbol, error=str(e))
            return self.create_default_analysis()
    
    def update_sentiment_history(self, signal):
        """Update sentiment history"""
        symbol = signal.symbol
        
        if symbol not in self.sentiment_history:
            self.sentiment_history[symbol] = []
        
        self.sentiment_history[symbol].append({
            'sentiment_score': signal.sentiment_score,
            'timestamp': signal.timestamp
        })
        
        # Keep only last 100 data points
        self.sentiment_history[symbol] = self.sentiment_history[symbol][-100:]
    
    async def analyze_sentiment(self, signal) -> float:
        """Analyze sentiment from multiple sources"""
        try:
            # Base sentiment from signal
            base_sentiment = signal.sentiment_score
            
            # Analyze on-chain activity sentiment
            onchain_sentiment = await self.analyze_onchain_sentiment(signal.on_chain_activity)
            
            # Analyze price action sentiment
            price_sentiment = self.analyze_price_sentiment(signal)
            
            # Combine sentiments with weights
            combined_sentiment = (
                base_sentiment * 0.4 +
                onchain_sentiment * 0.3 +
                price_sentiment * 0.3
            )
            
            # Normalize to -1 to 1 range
            return max(-1.0, min(1.0, combined_sentiment))
            
        except Exception as e:
            logger.error("Error analyzing sentiment", error=str(e))
            return 0.0
    
    async def analyze_onchain_sentiment(self, onchain_activity: Dict[str, Any]) -> float:
        """Analyze sentiment from on-chain activity"""
        if not onchain_activity:
            return 0.0
        
        sentiment_score = 0.0
        
        # Analyze transaction patterns
        tx_count = onchain_activity.get('transaction_count', 0)
        if tx_count > 1000:  # High activity
            sentiment_score += 0.2
        elif tx_count < 100:  # Low activity
            sentiment_score -= 0.1
        
        # Analyze whale movements
        large_transfers = onchain_activity.get('large_transfers', 0)
        if large_transfers > 10:
            sentiment_score -= 0.3  # Potential selling pressure
        
        # Analyze exchange flows
        exchange_inflow = onchain_activity.get('exchange_inflow', 0)
        exchange_outflow = onchain_activity.get('exchange_outflow', 0)
        
        if exchange_outflow > exchange_inflow:
            sentiment_score += 0.2  # Bullish (HODLing)
        elif exchange_inflow > exchange_outflow:
            sentiment_score -= 0.2  # Bearish (selling pressure)
        
        return sentiment_score
    
    def analyze_price_sentiment(self, signal) -> float:
        """Derive sentiment from price action"""
        symbol = signal.symbol
        
        if symbol not in self.sentiment_history or len(self.sentiment_history[symbol]) < 5:
            return 0.0
        
        # Recent sentiment trend
        recent_sentiments = [s['sentiment_score'] for s in self.sentiment_history[symbol][-5:]]
        sentiment_trend = np.mean(recent_sentiments)
        
        # Price momentum sentiment
        current_price = signal.price
        if len(self.sentiment_history[symbol]) > 1:
            prev_data = self.sentiment_history[symbol][-2]
            # This is simplified - in production would track price history
            price_momentum = 0.1 if current_price > 50000 else -0.1  # Placeholder
        else:
            price_momentum = 0.0
        
        return (sentiment_trend * 0.7) + (price_momentum * 0.3)
    
    async def analyze_emotions(self, signal) -> Dict[str, float]:
        """Analyze emotional content"""
        if not self.emotion_analyzer:
            return {"neutral": 1.0}
        
        try:
            # Simulate emotional analysis of market data
            # In production, this would analyze news, social media, etc.
            emotions = {
                "fear": 0.1,
                "greed": 0.2,
                "optimism": 0.3,
                "pessimism": 0.2,
                "uncertainty": 0.2
            }
            
            # Adjust based on sentiment score
            if signal.sentiment_score > 0.5:
                emotions["greed"] += 0.2
                emotions["optimism"] += 0.2
                emotions["fear"] -= 0.1
            elif signal.sentiment_score < -0.5:
                emotions["fear"] += 0.3
                emotions["pessimism"] += 0.2
                emotions["greed"] -= 0.1
            
            # Normalize
            total = sum(emotions.values())
            if total > 0:
                emotions = {k: v/total for k, v in emotions.items()}
            
            return emotions
            
        except Exception as e:
            logger.error("Error analyzing emotions", error=str(e))
            return {"neutral": 1.0}
    
    async def extract_narrative_themes(self, signal) -> List[str]:
        """Extract dominant narrative themes"""
        themes = []
        
        # Analyze based on sentiment and market conditions
        if signal.sentiment_score > 0.3:
            themes.extend(["bullish_momentum", "institutional_adoption", "technological_progress"])
        elif signal.sentiment_score < -0.3:
            themes.extend(["market_correction", "regulatory_concerns", "profit_taking"])
        else:
            themes.extend(["consolidation", "uncertainty", "range_bound"])
        
        # Add on-chain themes
        if signal.on_chain_activity:
            if signal.on_chain_activity.get('exchange_outflow', 0) > signal.on_chain_activity.get('exchange_inflow', 0):
                themes.append("hodling_behavior")
            else:
                themes.append("selling_pressure")
        
        return themes[:5]  # Return top 5 themes
    
    async def analyze_social_signals(self, signal) -> Dict[str, Any]:
        """Analyze social media and community signals"""
        # Placeholder for social signal analysis
        # In production, this would integrate with Twitter API, Reddit, Discord, etc.
        
        return {
            "twitter_mentions": 1000,
            "reddit_sentiment": signal.sentiment_score,
            "discord_activity": 0.5,
            "influencer_sentiment": signal.sentiment_score * 0.8,
            "community_engagement": 0.6,
            "viral_potential": 0.3
        }
    
    async def assess_market_psychology(self, signal) -> Dict[str, float]:
        """Assess overall market psychology"""
        psychology = {
            "fear_greed_index": 50.0,  # 0-100 scale
            "fomo_level": 0.3,
            "panic_level": 0.2,
            "euphoria_level": 0.1,
            "capitulation_risk": 0.1
        }
        
        # Adjust based on sentiment
        if signal.sentiment_score > 0.5:
            psychology["fear_greed_index"] = 70.0
            psychology["fomo_level"] = 0.6
            psychology["euphoria_level"] = 0.4
        elif signal.sentiment_score < -0.5:
            psychology["fear_greed_index"] = 30.0
            psychology["panic_level"] = 0.5
            psychology["capitulation_risk"] = 0.3
        
        # Adjust based on volatility
        volatility = signal.on_chain_activity.get('volatility', 0.0)
        if volatility > 0.5:
            psychology["panic_level"] += 0.2
            psychology["fear_greed_index"] -= 10.0
        
        return psychology
    
    def calculate_confidence(self, sentiment_score: float, emotion_analysis: Dict, social_signals: Dict) -> float:
        """Calculate confidence in subjectivity analysis"""
        confidence = 0.5  # Base confidence
        
        # Adjust based on sentiment strength
        sentiment_strength = abs(sentiment_score)
        confidence += sentiment_strength * 0.3
        
        # Adjust based on emotion clarity
        max_emotion = max(emotion_analysis.values()) if emotion_analysis else 0
        if max_emotion > 0.5:
            confidence += 0.1
        
        # Adjust based on social signal strength
        community_engagement = social_signals.get("community_engagement", 0)
        confidence += community_engagement * 0.2
        
        return min(confidence, 1.0)
    
    async def generate_reasoning(self, signal, sentiment_score: float, emotion_analysis: Dict, themes: List[str]) -> List[str]:
        """Generate human-readable reasoning"""
        reasoning = []
        
        # Sentiment reasoning
        if sentiment_score > 0.3:
            reasoning.append(f"Strong positive sentiment detected (score: {sentiment_score:.2f})")
        elif sentiment_score < -0.3:
            reasoning.append(f"Strong negative sentiment detected (score: {sentiment_score:.2f})")
        else:
            reasoning.append(f"Neutral sentiment observed (score: {sentiment_score:.2f})")
        
        # Emotion reasoning
        if emotion_analysis:
            dominant_emotion = max(emotion_analysis.items(), key=lambda x: x[1])
            reasoning.append(f"Dominant market emotion: {dominant_emotion[0]} ({dominant_emotion[1]:.2f})")
        
        # Theme reasoning
        if themes:
            reasoning.append(f"Key narrative themes: {', '.join(themes[:3])}")
        
        # On-chain reasoning
        if signal.on_chain_activity:
            if signal.on_chain_activity.get('exchange_outflow', 0) > signal.on_chain_activity.get('exchange_inflow', 0):
                reasoning.append("On-chain data shows net outflow from exchanges (bullish)")
            else:
                reasoning.append("On-chain data shows net inflow to exchanges (bearish)")
        
        return reasoning
    
    def create_default_analysis(self) -> SubjectivityAnalysis:
        """Create default analysis when errors occur"""
        return SubjectivityAnalysis(
            sentiment_score=0.0,
            emotion_analysis={"neutral": 1.0},
            narrative_themes=["uncertainty"],
            social_signals={},
            market_psychology={"fear_greed_index": 50.0},
            confidence=0.1,
            reasoning=["Analysis failed, using default neutral stance"]
        )