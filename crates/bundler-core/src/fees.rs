use bundler_types::{BundlerError, BundlerResult, FeeStrategy, Lamports};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    pubkey::Pubkey,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

use crate::rpc::SolanaRpcClient;

/// Historical fee data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeDataPoint {
    pub timestamp: DateTime<Utc>,
    pub slot: u64,
    pub fee_lamports: Lamports,
    pub percentile: u8,
}

/// Fee trend analysis
#[derive(Debug, Clone)]
pub struct FeeTrend {
    pub direction: TrendDirection,
    pub magnitude: f64, // Percentage change
    pub confidence: f64, // 0.0 to 1.0
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrendDirection {
    Rising,
    Falling,
    Stable,
}

/// Adaptive fee manager that adjusts fees based on network conditions
pub struct FeeManager {
    rpc_client: Arc<SolanaRpcClient>,
    strategy: FeeStrategy,
    fee_history: Arc<RwLock<VecDeque<FeeDataPoint>>>,
    max_history_size: usize,
}

impl FeeManager {
    /// Create a new fee manager
    pub fn new(rpc_client: Arc<SolanaRpcClient>, strategy: FeeStrategy) -> Self {
        Self {
            rpc_client,
            strategy,
            fee_history: Arc::new(RwLock::new(VecDeque::new())),
            max_history_size: 100, // Keep last 100 data points
        }
    }

    /// Calculate the optimal fee for a transaction
    pub async fn calculate_fee(&self, accounts: &[Pubkey]) -> BundlerResult<Lamports> {
        // Get recent prioritization fees
        let recent_fees = self.rpc_client
            .get_recent_prioritization_fees(accounts)
            .await?;

        if recent_fees.is_empty() {
            warn!("No recent prioritization fees available, using minimum fee");
            return Ok(1); // Minimum fee
        }

        // Calculate base fee using percentile strategy
        let base_fee = self.calculate_percentile_fee(&recent_fees)?;
        
        // Apply adaptive adjustments if enabled
        let adjusted_fee = if self.strategy.adaptive {
            self.apply_adaptive_adjustments(base_fee, &recent_fees).await?
        } else {
            base_fee
        };

        // Apply buffer
        let buffered_fee = self.apply_buffer(adjusted_fee);

        // Ensure we don't exceed maximum price
        let final_fee = std::cmp::min(buffered_fee, self.strategy.max_price_lamports);

        // Record this fee calculation for trend analysis
        self.record_fee_calculation(final_fee).await;

        debug!(
            "Fee calculation: base={}, adjusted={}, buffered={}, final={}",
            base_fee, adjusted_fee, buffered_fee, final_fee
        );

        Ok(final_fee)
    }

    /// Calculate fee based on percentile strategy
    fn calculate_percentile_fee(&self, fees: &[u64]) -> BundlerResult<Lamports> {
        if fees.is_empty() {
            return Ok(1);
        }

        let mut sorted_fees = fees.to_vec();
        sorted_fees.sort_unstable();

        let percentile_index = ((sorted_fees.len() as f64 * self.strategy.base_percentile as f64 / 100.0) as usize)
            .saturating_sub(1)
            .min(sorted_fees.len() - 1);

        Ok(sorted_fees[percentile_index])
    }

    /// Apply adaptive adjustments based on network trends
    async fn apply_adaptive_adjustments(&self, base_fee: Lamports, recent_fees: &[u64]) -> BundlerResult<Lamports> {
        let trend = self.analyze_fee_trend().await;
        
        let adjustment_factor = match trend.direction {
            TrendDirection::Rising => {
                // If fees are rising rapidly, be more aggressive
                let aggressiveness = (trend.magnitude * trend.confidence).min(0.5); // Max 50% increase
                1.0 + aggressiveness
            }
            TrendDirection::Falling => {
                // If fees are falling, we can be more conservative
                let conservativeness = (trend.magnitude * trend.confidence * 0.5).min(0.2); // Max 20% decrease
                1.0 - conservativeness
            }
            TrendDirection::Stable => 1.0, // No adjustment
        };

        let adjusted_fee = (base_fee as f64 * adjustment_factor) as Lamports;

        debug!(
            "Adaptive adjustment: trend={:?}, magnitude={:.2}, confidence={:.2}, factor={:.2}",
            trend.direction, trend.magnitude, trend.confidence, adjustment_factor
        );

        Ok(adjusted_fee)
    }

    /// Apply buffer percentage to the fee
    fn apply_buffer(&self, fee: Lamports) -> Lamports {
        let buffer_multiplier = 1.0 + (self.strategy.buffer_percent as f64 / 100.0);
        (fee as f64 * buffer_multiplier) as Lamports
    }

