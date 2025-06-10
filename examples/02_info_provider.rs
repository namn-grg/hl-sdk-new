use ferrofluid::providers::InfoProvider;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create mainnet provider
    let info = InfoProvider::mainnet();

    // 1. Test all_mids endpoint
    println!("=== Testing all_mids ===");
    match info.all_mids().await {
        Ok(mids) => {
            println!("Found {} mid prices", mids.len());
            // Print first 5 entries
            for (coin, price) in mids.iter().take(5) {
                println!("{}: {}", coin, price);
            }
        }
        Err(e) => println!("Error fetching mids: {}", e),
    }

    // 2. Test l2_book endpoint
    println!("\n=== Testing l2_book for BTC ===");
    match info.l2_book("BTC").await {
        Ok(book) => {
            println!("BTC Order Book at time {}", book.time);
            println!("Levels: {} bid levels, {} ask levels", 
                book.levels[0].len(), 
                book.levels[1].len()
            );
            
            // Show top 3 levels each side
            println!("\nTop 3 Bids:");
            for level in book.levels[0].iter().take(3) {
                println!("  Price: {}, Size: {}, Count: {}", level.px, level.sz, level.n);
            }
            
            println!("\nTop 3 Asks:");
            for level in book.levels[1].iter().take(3) {
                println!("  Price: {}, Size: {}, Count: {}", level.px, level.sz, level.n);
            }
        }
        Err(e) => println!("Error fetching L2 book: {}", e),
    }

    // 3. Test recent_trades endpoint
    println!("\n=== Testing recent_trades for ETH ===");
    match info.recent_trades("ETH").await {
        Ok(trades) => {
            println!("Recent ETH trades: {} trades", trades.len());
            for trade in trades.iter().take(5) {
                println!("  Time: {}, Side: {}, Price: {}, Size: {}", 
                    trade.time, trade.side, trade.px, trade.sz
                );
            }
        }
        Err(e) => println!("Error fetching recent trades: {}", e),
    }

    // 4. Test candles with builder pattern
    println!("\n=== Testing candles for SOL ===");
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let one_hour_ago = now - (60 * 60 * 1000); // 1 hour in milliseconds
    
    match info.candles("SOL")
        .interval("15m")
        .time_range(one_hour_ago, now)
        .send()
        .await
    {
        Ok(candles) => {
            println!("SOL 15m candles: {} candles", candles.len());
            for candle in candles.iter().take(3) {
                println!("  Time: {}-{}, O: {}, H: {}, L: {}, C: {}, V: {}", 
                    candle.time_open, candle.time_close,
                    candle.open, candle.high, candle.low, candle.close,
                    candle.vlm
                );
            }
        }
        Err(e) => println!("Error fetching candles: {}", e),
    }

    // 5. Test meta endpoint
    println!("\n=== Testing meta endpoint ===");
    match info.meta().await {
        Ok(meta) => {
            println!("Found {} assets in universe", meta.universe.len());
            for asset in meta.universe.iter().take(5) {
                println!("  {}: decimals={}, max_leverage={}, isolated_only={}", 
                    asset.name, asset.sz_decimals, asset.max_leverage, asset.only_isolated
                );
            }
        }
        Err(e) => println!("Error fetching meta: {}", e),
    }

    // 6. Test spot_meta endpoint
    println!("\n=== Testing spot_meta endpoint ===");
    match info.spot_meta().await {
        Ok(spot_meta) => {
            println!("Found {} spot pairs", spot_meta.universe.len());
            println!("Found {} tokens", spot_meta.tokens.len());
            
            println!("\nFirst 5 spot pairs:");
            for pair in spot_meta.universe.iter().take(5) {
                println!("  {}: index={}, canonical={}, tokens={:?}", 
                    pair.name, pair.index, pair.is_canonical, pair.tokens
                );
            }
            
            println!("\nFirst 5 tokens:");
            for token in spot_meta.tokens.iter().take(5) {
                println!("  {}: index={}, wei_decimals={}, token_id={}", 
                    token.name, token.index, token.wei_decimals, &token.token_id[..16]
                );
            }
        }
        Err(e) => println!("Error fetching spot meta: {}", e),
    }

    // 7. Test funding_history with builder
    println!("\n=== Testing funding_history for BTC ===");
    let one_day_ago = now - (24 * 60 * 60 * 1000); // 24 hours in milliseconds
    
    match info.funding_history("BTC")
        .time_range(one_day_ago, now)
        .send()
        .await
    {
        Ok(history) => {
            println!("BTC funding history: {} entries", history.len());
            for entry in history.iter().take(5) {
                println!("  Time: {}, Rate: {}, Premium: {}", 
                    entry.time, entry.funding_rate, entry.premium
                );
            }
        }
        Err(e) => println!("Error fetching funding history: {}", e),
    }

    println!("\n=== All tests completed ===");
    Ok(())
}