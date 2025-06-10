//! WebSocket provider for real-time market data and user events

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use dashmap::DashMap;
use fastwebsockets::{Frame, OpCode, Role, WebSocket, handshake};
use http_body_util::Empty;
use hyper::{Request, StatusCode, body::Bytes, header, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    Network,
    errors::HyperliquidError,
    types::ws::{Message, Subscription, WsRequest},
};

pub type SubscriptionId = u32;

#[derive(Clone)]
struct SubscriptionHandle {
    subscription: Subscription,
    tx: UnboundedSender<Message>,
}

/// WebSocket provider for Hyperliquid
///
/// This is a thin wrapper around fastwebsockets that provides:
/// - Type-safe subscriptions
/// - Simple message routing
/// - No automatic reconnection (user controls retry logic)
pub struct WsProvider {
    _network: Network,
    ws: Option<WebSocket<TokioIo<Upgraded>>>,
    subscriptions: Arc<DashMap<SubscriptionId, SubscriptionHandle>>,
    next_id: Arc<AtomicU32>,
    message_tx: Option<UnboundedSender<String>>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WsProvider {
    /// Connect to Hyperliquid WebSocket
    pub async fn connect(network: Network) -> Result<Self, HyperliquidError> {
        let url = match network {
            Network::Mainnet => "https://api.hyperliquid.xyz/ws",
            Network::Testnet => "https://api.hyperliquid-testnet.xyz/ws",
        };

        let ws = Self::establish_connection(url).await?;
        let subscriptions = Arc::new(DashMap::new());
        let next_id = Arc::new(AtomicU32::new(1));

        // Create message routing channel
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        // Spawn message routing task
        let subscriptions_clone = subscriptions.clone();
        let task_handle = tokio::spawn(async move {
            Self::message_router(message_rx, subscriptions_clone).await;
        });

        Ok(Self {
            _network: network,
            ws: Some(ws),
            subscriptions,
            next_id,
            message_tx: Some(message_tx),
            task_handle: Some(task_handle),
        })
    }

    async fn establish_connection(
        url: &str,
    ) -> Result<WebSocket<TokioIo<Upgraded>>, HyperliquidError> {
        use hyper_rustls::HttpsConnectorBuilder;
        use hyper_util::client::legacy::Client;

        let uri = url
            .parse::<hyper::Uri>()
            .map_err(|e| HyperliquidError::WebSocket(format!("Invalid URL: {}", e)))?;

        // Create HTTPS connector with proper configuration
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Failed to load native roots: {}", e))
            })?
            .https_only()
            .enable_http1()
            .build();

        let client = Client::builder(hyper_util::rt::TokioExecutor::new())
            .build::<_, Empty<Bytes>>(https);

        // Create WebSocket upgrade request
        let host = uri
            .host()
            .ok_or_else(|| HyperliquidError::WebSocket("No host in URL".to_string()))?;