    /// Analyze fee trends from historical data
    async fn analyze_fee_trend(&self) -> FeeTrend {
        let history = self.fee_history.read().unwrap();
        
        if history.len() < 5 {
            return FeeTrend {
                direction: TrendDirection::Stable,
                magnitude: 0.0,
                confidence: 0.0,
            };
        }

        // Analyze the last 10 data points (or all if less than 10)
        let recent_count = std::cmp::min(10, history.len());
        let recent_fees: Vec<f64> = history
            .iter()
            .rev()
            .take(recent_count)
            .map(|point| point.fee_lamports as f64)
            .collect();

        // Calculate linear regression to determine trend
        let (slope, r_squared) = self.calculate_linear_regression(&recent_fees);
        
        let direction = if slope > 0.05 {
            TrendDirection::Rising
        } else if slope < -0.05 {
            TrendDirection::Falling
        } else {
            TrendDirection::Stable
        };

        let magnitude = slope.abs();
        let confidence = r_squared; // R-squared as confidence measure

        FeeTrend {
            direction,
            magnitude,
            confidence,
        }
    }

    /// Calculate linear regression for trend analysis
    fn calculate_linear_regression(&self, values: &[f64]) -> (f64, f64) {
        if values.len() < 2 {
            return (0.0, 0.0);
        }

        let n = values.len() as f64;
        let x_values: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
        
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(values.iter()).map(|(x, y)| x * y).sum();
        let sum_x_squared: f64 = x_values.iter().map(|x| x * x).sum();
        let sum_y_squared: f64 = values.iter().map(|y| y * y).sum();

        // Calculate slope (beta)
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x_squared - sum_x * sum_x);
        
        // Calculate R-squared
        let y_mean = sum_y / n;
        let ss_tot: f64 = values.iter().map(|y| (y - y_mean).powi(2)).sum();
        let ss_res: f64 = x_values.iter().zip(values.iter())
            .map(|(x, y)| {
                let predicted = slope * x + (sum_y - slope * sum_x) / n;
                (y - predicted).powi(2)
            })
            .sum();
        
        let r_squared = if ss_tot > 0.0 { 1.0 - (ss_res / ss_tot) } else { 0.0 };

        (slope, r_squared.max(0.0).min(1.0))
    }

    /// Record a fee calculation for trend analysis
    async fn record_fee_calculation(&self, fee: Lamports) {
        let mut history = self.fee_history.write().unwrap();
        
        let data_point = FeeDataPoint {
            timestamp: Utc::now(),
            slot: 0, // We could get the current slot here if needed
            fee_lamports: fee,
            percentile: self.strategy.base_percentile,
        };

        history.push_back(data_point);

        // Keep only the most recent data points
        while history.len() > self.max_history_size {
            history.pop_front();
        }
    }

    /// Create compute budget instructions for a transaction
    pub fn create_compute_budget_instructions(
        &self,
        compute_units: u32,
        fee_lamports: Lamports,
    ) -> Vec<Instruction> {
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(compute_units),
            ComputeBudgetInstruction::set_compute_unit_price(fee_lamports),
        ]
    }

    /// Implement fee bumping for failed transactions
    pub async fn bump_fee(&self, original_fee: Lamports, attempt: u32) -> BundlerResult<Lamports> {
        if !self.strategy.enable_bump {
            return Ok(original_fee);
        }

        if attempt > self.strategy.max_bump_attempts {
            return Err(BundlerError::Transaction(
                format!("Maximum fee bump attempts ({}) exceeded", self.strategy.max_bump_attempts)
            ));
        }

        // Exponential fee bumping: 1.5x, 2.25x, 3.375x, etc.
        let bump_multiplier = 1.5_f64.powi(attempt as i32);
        let bumped_fee = (original_fee as f64 * bump_multiplier) as Lamports;
        
        // Ensure we don't exceed maximum price
        let final_fee = std::cmp::min(bumped_fee, self.strategy.max_price_lamports);

        info!(
            "Bumping fee: original={}, attempt={}, multiplier={:.2}, bumped={}, final={}",
            original_fee, attempt, bump_multiplier, bumped_fee, final_fee
        );

        Ok(final_fee)
    }

    /// Get statistics for monitoring
    pub async fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        stats.insert("base_percentile".to_string(), serde_json::Value::Number(self.strategy.base_percentile.into()));
        stats.insert("buffer_percent".to_string(), serde_json::Value::Number(self.strategy.buffer_percent.into()));
        stats.insert("adaptive_enabled".to_string(), serde_json::Value::Bool(self.strategy.adaptive));
        
        let fee_stats = self.get_fee_statistics();
        stats.insert("avg_fee".to_string(), serde_json::Value::Number(fee_stats.avg_fee.into()));
        stats.insert("sample_count".to_string(), serde_json::Value::Number(fee_stats.sample_count.into()));
        
        stats
    }
    
    /// Get current fee statistics
    pub fn get_fee_statistics(&self) -> FeeStatistics {
        let history = self.fee_history.read().unwrap();
        
        if history.is_empty() {
            return FeeStatistics::default();
        }

        let fees: Vec<Lamports> = history.iter().map(|point| point.fee_lamports).collect();
        let mut sorted_fees = fees.clone();
        sorted_fees.sort_unstable();

        let min_fee = *sorted_fees.first().unwrap_or(&0);
        let max_fee = *sorted_fees.last().unwrap_or(&0);
        let median_fee = sorted_fees[sorted_fees.len() / 2];
        let avg_fee = fees.iter().sum::<Lamports>() / fees.len() as Lamports;

        FeeStatistics {
            min_fee,
            max_fee,
            median_fee,
            avg_fee,
            sample_count: fees.len(),
            last_updated: history.back().map(|point| point.timestamp),
        }
    }

    /// Clear fee history (useful for testing)
    pub fn clear_history(&self) {
        let mut history = self.fee_history.write().unwrap();
        history.clear();
    }
}

