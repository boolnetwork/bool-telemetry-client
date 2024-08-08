use lazy_static::lazy_static;
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::sync::RwLock;
use std::time::Duration;

lazy_static! {
    static ref DEVICE_STATUS: RwLock<DeviceStatus> = RwLock::new(DeviceStatus::default());
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
    id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct SaveDataParams {
    key: String,
    value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct DeviceStatus {
    device_id: String,
    #[serde(default)]
    peers_count: u32,
}

async fn update_status(
    client: &Client,
    url: &str,
    params: &DeviceStatus,
) -> Result<(), Box<dyn Error>> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "update_status".to_string(),
        params: json!(params),
        id: 1,
    };

    let response: JsonRpcResponse = client.post(url).json(&request).send().await?.json().await?;

    if let Some(result) = response.result {
        debug!("Response: {:?}", result);
    } else if let Some(error) = response.error {
        error!("Error: {:?}", error);
    }

    Ok(())
}

#[allow(dead_code)]
async fn get_status(client: &Client, url: &str) -> Result<(), Box<dyn Error>> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "get_status".to_string(),
        params: serde_json::Value::Null,
        id: 2,
    };

    let response: JsonRpcResponse = client.post(url).json(&request).send().await?.json().await?;

    if let Some(result) = response.result {
        info!("Response: {:?}", result);
    } else if let Some(error) = response.error {
        error!("Error: {:?}", error);
    }

    Ok(())
}

pub async fn start_update_status(url: &str, interval: u64) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval));
    let client = Client::new();
    loop {
        interval.tick().await;
        let device = DEVICE_STATUS.read().unwrap().clone();
        if !device.device_id.is_empty() {
            if let Err(_) = update_status(&client, url, &device).await {
                error!("update status to telemetry failed");
            }
        } else {
            debug!("skip update status");
        }
    }
}

pub fn set_device_id(device_id: String) {
    DEVICE_STATUS.write().unwrap().device_id = device_id;
}

pub fn set_peers_count(peers_count: u32) {
    DEVICE_STATUS.write().unwrap().peers_count = peers_count;
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger::Env;
    use rand;
    use rand::random;
    use tokio::runtime::Runtime;

    #[test]
    #[ignore]
    fn it_works() {
        env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let url = "http://127.0.0.1:3030";
            let interval = 1;

            tokio::spawn(async move { start_update_status(url, interval).await });

            let client = Client::new();

            let mut interval = tokio::time::interval(Duration::from_secs(1));
            for i in 0..100 {
                interval.tick().await;

                if i < 3 {
                    continue;
                }

                if i % 5 == 0 {
                    let random_u128: u128 = random();
                    let device_id = format!("0x{:064x}", random_u128);
                    set_device_id(device_id);
                }

                set_peers_count(random());

                // Get data
                let _ = get_status(&client, url).await;
            }
        });
    }
}
