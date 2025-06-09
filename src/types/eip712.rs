use alloy::primitives::{keccak256, B256, U256};
use alloy::sol_types::Eip712Domain;

pub trait HyperliquidAction: Sized + serde::Serialize {
    /// The EIP-712 type string (without HyperliquidTransaction: prefix)
    const TYPE_STRING: &'static str;
    
    /// Whether this uses the HyperliquidTransaction: prefix
    const USE_PREFIX: bool = true;
    
    /// Get chain ID for domain construction (if applicable)
    fn chain_id(&self) -> Option<u64> {
        None
    }
    
    /// Get the EIP-712 domain for this action
    fn domain(&self) -> Eip712Domain {
        let chain_id = self.chain_id().unwrap_or(1); // Default to mainnet
        alloy::sol_types::eip712_domain! {
            name: "HyperliquidSignTransaction",
            version: "1",
            chain_id: chain_id,
            verifying_contract: alloy::primitives::address!("0000000000000000000000000000000000000000"),
        }
    }
    
    fn type_hash() -> B256 {
        let type_string = if Self::USE_PREFIX {
            format!("HyperliquidTransaction:{}", Self::TYPE_STRING)
        } else {
            Self::TYPE_STRING.to_string()
        };
        keccak256(type_string.as_bytes())
    }
    
    fn encode_data(&self) -> Vec<u8> {
        // Generic encoding using the struct's fields
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&Self::type_hash()[..]);
        
        // Use serde to get field values, then encode each
        let mut json = serde_json::to_value(self).unwrap();
        if let serde_json::Value::Object(ref mut map) = json {
            // Remove signature_chain_id from encoding - it's only for domain
            map.remove("signatureChainId");
            
            // Note: This is a simplified version - in practice, you'd need to
            // ensure fields are encoded in the correct order as specified in TYPE_STRING
            for (_, value) in map {
                encoded.extend_from_slice(&encode_field(&value)[..]);
            }
        }
        encoded
    }
    
    fn struct_hash(&self) -> B256 {
        keccak256(&self.encode_data())
    }
    
    fn eip712_signing_hash(&self, domain: &Eip712Domain) -> B256 {
        let mut buf = Vec::with_capacity(66);
        buf.push(0x19);
        buf.push(0x01);
        buf.extend_from_slice(&domain.separator()[..]);
        buf.extend_from_slice(&self.struct_hash()[..]);
        keccak256(&buf)
    }
}

fn encode_field(value: &serde_json::Value) -> [u8; 32] {
    match value {
        serde_json::Value::String(s) => keccak256(s.as_bytes()).into(),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                U256::from(u).to_be_bytes::<32>()
            } else {
                [0u8; 32]
            }
        }
        _ => [0u8; 32],
    }
}