        let req = Request::builder()
            .method("GET")
            .uri(&uri)
            .header(header::HOST, host)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, handshake::generate_key())
            .body(Empty::new())
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Request build failed: {}", e))
            })?;

        let res = client.request(req).await.map_err(|e| {
            HyperliquidError::WebSocket(format!("HTTP request failed: {}", e))
        })?;

        if res.status() != StatusCode::SWITCHING_PROTOCOLS {
            return Err(HyperliquidError::WebSocket(format!(
                "WebSocket upgrade failed: {}",
                res.status()
            )));
        }

        let upgraded = hyper::upgrade::on(res)
            .await
            .map_err(|e| HyperliquidError::WebSocket(format!("Upgrade failed: {}", e)))?;

        Ok(WebSocket::after_handshake(
            TokioIo::new(upgraded),
            Role::Client,
        ))
    }

    /// Subscribe to L2 order book updates
    pub async fn subscribe_l2_book(
        &mut self,
        coin: &str,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let subscription = Subscription::L2Book {
            coin: coin.to_string(),
        };
        self.subscribe(subscription).await
    }

    /// Subscribe to trades
    pub async fn subscribe_trades(
        &mut self,
        coin: &str,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let subscription = Subscription::Trades {
            coin: coin.to_string(),
        };
        self.subscribe(subscription).await
    }

    /// Subscribe to all mid prices
    pub async fn subscribe_all_mids(
        &mut self,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        self.subscribe(Subscription::AllMids).await
    }

    /// Generic subscription method
    pub async fn subscribe(
        &mut self,
        subscription: Subscription,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| HyperliquidError::WebSocket("Not connected".to_string()))?;

        // Send subscription request
        let request = WsRequest::subscribe(subscription.clone());
        let payload = serde_json::to_string(&request)
            .map_err(|e| HyperliquidError::Serialize(e.to_string()))?;

        ws.write_frame(Frame::text(payload.into_bytes().into()))
            .await
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Failed to send subscription: {}", e))
            })?;

        // Create channel for this subscription
        let (tx, rx) = mpsc::unbounded_channel();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        self.subscriptions
            .insert(id, SubscriptionHandle { subscription, tx });

        Ok((id, rx))
    }

    /// Unsubscribe from a subscription
    pub async fn unsubscribe(
        &mut self,
        id: SubscriptionId,
    ) -> Result<(), HyperliquidError> {
        if let Some((_, handle)) = self.subscriptions.remove(&id) {
            let ws = self.ws.as_mut().ok_or_else(|| {
                HyperliquidError::WebSocket("Not connected".to_string())
            })?;

            let request = WsRequest::unsubscribe(handle.subscription);
            let payload = serde_json::to_string(&request)
                .map_err(|e| HyperliquidError::Serialize(e.to_string()))?;

            ws.write_frame(Frame::text(payload.into_bytes().into()))
                .await
                .map_err(|e| {
                    HyperliquidError::WebSocket(format!(
                        "Failed to send unsubscribe: {}",
                        e
                    ))
                })?;
        }

        Ok(())
    }

    /// Send a ping to keep connection alive
    pub async fn ping(&mut self) -> Result<(), HyperliquidError> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| HyperliquidError::WebSocket("Not connected".to_string()))?;

        let request = WsRequest::ping();
        let payload = serde_json::to_string(&request)
            .map_err(|e| HyperliquidError::Serialize(e.to_string()))?;

        ws.write_frame(Frame::text(payload.into_bytes().into()))
            .await
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Failed to send ping: {}", e))
            })?;

        Ok(())
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.ws.is_some()
    }

    /// Start reading messages (must be called after connecting)
    pub async fn start_reading(&mut self) -> Result<(), HyperliquidError> {
        let mut ws = self
            .ws
            .take()
            .ok_or_else(|| HyperliquidError::WebSocket("Not connected".to_string()))?;

        let message_tx = self.message_tx.clone().ok_or_else(|| {
            HyperliquidError::WebSocket("Message channel not initialized".to_string())
        })?;

        tokio::spawn(async move {
            loop {
                match ws.read_frame().await {
                    Ok(frame) => match frame.opcode {
                        OpCode::Text => {
                            if let Ok(text) = String::from_utf8(frame.payload.to_vec()) {
                                let _ = message_tx.send(text);
                            }
                        }
                        OpCode::Close => {
                            break;
                        }
                        _ => {}
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn message_router(
        mut rx: UnboundedReceiver<String>,
        subscriptions: Arc<DashMap<SubscriptionId, SubscriptionHandle>>,
    ) {
        while let Some(text) = rx.recv().await {
            // Use simd-json for fast parsing
            let mut text_bytes = text.into_bytes();
            match simd_json::from_slice::<Message>(&mut text_bytes) {
                Ok(message) => {
                    // Route to all active subscriptions
                    // In a more sophisticated implementation, we'd match by subscription type
                    for entry in subscriptions.iter() {
                        let _ = entry.value().tx.send(message.clone());
                    }
                }
                Err(_) => {
                    // Ignore parse errors
                }
            }
        }
    }
}

impl Drop for WsProvider {
    fn drop(&mut self) {
        // Clean shutdown
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }
}
