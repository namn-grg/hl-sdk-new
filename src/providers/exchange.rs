use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy::primitives::{Address, B256, keccak256};
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, body::Bytes};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    constants::*,
    errors::HyperliquidError,
    signers::{HyperliquidSignature, HyperliquidSigner},
    types::{
        actions::*, eip712::HyperliquidAction, requests::*,
        responses::ExchangeResponseStatus,
    },
};

type Result<T> = std::result::Result<T, HyperliquidError>;

pub struct ExchangeProvider<S: HyperliquidSigner> {
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    endpoint: &'static str,
    rate_limiter: Arc<crate::providers::info::RateLimiter>,
    signer: S,
    vault_address: Option<Address>,
    agent: Option<Address>,
    builder: Option<Address>,
}

impl<S: HyperliquidSigner> ExchangeProvider<S> {
    // ==================== Helper Methods ====================

    fn infer_network(&self) -> (u64, &'static str) {
        if self.endpoint.contains("testnet") {
            (CHAIN_ID_TESTNET, AGENT_SOURCE_TESTNET)
        } else {
            (CHAIN_ID_MAINNET, AGENT_SOURCE_MAINNET)
        }
    }

    /// Get the configured builder address
    pub fn builder(&self) -> Option<Address> {
        self.builder
    }

    // ==================== Constructors ====================

    pub fn mainnet(signer: S) -> Self {
        Self::new(signer, EXCHANGE_ENDPOINT_MAINNET, None, None, None)
    }

    pub fn testnet(signer: S) -> Self {
        Self::new(signer, EXCHANGE_ENDPOINT_TESTNET, None, None, None)
    }

    pub fn mainnet_vault(signer: S, vault_address: Address) -> Self {
        Self::new(
            signer,
            EXCHANGE_ENDPOINT_MAINNET,
            Some(vault_address),
            None,
            None,
        )
    }

    pub fn testnet_vault(signer: S, vault_address: Address) -> Self {
        Self::new(
            signer,
            EXCHANGE_ENDPOINT_TESTNET,
            Some(vault_address),
            None,
            None,
        )
    }

    pub fn mainnet_agent(signer: S, agent_address: Address) -> Self {
        Self::new(
            signer,
            EXCHANGE_ENDPOINT_MAINNET,
            None,
            Some(agent_address),
            None,
        )
    }

    pub fn testnet_agent(signer: S, agent_address: Address) -> Self {
        Self::new(
            signer,
            EXCHANGE_ENDPOINT_TESTNET,
            None,
            Some(agent_address),
            None,
        )
    }

    // New builder-specific constructors
    pub fn mainnet_builder(signer: S, builder_address: Address) -> Self {
        Self::new(
            signer,
            EXCHANGE_ENDPOINT_MAINNET,
            None,
            None,
            Some(builder_address),
        )
    }

    pub fn testnet_builder(signer: S, builder_address: Address) -> Self {
        Self::new(
            signer,
            EXCHANGE_ENDPOINT_TESTNET,
            None,
            None,
            Some(builder_address),
        )
    }

    // Combined constructors
    pub fn mainnet_with_options(
        signer: S,
        vault: Option<Address>,
        agent: Option<Address>,
        builder: Option<Address>,
    ) -> Self {
        Self::new(signer, EXCHANGE_ENDPOINT_MAINNET, vault, agent, builder)
    }

    pub fn testnet_with_options(
        signer: S,
        vault: Option<Address>,
        agent: Option<Address>,
        builder: Option<Address>,
    ) -> Self {
        Self::new(signer, EXCHANGE_ENDPOINT_TESTNET, vault, agent, builder)
    }

    fn new(
        signer: S,
        endpoint: &'static str,
        vault_address: Option<Address>,
        agent: Option<Address>,
        builder: Option<Address>,
    ) -> Self {
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .unwrap()
            .https_only()
            .enable_http1()
            .build();
        let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(https);
        let rate_limiter = Arc::new(crate::providers::info::RateLimiter::new(
            RATE_LIMIT_MAX_TOKENS,
            RATE_LIMIT_REFILL_RATE,
        ));

        Self {
            client,
            endpoint,
            rate_limiter,
            signer,
            vault_address,
            agent,
            builder,
        }
    }

    // ==================== Direct Order Operations ====================

    pub async fn place_order(
        &self,
        order: &OrderRequest,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_PLACE_ORDER)?;

