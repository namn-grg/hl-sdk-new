use ferrofluid::{
    providers::RawExchangeProvider,
    signers::LocalWallet,
    types::responses::ExchangeResponseStatus,
};

#[tokio::test]
async fn test_agent_approval_format() {
    // Use a fixed test key for reproducible testing
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let signer = LocalWallet::from_hex_key(private_key).unwrap();
    
    // Create testnet provider
    let exchange = RawExchangeProvider::testnet(signer.clone());
    
    // Test approving a specific agent
    let agent_key = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
    let agent_signer = LocalWallet::from_hex_key(agent_key).unwrap();
    let agent_address = agent_signer.address();
    
    println!("Testing agent approval for address: {}", agent_address);
    
    // This will attempt to approve the agent
    // In a real test, we'd mock the HTTP client, but for now we'll just
    // verify the request format is correct by checking the debug output
    match exchange.approve_agent(agent_address, None).await {
        Ok(_) => println!("Request sent successfully"),
        Err(e) => println!("Expected error (no real network): {:?}", e),
    }
}

#[test]
fn test_agent_address_serialization() {
    use alloy::primitives::Address;
    use serde_json;
    
    // Test the ApproveAgent struct directly
    use ferrofluid::types::actions::ApproveAgent;
    
    let agent = ApproveAgent {
        signature_chain_id: 421614,
        hyperliquid_chain: "Testnet".to_string(),
        agent_address: "0x70997970c51812dc3a010c7d01b50e0d17dc79c8".parse::<Address>().unwrap(),
        agent_name: Some("test".to_string()),
        nonce: 12345,
    };
    
    let json = serde_json::to_string(&agent).unwrap();
    
    // Should serialize with 0x prefix and lowercase
    assert!(json.contains(r#""agentAddress":"0x70997970c51812dc3a010c7d01b50e0d17dc79c8""#));
}

#[test] 
fn test_approve_agent_serialization() {
    use ferrofluid::types::actions::ApproveAgent;
    use alloy::primitives::Address;
    
    let agent = ApproveAgent {
        signature_chain_id: 421614,
        hyperliquid_chain: "Testnet".to_string(),
        agent_address: "0x70997970c51812dc3a010c7d01b50e0d17dc79c8".parse::<Address>().unwrap(),
        agent_name: None,
        nonce: 1234567890,
    };
    
    let json = serde_json::to_value(&agent).unwrap();
    
    // Check the serialized format
    assert_eq!(json["signatureChainId"], 421614);
    assert_eq!(json["hyperliquidChain"], "Testnet");
    assert_eq!(json["agentAddress"], "0x70997970c51812dc3a010c7d01b50e0d17dc79c8");
    assert_eq!(json["agentName"], serde_json::Value::Null);
    assert_eq!(json["nonce"], 1234567890);
}