/// Trading behavior patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use adnet_testbot_integration::AdnetClient;
use serde::{Deserialize, Serialize};

/// PT-L-020: Spot Market Order
///
/// Place and execute a market order on DEX via `dex.delta/place_order`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotMarketOrder {
    pub pair: String,
    pub side: OrderSide,
    pub amount: String,
    /// Delta private key (`ap1...`) used to sign the transaction.
    pub private_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
    }
}

impl SpotMarketOrder {
    pub fn new(pair: String, side: OrderSide, amount: String, private_key: String) -> Self {
        Self {
            pair,
            side,
            amount,
            private_key,
        }
    }

    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} executing market order: {:?} {} {}",
            context.execution.bot_id,
            self.side,
            self.amount,
            self.pair
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        // Build dex.delta/place_order inputs:
        //   trader (address derived from key — pass key, server derives address)
        //   market (string)
        //   side   (buy | sell)
        //   order_type (market)
        //   quantity (u64 encoded as string)
        let quantity: u64 = self.amount.parse().unwrap_or(0);
        let inputs = vec![
            self.pair.clone(),              // market
            self.side.as_str().to_string(), // side
            "market".to_string(),           // order_type
            quantity.to_string(),           // quantity
        ];

        let tx_id = client
            .execute_delta_transaction("dex.delta", "place_order", inputs, &self.private_key, 0)
            .await?;

        tracing::info!(
            "Bot {} market order submitted: tx_id={}",
            context.execution.bot_id,
            tx_id
        );

        Ok(BehaviorResult::success(format!(
            "Market order executed: {:?} {} {} — tx_id={}",
            self.side, self.amount, self.pair, tx_id
        ))
        .with_data(serde_json::json!({ "transaction_id": tx_id })))
    }
}

/// PT-L-021: Limit Order Lifecycle
///
/// Place limit order, wait for partial fill (simulated), then cancel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitOrderLifecycle {
    pub pair: String,
    pub side: OrderSide,
    pub amount: String,
    pub price: String,
    /// Delta private key (`ap1...`) used to sign the transaction.
    pub private_key: String,
}

impl LimitOrderLifecycle {
    pub fn new(
        pair: String,
        side: OrderSide,
        amount: String,
        price: String,
        private_key: String,
    ) -> Self {
        Self {
            pair,
            side,
            amount,
            price,
            private_key,
        }
    }

    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} executing limit order lifecycle",
            context.execution.bot_id
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        // Step 1: Place limit order via dex.delta/place_order
        tracing::debug!("Placing limit order: {} @ {}", self.amount, self.price);

        let quantity: u64 = self.amount.parse().unwrap_or(0);
        let price: u64 = self.price.parse().unwrap_or(0);
        let inputs = vec![
            self.pair.clone(),              // market
            self.side.as_str().to_string(), // side
            "limit".to_string(),            // order_type
            quantity.to_string(),           // quantity
            price.to_string(),              // price
        ];

        let order_tx_id = client
            .execute_delta_transaction("dex.delta", "place_order", inputs, &self.private_key, 0)
            .await?;

        tracing::info!(
            "Bot {} limit order placed: tx_id={}",
            context.execution.bot_id,
            order_tx_id
        );

        // Step 2: Simulate waiting for partial fill (2s max in test context)
        tracing::debug!("Waiting for partial fill (2s)...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 3: Cancel the remaining order
        tracing::debug!("Canceling remaining order: {}", order_tx_id);

        client
            .cancel_delta_order(&self.private_key, &order_tx_id)
            .await?;

        tracing::info!(
            "Bot {} limit order cancelled: tx_id={}",
            context.execution.bot_id,
            order_tx_id
        );

        Ok(
            BehaviorResult::success("Limit order lifecycle completed").with_data(
                serde_json::json!({
                    "order_tx_id": order_tx_id,
                    "status": "cancelled",
                }),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_order_creation() {
        let behavior = SpotMarketOrder::new(
            "AX/DX".to_string(),
            OrderSide::Buy,
            "100".to_string(),
            "ap1test".to_string(),
        );
        assert_eq!(behavior.pair, "AX/DX");
        assert_eq!(behavior.private_key, "ap1test");
    }

    #[test]
    fn test_limit_order_creation() {
        let behavior = LimitOrderLifecycle::new(
            "AX/DX".to_string(),
            OrderSide::Sell,
            "50".to_string(),
            "1000".to_string(),
            "ap1test".to_string(),
        );
        assert_eq!(behavior.pair, "AX/DX");
        assert_eq!(behavior.price, "1000");
        assert_eq!(behavior.private_key, "ap1test");
    }

    #[test]
    fn test_order_side_as_str() {
        assert_eq!(OrderSide::Buy.as_str(), "buy");
        assert_eq!(OrderSide::Sell.as_str(), "sell");
    }
}