        let bulk_order = BulkOrder {
            orders: vec![order.clone()],
            grouping: "na".to_string(),
            builder: self.builder.map(|addr| BuilderInfo {
                builder: format!("0x{}", hex::encode(addr)),
                fee: 0, // Default fee, use place_order_with_builder_fee to specify
            }),
        };

        self.send_l1_action("order", &bulk_order).await
    }

    pub async fn place_order_with_builder_fee(
        &self,
        order: &OrderRequest,
        builder_fee: u64,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_PLACE_ORDER)?;

        let bulk_order = BulkOrder {
            orders: vec![order.clone()],
            grouping: "na".to_string(),
            builder: self.builder.map(|addr| BuilderInfo {
                builder: format!("0x{}", hex::encode(addr)),
                fee: builder_fee,
            }),
        };

        self.send_l1_action("order", &bulk_order).await
    }

    pub async fn place_order_with_cloid(
        &self,
        mut order: OrderRequest,
        cloid: Uuid,
    ) -> Result<ExchangeResponseStatus> {
        order = order.with_cloid(Some(cloid));
        self.place_order(&order).await
    }

    pub async fn cancel_order(
        &self,
        asset: u32,
        oid: u64,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_CANCEL_ORDER)?;

        let bulk_cancel = BulkCancel {
            cancels: vec![CancelRequest { asset, oid }],
        };

        self.send_l1_action("cancel", &bulk_cancel).await
    }

    pub async fn cancel_order_by_cloid(
        &self,
        asset: u32,
        cloid: Uuid,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_CANCEL_ORDER)?;

        let bulk_cancel = BulkCancelCloid {
            cancels: vec![CancelRequestCloid::new(asset, cloid)],
        };

        self.send_l1_action("cancelByCloid", &bulk_cancel).await
    }

    pub async fn modify_order(
        &self,
        oid: u64,
        new_order: OrderRequest,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_MODIFY_ORDER)?;

        let bulk_modify = BulkModify {
            modifies: vec![ModifyRequest {
                oid,
                order: new_order,
            }],
        };

        self.send_l1_action("batchModify", &bulk_modify).await
    }

    // ==================== Bulk Operations ====================

    pub async fn bulk_orders(
        &self,
        orders: Vec<OrderRequest>,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_BULK_ORDER)?;

        let bulk_order = BulkOrder {
            orders,
            grouping: "na".to_string(),
            builder: self.builder.map(|addr| BuilderInfo {
                builder: format!("0x{}", hex::encode(addr)),
                fee: 0, // Default fee, use bulk_orders_with_builder_fee to specify
            }),
        };

        self.send_l1_action("order", &bulk_order).await
    }

    pub async fn bulk_orders_with_builder_fee(
        &self,
        orders: Vec<OrderRequest>,
        builder_fee: u64,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_BULK_ORDER)?;

        let bulk_order = BulkOrder {
            orders,
            grouping: "na".to_string(),
            builder: self.builder.map(|addr| BuilderInfo {
                builder: format!("0x{}", hex::encode(addr)),
                fee: builder_fee,
            }),
        };

        self.send_l1_action("order", &bulk_order).await
    }

    pub async fn bulk_orders_with_cloids(
        &self,
        orders: Vec<(OrderRequest, Option<Uuid>)>,
    ) -> Result<ExchangeResponseStatus> {
        let orders = orders
            .into_iter()
            .map(|(order, cloid)| order.with_cloid(cloid))
            .collect();

        self.bulk_orders(orders).await
    }

    pub async fn bulk_cancel(
        &self,
        cancels: Vec<CancelRequest>,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_BULK_CANCEL)?;

        let bulk_cancel = BulkCancel { cancels };
        self.send_l1_action("cancel", &bulk_cancel).await
    }

    pub async fn bulk_cancel_by_cloid(
        &self,
        cancels: Vec<CancelRequestCloid>,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_BULK_CANCEL)?;

        let bulk_cancel = BulkCancelCloid { cancels };
        self.send_l1_action("cancelByCloid", &bulk_cancel).await
    }

    pub async fn bulk_modify(
        &self,
        modifies: Vec<ModifyRequest>,
    ) -> Result<ExchangeResponseStatus> {
        self.rate_limiter.check_weight(WEIGHT_BULK_ORDER)?;

        let bulk_modify = BulkModify { modifies };
        self.send_l1_action("batchModify", &bulk_modify).await
    }

    // ==================== Account Management ====================

    pub async fn update_leverage(
        &self,
        asset: u32,
        is_cross: bool,
        leverage: u32,
    ) -> Result<ExchangeResponseStatus> {
        let update = UpdateLeverage {
            asset,
            is_cross,
            leverage,
        };
        self.send_l1_action("updateLeverage", &update).await
    }

    pub async fn update_isolated_margin(
        &self,
        asset: u32,
        is_buy: bool,
        ntli: i64,
    ) -> Result<ExchangeResponseStatus> {
        let update = UpdateIsolatedMargin {
            asset,
            is_buy,
            ntli,
        };
        self.send_l1_action("updateIsolatedMargin", &update).await
    }

    pub async fn set_referrer(&self, code: String) -> Result<ExchangeResponseStatus> {
        let referrer = SetReferrer { code };
        self.send_l1_action("setReferrer", &referrer).await
    }

    // ==================== User Actions (EIP-712) ====================

    pub async fn usd_transfer(
        &self,
        destination: Address,
        amount: &str,
    ) -> Result<ExchangeResponseStatus> {
        let (chain_id, _) = self.infer_network();
        let chain = if chain_id == CHAIN_ID_MAINNET {
            "Mainnet"
        } else {
            "Testnet"
        };

        let action = UsdSend {
            signature_chain_id: chain_id,
            hyperliquid_chain: chain.to_string(),
            destination: format!("0x{}", hex::encode(destination)),
            amount: amount.to_string(),
            time: Self::current_nonce(),
        };

        self.send_user_action(&action).await
    }

    pub async fn withdraw(
        &self,
        destination: Address,
        amount: &str,
    ) -> Result<ExchangeResponseStatus> {
        let (chain_id, _) = self.infer_network();
        let chain = if chain_id == CHAIN_ID_MAINNET {
            "Mainnet"
        } else {
            "Testnet"
        };

        let action = Withdraw {
            signature_chain_id: chain_id,
            hyperliquid_chain: chain.to_string(),
            destination: format!("0x{}", hex::encode(destination)),
            amount: amount.to_string(),
            time: Self::current_nonce(),
        };

        self.send_user_action(&action).await
    }

    pub async fn spot_transfer(
        &self,
        destination: Address,
        token: &str,
        amount: &str,
    ) -> Result<ExchangeResponseStatus> {
        let (chain_id, _) = self.infer_network();
        let chain = if chain_id == CHAIN_ID_MAINNET {
            "Mainnet"
        } else {
            "Testnet"
        };

        let action = SpotSend {
            signature_chain_id: chain_id,
            hyperliquid_chain: chain.to_string(),
            destination: format!("0x{}", hex::encode(destination)),
            token: token.to_string(),
            amount: amount.to_string(),
            time: Self::current_nonce(),
        };

        self.send_user_action(&action).await
    }

    pub async fn approve_agent(
        &self,
        agent_address: Address,
        agent_name: Option<String>,
    ) -> Result<ExchangeResponseStatus> {
        let (chain_id, _) = self.infer_network();
        let chain = if chain_id == CHAIN_ID_MAINNET {
            "Mainnet"
        } else {
            "Testnet"
        };

        let action = ApproveAgent {
            signature_chain_id: chain_id,
            hyperliquid_chain: chain.to_string(),
            agent_address: format!("0x{}", hex::encode(agent_address)),
            agent_name,
            nonce: Self::current_nonce(),
        };

        self.send_user_action(&action).await
    }

    pub async fn approve_builder_fee(
        &self,
        builder: Address,
        max_fee_rate: String,
    ) -> Result<ExchangeResponseStatus> {
        let (chain_id, _) = self.infer_network();
        let chain = if chain_id == CHAIN_ID_MAINNET {
            "Mainnet"
        } else {
            "Testnet"
        };

        let action = ApproveBuilderFee {
            signature_chain_id: chain_id,
            hyperliquid_chain: chain.to_string(),
            builder: format!("0x{}", hex::encode(builder)),
            max_fee_rate,
            nonce: Self::current_nonce(),
        };

        self.send_user_action(&action).await
    }

    // ==================== Vault Operations ====================

    pub async fn vault_transfer(
        &self,
        vault_address: Address,
        is_deposit: bool,
        usd: u64,
    ) -> Result<ExchangeResponseStatus> {
        let transfer = VaultTransfer {
            vault_address: format!("0x{}", hex::encode(vault_address)),
            is_deposit,
            usd,
        };

        self.send_l1_action("vaultTransfer", &transfer).await
    }

    // ==================== Spot Operations ====================

    pub async fn spot_transfer_to_perp(
        &self,
        usd_size: u64,
        to_perp: bool,
    ) -> Result<ExchangeResponseStatus> {
        let transfer = ClassTransfer { usd_size, to_perp };

        let spot_user = SpotUser {
            class_transfer: transfer,
        };

        self.send_l1_action("spotUser", &spot_user).await
    }

    // ==================== Helper Methods ====================

    fn current_nonce() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn hash_action<T: Serialize>(
        action_type: &str,
        action: &T,
        timestamp: u64,
        vault_address: Option<Address>,
    ) -> Result<B256> {
        // Create the tagged action for hashing
        let mut tagged_action = serde_json::to_value(action)?;
        if let Value::Object(ref mut map) = tagged_action {
            map.insert("type".to_string(), json!(action_type));
        }

        // NOTE: Hyperliquid uses MessagePack (rmp_serde) for action serialization
        // This is different from typical EVM systems that use RLP
        let mut bytes = rmp_serde::to_vec_named(&tagged_action).map_err(|e| {
            HyperliquidError::InvalidRequest(format!("Failed to serialize action: {}", e))
        })?;
        bytes.extend(timestamp.to_be_bytes());
        if let Some(vault) = vault_address {
            bytes.push(1);
            bytes.extend(vault.as_slice());
        } else {
            bytes.push(0);
        }
        Ok(keccak256(bytes))
    }

    async fn send_l1_action<T: Serialize>(
        &self,
        action_type: &str,
        action: &T,
    ) -> Result<ExchangeResponseStatus> {
        let nonce = Self::current_nonce();
        let connection_id =
            Self::hash_action(action_type, action, nonce, self.vault_address)?;

        // Create Agent L1 action
        let (_, agent_source) = self.infer_network();
        let agent = Agent {
            source: agent_source.to_string(),
            connection_id,
        };

        // Sign using EIP-712
        let domain = agent.domain();
        let signing_hash = agent.eip712_signing_hash(&domain);
        let signature = self.signer.sign_hash(signing_hash).await?;

        // Build action value with type tag
        let mut action_value = serde_json::to_value(action)?;
        if let Value::Object(ref mut map) = action_value {
            map.insert("type".to_string(), json!(action_type));
        }

        // Wrap action if using agent
        let final_action = if let Some(agent_address) = &self.agent {
            let (_, agent_source) = self.infer_network();
            json!({
                "type": "agent",
                "agentAddress": agent_address,
                "agentAction": action_value,
                "source": agent_source,
            })
        } else {
            action_value
        };

        self.post(final_action, signature, nonce).await
    }

    async fn send_user_action<T: HyperliquidAction + Serialize>(
        &self,
        action: &T,
    ) -> Result<ExchangeResponseStatus> {
        let domain = action.domain();
        let signing_hash = action.eip712_signing_hash(&domain);
        let signature = self.signer.sign_hash(signing_hash).await?;

        // Get action type from type name
        // This extracts "UsdSend" from "ferrofluid::types::actions::UsdSend"
        let action_type = std::any::type_name::<T>()
            .split("::")
            .last()
            .unwrap_or("Unknown");

        // Get action value and extract nonce
        let mut action_value = serde_json::to_value(action)?;
        let nonce = action_value
            .get("time")
            .or_else(|| action_value.get("nonce"))
            .and_then(|v| v.as_u64())
            .unwrap_or_else(Self::current_nonce);

        // Add type tag
        if let Value::Object(ref mut map) = action_value {
            map.insert("type".to_string(), json!(action_type));
        }

        self.post(action_value, signature, nonce).await
    }

    async fn post(
        &self,
        action: Value,
        signature: HyperliquidSignature,
        nonce: u64,
    ) -> Result<ExchangeResponseStatus> {
        let sig_hex = format!(
            "{:064x}{:064x}{:02x}",
            signature.r, signature.s, signature.v
        );

        let payload = json!({
            "action": action,
            "signature": sig_hex,
            "nonce": nonce,
            "vaultAddress": self.vault_address,
        });

        let body = Full::new(Bytes::from(serde_json::to_vec(&payload)?));
        let request = Request::builder()
            .method(Method::POST)
            .uri(self.endpoint)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| HyperliquidError::Network(e.to_string()))?;

        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| HyperliquidError::Network(e.to_string()))?;
        let status = response.status();
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .map_err(|e| HyperliquidError::Network(e.to_string()))?
            .to_bytes();

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&body_bytes);
            return Err(HyperliquidError::Http {
                status: status.as_u16(),
                body: error_text.to_string(),
            });
        }

        serde_json::from_slice(&body_bytes).map_err(|e| {
            HyperliquidError::InvalidResponse(format!(
                "Failed to parse exchange response: {}",
                e
            ))
        })
    }
}

