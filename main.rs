mod binance_api;
use binance_api::binance_api::BinanceAPI;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bn_api = BinanceAPI::new("api_key", "secret_key", "swap")
        .await
        .unwrap();
    // let delay = bn_api.listen_key_manager("delay").await.unwrap();
    // let delete = bn_api.listen_key_manager("delete").await.unwrap();
    let mut ws = bn_api.generate_websocket("market");
    bn_api
        .subscribe_websocket(&mut ws, &vec!["BTCUSDT", "ETHUSDT"], "aggTrade")
        .await
        .unwrap();
    for _ in 0..10 {
        dbg!(bn_api.websocket_read_once(&mut ws));
    }
    let klines = bn_api
        .history_klines("BTCUSDT", "1h", "2024-01-01 00:00:00", "")
        .await
        .unwrap();
    let exchange_info = bn_api.get_exchange_info().await.unwrap();
    let account_info = bn_api.pull_account().await.unwrap();
    let position = bn_api.get_position().await.unwrap();
    let prices = bn_api.get_price("").await.unwrap();
    let tickers = bn_api.get_ticker("").await.unwrap();
    let balance = bn_api.get_balance().await.unwrap();
    let an_order = bn_api
        .new_order("BTCUSDT", "BUY", "LIMIT", "0.1", "50000", "GTC")
        .await
        .unwrap();
    let cancel_order = bn_api.cancel_order("BTCUSDT", "0", true).await.unwrap();
    Ok(())
}
