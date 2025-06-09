use crate::hyperliquid_action;
use alloy::primitives::{Address, B256};
use serde;

// User Actions (with HyperliquidTransaction: prefix)

hyperliquid_action! {
    /// USD transfer action
    struct UsdSend {
        pub signature_chain_id: u64,
        pub hyperliquid_chain: String,
        pub destination: String,
        pub amount: String,
        pub time: u64,
    }
    => "UsdSend(string hyperliquidChain,string destination,string amount,uint64 time)"
}

impl UsdSend {
    fn chain_id(&self) -> Option<u64> {
        Some(self.signature_chain_id)
    }
}

hyperliquid_action! {
    /// Withdraw from bridge action
    struct Withdraw {
        pub signature_chain_id: u64,
        pub hyperliquid_chain: String,
        pub destination: String,
        pub amount: String,
        pub time: u64,
    }
    => "Withdraw(string hyperliquidChain,string destination,string amount,uint64 time)"
}

hyperliquid_action! {
    /// Spot token transfer action
    struct SpotSend {
        pub signature_chain_id: u64,
        pub hyperliquid_chain: String,
        pub destination: String,
        pub token: String,
        pub amount: String,
        pub time: u64,
    }
    => "SpotSend(string hyperliquidChain,string destination,string token,string amount,uint64 time)"
}

hyperliquid_action! {
    /// Approve an agent for trading
    struct ApproveAgent {
        pub signature_chain_id: u64,
        pub hyperliquid_chain: String,
        pub agent_address: Address,
        pub agent_name: Option<String>,
        pub nonce: u64,
    }
    => "ApproveAgent(string hyperliquidChain,address agentAddress,string agentName,uint64 nonce)"
}

hyperliquid_action! {
    /// Approve builder fee
    struct ApproveBuilderFee {
        pub signature_chain_id: u64,
        pub hyperliquid_chain: String,
        pub max_fee_rate: String,
        pub builder: String,
        pub nonce: u64,
    }
    => "ApproveBuilderFee(string hyperliquidChain,string maxFeeRate,string builder,uint64 nonce)"
}

// L1 Actions (without HyperliquidTransaction: prefix)

hyperliquid_action! {
    /// Agent connection action
    struct Agent {
        pub source: String,
        pub connection_id: B256,
    }
    => "Agent(string source,bytes32 connectionId)", no_prefix
}

// Exchange Actions (these don't need EIP-712 signing but are included for completeness)

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLeverage {
    pub asset: u32,
    pub is_cross: bool,
    pub leverage: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIsolatedMargin {
    pub asset: u32,
    pub is_buy: bool,
    pub ntli: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultTransfer {
    pub vault_address: Address,
    pub is_deposit: bool,
    pub usd: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotUser {
    pub class_transfer: ClassTransfer,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassTransfer {
    pub usdc: u64,
    pub to_perp: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetReferrer {
    pub code: String,
}

// Bulk actions that contain other types

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOrder {
    pub orders: Vec<OrderRequest>,
    pub grouping: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder: Option<BuilderInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCancel {
    pub cancels: Vec<CancelRequest>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkModify {
    pub modifies: Vec<ModifyRequest>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCancelCloid {
    pub cancels: Vec<CancelRequestCloid>,
}

// Supporting types (these will be defined in other files)
// For now, creating placeholder types

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRequest {
    // Placeholder - will be defined in requests.rs
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    // Placeholder - will be defined in requests.rs
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyRequest {
    // Placeholder - will be defined in requests.rs
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequestCloid {
    // Placeholder - will be defined in requests.rs
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderInfo {
    // Placeholder - will be defined in common.rs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::eip712::HyperliquidAction;
    use alloy::primitives::keccak256;
    
    #[test]
    fn test_usd_send_type_hash() {
        let expected = keccak256("HyperliquidTransaction:UsdSend(string hyperliquidChain,string destination,string amount,uint64 time)");
        assert_eq!(UsdSend::type_hash(), expected);
    }
    
    #[test]
    fn test_agent_type_hash() {
        let expected = keccak256("Agent(string source,bytes32 connectionId)");
        assert_eq!(Agent::type_hash(), expected);
    }
}
