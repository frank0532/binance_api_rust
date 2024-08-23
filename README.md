# binance_api_rust
Simple Binance API in Rust.

## The basic usages
* initiate.  
`let bn_api = BinanceAPI::new("api_key", "secret_key","swap").await.unwrap();`

* get history klines data.  
`let klines = bn_api.history_klines("BTCUSDT", "1h", "2024-01-01 00:00:00", "").await.unwrap();`

* create websocket.  
`let mut ws = bn_api.generate_websocket("market");`
* book 4-hours klines stream for 'BTCUSDT' and 'ETHUSDT'.  
`bn_api.subscribe_websocket(&mut ws, &vec!["BTCUSDT", "ETHUSDT"], "kline_4h").await.unwrap();`

* send a new real-trade order to Binance: buy 0.1 'BTCUSDT' at market real-time price on Binance.  
`bn_api.new_order("BTCUSDT", "BUY", "MARKET", "0.1").await.unwrap();`
