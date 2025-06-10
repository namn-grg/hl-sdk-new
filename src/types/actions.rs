use crate::{hyperliquid_action, l1_action};
use crate::types::requests::{OrderRequest, CancelRequest, CancelRequestCloid, ModifyRequest, BuilderInfo};
use alloy::primitives::B256;
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
    => encode(hyperliquid_chain, destination, amount, time)
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
    => encode(hyperliquid_chain, destination, amount, time)
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
    => encode(hyperliquid_chain, destination, token, amount, time)
}

hyperliquid_action! {
    /// Approve an agent for trading
    struct ApproveAgent {
        pub signature_chain_id: u64,
        pub hyperliquid_chain: String,
        pub agent_address: String,
        pub agent_name: Option<String>,
        pub nonce: u64,
    }
    => "ApproveAgent(string hyperliquidChain,address agentAddress,string agentName,uint64 nonce)"
    => encode(hyperliquid_chain, agent_address, agent_name, nonce)
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
    => encode(hyperliquid_chain, max_fee_rate, builder, nonce)
}

// L1 Actions (use Exchange domain)

l1_action! {
    /// Agent connection action
    struct Agent {
        pub source: String,
        pub connection_id: B256,
    }
    => "Agent(string source,bytes32 connectionId)"
    => encode(source, connection_id)
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
    pub vault_address: String,
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
    pub usd_size: u64,
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

// Types are now imported from requests.rs

// The macros don't handle signature_chain_id, so we need to remove the duplicate trait impls

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
        // L1 actions don't use the HyperliquidTransaction: prefix
        let expected = keccak256("Agent(string source,bytes32 connectionId)");
        assert_eq!(Agent::type_hash(), expected);
    }
    
    #[test]
    fn test_agent_domain() {
        let agent = Agent {
            source: "a".to_string(),
            connection_id: B256::default(),
        };
        
        // L1 actions use the Exchange domain
        let domain = agent.domain();
        let expected_domain = alloy::sol_types::eip712_domain! {
            name: "Exchange",
            version: "1",
            chain_id: 1337u64,
            verifying_contract: alloy::primitives::address!("0000000000000000000000000000000000000000"),
        };
        
        // Compare domain separators to verify they're the same
        assert_eq!(domain.separator(), expected_domain.separator());
    }
}