/// Fee statistics for monitoring and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStatistics {
    pub min_fee: Lamports,
    pub max_fee: Lamports,
    pub median_fee: Lamports,
    pub avg_fee: Lamports,
    pub sample_count: usize,
    pub last_updated: Option<DateTime<Utc>>,
}

impl Default for FeeStatistics {
    fn default() -> Self {
        Self {
            min_fee: 0,
            max_fee: 0,
            median_fee: 0,
            avg_fee: 0,
            sample_count: 0,
            last_updated: None,
        }
    }
}

/// Fee calculation context for more sophisticated fee strategies
#[derive(Debug, Clone)]
pub struct FeeContext {
    pub accounts: Vec<Pubkey>,
    pub compute_units: u32,
    pub priority: FeePriority,
    pub max_acceptable_fee: Option<Lamports>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeePriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl FeePriority {
    pub fn multiplier(&self) -> f64 {
        match self {
            FeePriority::Low => 0.8,
            FeePriority::Normal => 1.0,
            FeePriority::High => 1.5,
            FeePriority::Urgent => 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bundler_config::BundlerConfigBuilder;
    use std::sync::Arc;

    fn create_test_fee_manager() -> (FeeManager, Arc<SolanaRpcClient>) {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .build()
            .unwrap();
        
        let rpc_client = Arc::new(SolanaRpcClient::new(&config).unwrap());
        let fee_manager = FeeManager::new(Arc::clone(&rpc_client), FeeStrategy::default());
        
        (fee_manager, rpc_client)
    }

    #[test]
    fn test_percentile_fee_calculation() {
        let (fee_manager, _) = create_test_fee_manager();
        
        let fees = vec![100, 200, 300, 400, 500];
        let p75_fee = fee_manager.calculate_percentile_fee(&fees).unwrap();
        
        // P75 of [100, 200, 300, 400, 500] should be 400
        assert_eq!(p75_fee, 400);
    }

    #[test]
    fn test_buffer_application() {
        let (fee_manager, _) = create_test_fee_manager();
        
        let base_fee = 1000;
        let buffered_fee = fee_manager.apply_buffer(base_fee);
        
        // With default 10% buffer: 1000 * 1.1 = 1100
        assert_eq!(buffered_fee, 1100);
    }

    #[test]
    fn test_fee_bumping() {
        let (fee_manager, _) = create_test_fee_manager();
        
        let original_fee = 1000;
        
        // First bump: 1000 * 1.5 = 1500
        let bump1 = tokio_test::block_on(fee_manager.bump_fee(original_fee, 1)).unwrap();
        assert_eq!(bump1, 1500);
        
        // Second bump: 1000 * 1.5^2 = 2250
        let bump2 = tokio_test::block_on(fee_manager.bump_fee(original_fee, 2)).unwrap();
        assert_eq!(bump2, 2250);
    }

    #[test]
    fn test_linear_regression() {
        let (fee_manager, _) = create_test_fee_manager();
        
        // Test with increasing values
        let increasing_values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (slope, r_squared) = fee_manager.calculate_linear_regression(&increasing_values);
        
        assert!(slope > 0.0); // Should have positive slope
        assert!(r_squared > 0.9); // Should have high R-squared for perfect line
    }

    #[test]
    fn test_compute_budget_instructions() {
        let (fee_manager, _) = create_test_fee_manager();
        
        let instructions = fee_manager.create_compute_budget_instructions(200_000, 5000);
        
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].program_id, solana_sdk::compute_budget::id());
        assert_eq!(instructions[1].program_id, solana_sdk::compute_budget::id());
    }

    #[tokio::test]
    async fn test_fee_history_management() {
        let (fee_manager, _) = create_test_fee_manager();
        
        // Record several fee calculations
        for i in 1..=5 {
            fee_manager.record_fee_calculation(i * 1000).await;
        }
        
        let stats = fee_manager.get_fee_statistics();
        assert_eq!(stats.sample_count, 5);
        assert_eq!(stats.min_fee, 1000);
        assert_eq!(stats.max_fee, 5000);
    }

    #[test]
    fn test_fee_priority_multipliers() {
        assert_eq!(FeePriority::Low.multiplier(), 0.8);
        assert_eq!(FeePriority::Normal.multiplier(), 1.0);
        assert_eq!(FeePriority::High.multiplier(), 1.5);
        assert_eq!(FeePriority::Urgent.multiplier(), 2.0);
    }
}