// ==================== OrderBuilder Pattern ====================

pub struct OrderBuilder<'a, S: HyperliquidSigner> {
    provider: &'a ExchangeProvider<S>,
    asset: u32,
    is_buy: Option<bool>,
    limit_px: Option<String>,
    sz: Option<String>,
    reduce_only: bool,
    order_type: Option<OrderType>,
    cloid: Option<Uuid>,
}

impl<'a, S: HyperliquidSigner> OrderBuilder<'a, S> {
    pub fn new(provider: &'a ExchangeProvider<S>, asset: u32) -> Self {
        Self {
            provider,
            asset,
            is_buy: None,
            limit_px: None,
            sz: None,
            reduce_only: false,
            order_type: None,
            cloid: None,
        }
    }

    pub fn buy(mut self) -> Self {
        self.is_buy = Some(true);
        self
    }

    pub fn sell(mut self) -> Self {
        self.is_buy = Some(false);
        self
    }

    pub fn limit_px(mut self, price: impl ToString) -> Self {
        self.limit_px = Some(price.to_string());
        self
    }

    pub fn size(mut self, size: impl ToString) -> Self {
        self.sz = Some(size.to_string());
        self
    }

    pub fn reduce_only(mut self, reduce: bool) -> Self {
        self.reduce_only = reduce;
        self
    }

    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = Some(order_type);
        self
    }

    pub fn cloid(mut self, id: Uuid) -> Self {
        self.cloid = Some(id);
        self
    }

    // Convenience methods for common order types
    pub fn limit_buy(self, price: impl ToString, size: impl ToString) -> Self {
        self.buy().limit_px(price).size(size)
    }

    pub fn limit_sell(self, price: impl ToString, size: impl ToString) -> Self {
        self.sell().limit_px(price).size(size)
    }

    pub fn trigger_buy(
        self,
        trigger_px: impl ToString,
        size: impl ToString,
        tpsl: &str,
    ) -> Self {
        self.buy()
            .size(size)
            .order_type(OrderType::Trigger(Trigger {
                trigger_px: trigger_px.to_string(),
                is_market: true,
                tpsl: tpsl.to_string(),
            }))
    }

    pub fn trigger_sell(
        self,
        trigger_px: impl ToString,
        size: impl ToString,
        tpsl: &str,
    ) -> Self {
        self.sell()
            .size(size)
            .order_type(OrderType::Trigger(Trigger {
                trigger_px: trigger_px.to_string(),
                is_market: true,
                tpsl: tpsl.to_string(),
            }))
    }

    pub fn build(self) -> Result<OrderRequest> {
        Ok(OrderRequest {
            asset: self.asset,
            is_buy: self.is_buy.ok_or(HyperliquidError::InvalidRequest(
                "is_buy must be specified".to_string(),
            ))?,
            limit_px: self.limit_px.ok_or(HyperliquidError::InvalidRequest(
                "limit_px must be specified".to_string(),
            ))?,
            sz: self.sz.ok_or(HyperliquidError::InvalidRequest(
                "sz must be specified".to_string(),
            ))?,
            reduce_only: self.reduce_only,
            order_type: self.order_type.unwrap_or(OrderType::Limit(Limit {
                tif: TIF_GTC.to_string(),
            })),
            cloid: self.cloid.map(|id| format!("{:032x}", id.as_u128())),
        })
    }

    pub async fn send(self) -> Result<ExchangeResponseStatus> {
        let provider = self.provider;
        let order = self.build()?;
        provider.place_order(&order).await
    }
}

impl<S: HyperliquidSigner> ExchangeProvider<S> {
    pub fn order(&self, asset: u32) -> OrderBuilder<S> {
        OrderBuilder::new(self, asset)
    }
}
