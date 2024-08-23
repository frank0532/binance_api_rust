pub mod binance_api {
    use chrono::{
        prelude::{DateTime, TimeZone, Utc},
        Duration, NaiveDateTime,
    };
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::{collections::HashMap, error::Error, net::TcpStream};
    use tungstenite::{connect, protocol::WebSocket, stream::MaybeTlsStream, Message};
    use url::Url;

    type SendRequestRe = serde_json::Value;
    #[derive(Default, Debug)]
    pub struct BinanceAPI<'a> {
        api_key: &'a str,
        secret_key: &'a str,
        pub account_type: &'a str,
        base_url: &'a str,
        wss_url: &'a str,
        listen_key: String,
    }

    impl<'a> BinanceAPI<'a> {
        pub async fn new(
            api_key: &'a str,
            secret_key: &'a str,
            account_type: &'a str,
        ) -> Result<Self, Box<dyn Error>> {
            let (base_url, wss_url) = match account_type {
                "spot" => ("https://api.binance.com", "wss://stream.binance.com/ws"),
                "swap" => ("https://fapi.binance.com", "wss://fstream.binance.com/ws"),
                _ => Self::panic_not_define("Account type", account_type, ("", "")),
            };
            let mut bn_api = BinanceAPI {
                api_key: api_key,
                secret_key: secret_key,
                account_type: account_type,
                base_url: base_url,
                wss_url: wss_url,
                listen_key: Default::default(),
            };
            if api_key.is_empty() || secret_key.is_empty() {
            } else {
                bn_api.listen_key = bn_api.listen_key_manager("generate").await.unwrap();
            };
            return Ok(bn_api);
        }

        fn panic_not_define<T>(type_name: &str, type_content: &str, res: T) -> T {
            assert!(false, "{type_name} `{type_content}` is not defined.");
            return res;
        }

        fn generate_exchange_url(&self, spot_swap_url: (&str, &str)) -> String {
            let current_type = self.account_type;
            let url = match current_type {
                "spot" => spot_swap_url.0,
                "swap" => spot_swap_url.1,
                _ => Self::panic_not_define("Account type", current_type, ""),
            };
            self.base_url.to_string() + url
        }

        fn generate_signature(&self, param_map: &HashMap<String, String>) -> String {
            let mut query = String::new();
            for (key, value) in param_map {
                query.push_str(&format!("{}={}&", key, value));
            }
            query.pop();
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(self.secret_key.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(query.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        }

        pub async fn send_request(
            &self,
            url: &str,
            method: &str,
            param_map: &mut HashMap<String, String>,
            signature: bool,
        ) -> Result<SendRequestRe, Box<dyn Error>> {
            let client = reqwest::Client::new();
            let mut headers_map = reqwest::header::HeaderMap::new();
            headers_map.insert("Content-Type", "application/json".parse().unwrap());
            headers_map.insert("X-MBX-APIKEY", self.api_key.parse().unwrap());
            let mut signature_map = HashMap::new();
            if signature {
                param_map.insert(
                    "timestamp".to_string(),
                    Utc::now().timestamp_millis().to_string(),
                );
                signature_map.insert("signature", self.generate_signature(&param_map));
            }
            let res = match method {
                "GET" => client.get(url),
                "POST" => client.post(url),
                "PUT" => client.put(url),
                "DELETE" => client.delete(url),
                _ => Self::panic_not_define("Request method", method, client.get(url)),
            };
            let res = res.headers(headers_map);
            let res = if param_map.is_empty() {
                res
            } else {
                res.query(&param_map)
            };
            let res = if signature {
                res.query(&signature_map)
            } else {
                res
            };
            let res = res.send().await?.text().await?;
            Ok(serde_json::from_str(&res).expect("Can't parse data to JSON"))
        }

        pub async fn listen_key_manager(&self, method: &str) -> Result<String, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/userDataStream", "/fapi/v1/listenKey"));
            let method_request = match method {
                "generate" => "POST",
                "delay" => "PUT",
                "delete" => "DELETE",
                _ => Self::panic_not_define("Listen key method", method, ""),
            };
            let mut param_map = HashMap::new();
            if ["delay", "delete"].contains(&method) {
                param_map.insert("listenKey".to_string(), self.listen_key.to_string());
            }

            let parsed = self
                .send_request(url.as_str(), method_request, &mut param_map, false)
                .await?;
            if method == "generate" {
                let listenkey = parsed["listenKey"].to_string();
                let listenkey = listenkey.split("\"").collect::<Vec<_>>()[1];
                return Ok(listenkey.to_string());
            } else {
                return Ok("".to_string());
            }
        }

        pub fn generate_websocket(&self, type_ws: &str) -> WebSocket<MaybeTlsStream<TcpStream>> {
            let stream_url = self.wss_url.to_string();
            let stream_url = match type_ws {
                "account" => stream_url + "/" + self.listen_key.as_str(),
                "market" => stream_url,
                _ => Self::panic_not_define("Websocket type", type_ws, stream_url),
            };
            let (mut websocket, _response) =
                connect(Url::parse(&stream_url).unwrap()).expect("Can't connect.");
            return websocket;
        }

        pub async fn subscribe_websocket(
            &self,
            ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
            symbols: &Vec<&str>,
            sub_type: &str,
        ) -> Result<(), Box<dyn Error>> {
            let subscribes = symbols
                .iter()
                .map(|a| format!(r#""{}@{}""#, a.to_lowercase(), sub_type))
                .collect::<Vec<_>>()
                .join(",");
            let subscribes = format!(
                r#"{{"method": "SUBSCRIBE", "params": [{}], "id": 1}}"#,
                subscribes
            );
            ws.write_message(Message::Text((&subscribes).into()))?;
            return Ok(());
        }

        pub async fn unsubscribe_websocket(
            &self,
            ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
            symbols: &Vec<&str>,
            sub_type: &str,
        ) -> Result<(), Box<dyn Error>> {
            let subscribes = symbols
                .iter()
                .map(|a| format!(r#""{}@{}""#, a.to_lowercase(), sub_type))
                .collect::<Vec<_>>()
                .join(",");
            let subscribes = format!(
                r#"{{"method": "UNSUBSCRIBE", "params": [{}], "id": 312}}"#,
                subscribes
            );
            ws.write_message(Message::Text((&subscribes).into()))?;
            return Ok(());
        }

        pub fn websocket_read_once(&self, ws: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> String {
            match ws.read_message().unwrap() {
                tungstenite::Message::Text(message) => message,
                _ => "{\"Error\":\"Can't getting text from websocket.\"}".to_string(),
            }
        }

        fn str2datetime(&self, utc_str: &str) -> DateTime<Utc> {
            NaiveDateTime::parse_from_str(utc_str, "%Y-%m-%d %H:%M:%S")
                .unwrap()
                .and_utc()
        }

        pub async fn history_klines(
            &self,
            symbol: &str,
            interval: &str,
            start_time_utc: &str,
            end_time_utc: &str,
        ) -> Result<Vec<SendRequestRe>, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/klines", "/fapi/v1/klines"));
            let mut param_map = std::collections::HashMap::new();
            param_map.insert("symbol".to_string(), symbol.to_string());
            param_map.insert("interval".to_string(), interval.to_string());
            param_map.insert(
                "startTime".to_string(),
                (self.str2datetime(start_time_utc).timestamp() * 1000).to_string(),
            );
            if !end_time_utc.is_empty() {
                param_map.insert(
                    "endTime".to_string(),
                    (self.str2datetime(end_time_utc).timestamp() * 1000).to_string(),
                );
            }
            let mut kline_data = vec![];
            loop {
                let parsed = self
                    .send_request(url.as_str(), "GET", &mut param_map, false)
                    .await?;
                let kdatai = parsed.as_array().unwrap().to_owned();
                if kdatai.is_empty() {
                    break;
                } else {
                    param_map.insert("startTime".to_string(),(kdatai[kdatai.len() - 1][0].as_i64().unwrap() + 1).to_string());
                    kline_data.extend(kdatai);
                }
            }
            Ok(kline_data[..kline_data.len() - 1].to_vec())
            // Ok(kline_data)
        }

        pub async fn get_exchange_info(&self) -> Result<SendRequestRe, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/exchangeInfo", "/fapi/v1/exchangeInfo"));
            let mut param_map = std::collections::HashMap::new();
            let parsed = self
                .send_request(url.as_str(), "GET", &mut param_map, false)
                .await?;
            Ok(parsed)
        }

        pub async fn get_price(&self, symbol: &str) -> Result<SendRequestRe, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/ticker/price", "/fapi/v1/ticker/price"));
            let mut param_map = std::collections::HashMap::new();
            if symbol.is_empty() {
            } else {
                param_map.insert("symbol".to_string(), symbol.to_string());
            }
            let parsed = self
                .send_request(url.as_str(), "GET", &mut param_map, false)
                .await?;
            Ok(parsed)
        }

        pub async fn get_ticker(&self, symbol: &str) -> Result<SendRequestRe, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/ticker/24hr", "/fapi/v1/ticker/24hr"));
            let mut param_map = std::collections::HashMap::new();
            if symbol.is_empty() {
            } else {
                param_map.insert("symbol".to_string(), symbol.to_string());
            }
            let parsed = self
                .send_request(url.as_str(), "GET", &mut param_map, false)
                .await?;
            Ok(parsed)
        }

        pub async fn new_order(
            &self,
            symbol: &str,
            side: &str,
            trade_type: &str,
            quantity: &str,
            price: &str,
            time_inforce: &str,
        ) -> Result<SendRequestRe, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/order", "/fapi/v1/order"));
            let mut param_map = std::collections::HashMap::new();
            param_map.insert("symbol".to_string(), symbol.to_string());
            param_map.insert("side".to_string(), side.to_string());
            param_map.insert("type".to_string(), trade_type.to_string());
            param_map.insert("quantity".to_string(), quantity.to_string());
            if trade_type == "LIMIT" {
                param_map.insert("price".to_string(), price.to_string());
                param_map.insert("timeInForce".to_string(), time_inforce.to_string());
            }
            let parsed = self
                .send_request(url.as_str(), "POST", &mut param_map, true)
                .await?;
            Ok(parsed)
        }

        pub async fn cancel_order(
            &self,
            symbol: &str,
            order_id: &str,
            all: bool,
        ) -> Result<SendRequestRe, Box<dyn Error>> {
            let url = if all {
                self.generate_exchange_url(("/api/v3/openOrders", "/fapi/v1/allOpenOrders"))
            } else {
                self.generate_exchange_url(("/api/v3/order", "/fapi/v1/order"))
            };
            let mut param_map = std::collections::HashMap::new();
            param_map.insert("symbol".to_string(), symbol.to_string());
            if !all {
                param_map.insert("orderId".to_string(), order_id.to_string());
            }
            let parsed = self
                .send_request(url.as_str(), "DELETE", &mut param_map, true)
                .await?;
            Ok(parsed)
        }

        pub async fn pull_account(&self) -> Result<SendRequestRe, Box<dyn Error>> {
            let url = self.generate_exchange_url(("/api/v3/account", "/fapi/v2/account"));
            let mut param_map = std::collections::HashMap::new();
            let parsed = self
                .send_request(url.as_str(), "GET", &mut param_map, true)
                .await?;
            Ok(parsed)
        }

        pub async fn get_position(&self) -> Result<SendRequestRe, Box<dyn Error>> {
            assert!(self.account_type == "swap", "only `swap` can get position.");
            let url = self.generate_exchange_url(("", "/fapi/v2/positionRisk"));
            let mut param_map = std::collections::HashMap::new();
            let parsed = self
                .send_request(url.as_str(), "GET", &mut param_map, true)
                .await?;
            Ok(parsed)
        }

        pub async fn get_balance(&self) -> Result<SendRequestRe, Box<dyn Error>> {
            assert!(self.account_type == "swap", "only `swap` can get balance.");
            let url = self.base_url.to_string() + "/fapi/v2/balance";
            let mut param_map = HashMap::new();
            let parsed = self
                .send_request(url.as_str(), "GET", &mut param_map, true)
                .await?;
            Ok(parsed)
        }
    }
}
