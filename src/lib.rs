use lazy_static::lazy_static;
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::sync::RwLock;
use std::time::Duration;

lazy_static! {
    static ref DEVICE_STATUS: RwLock<DeviceStatus> = RwLock::new(DeviceStatus::new());
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct DeviceStatus {
    device_id: String,
    device_owner: String,
    device_version: String,
    peer_id: String,
    #[serde(default)]
    peers_count: u32,
    best_block_number: u32,
    finalized_block_number: u32,
    upload_bandwidth: Vec<u64>,
    download_bandwidth: Vec<u64>,
    uptime: i64,
    monitor_type: u8,
    monitor_sync_chains: Vec<(u32, u32)>,
    errors: Vec<u8>,
}

impl DeviceStatus {
    fn new() -> Self {
        let mut device_status = Self::default();
        device_status.upload_bandwidth = vec![0; 30];
        device_status.download_bandwidth = vec![0; 30];
        device_status
    }
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

fn start_calculate_bandwidth() {
    {
        let mut device = DEVICE_STATUS.write().unwrap();
        device.upload_bandwidth.pop();
        device.upload_bandwidth.push(0);
        device.download_bandwidth.pop();
        device.download_bandwidth.push(0);
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut device = DEVICE_STATUS.write().unwrap();
            device.upload_bandwidth.remove(0);
            device.upload_bandwidth.push(0);
            device.download_bandwidth.remove(0);
            device.download_bandwidth.push(0);
        }
    });
}

pub async fn start_update_status(url: &str, interval: u64) {
    start_calculate_bandwidth();
    let mut interval = tokio::time::interval(Duration::from_secs(interval));
    let client = Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
    loop {
        interval.tick().await;
        let device = DEVICE_STATUS.read().unwrap().clone();
        if !device.device_id.is_empty() && !device.device_owner.is_empty()
        && !device.peer_id.is_empty()
            {
            if let Err(e) = update_status(&client, url, &device).await {
                debug!("update status to telemetry failed with error: {}",e);
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

pub fn set_device_owner(device_owner: String) {
    DEVICE_STATUS.write().unwrap().device_owner = device_owner;
}

pub fn set_device_version(version: String) {
    DEVICE_STATUS.write().unwrap().device_version = version;
}

pub fn set_peer_id(peer_id: String) {
    DEVICE_STATUS.write().unwrap().peer_id = peer_id;
}

pub fn set_peers_count(peers_count: u32) {
    DEVICE_STATUS.write().unwrap().peers_count = peers_count;
}

pub fn set_best_block_number(block_number: u32) {
    DEVICE_STATUS.write().unwrap().best_block_number = block_number;
}

pub fn set_finalized_block_number(block_number: u32) {
    DEVICE_STATUS.write().unwrap().finalized_block_number = block_number;
}

pub fn set_uptime(uptime: i64) {
    DEVICE_STATUS.write().unwrap().uptime = uptime;
}

pub fn set_monitor_sync_status(ty: u8, chains: Vec<(u32, u32)>) {
    DEVICE_STATUS.write().unwrap().monitor_type = ty;
    DEVICE_STATUS.write().unwrap().monitor_sync_chains = chains;
}

pub fn set_errors(errors: Vec<u8>) {
    DEVICE_STATUS.write().unwrap().errors = errors;
}

pub fn add_upload(n: u64) {
    let mut device_status = DEVICE_STATUS.write().unwrap();
    if let Some(old) = device_status.upload_bandwidth.pop() {
        device_status.upload_bandwidth.push(old + n);
    }
}

pub fn add_download(n: u64) {
    let mut device_status = DEVICE_STATUS.write().unwrap();
    if let Some(old) = device_status.download_bandwidth.pop() {
        device_status.download_bandwidth.push(old + n);
    }
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

            add_upload(100);
            add_download(1000);
            tokio::spawn(async move { start_update_status(url, interval).await });

            let client = Client::new();

            let mut interval = tokio::time::interval(Duration::from_secs(1));
            for i in 0..1000 {
                interval.tick().await;

                if i < 3 {
                    continue;
                }

                if i == 5 {
                    let random_u128: u128 = random();
                    let device_id = format!("0x{:064x}", random_u128);
                    set_device_id(device_id);
                    set_device_version("0.11.19".to_string());
                }

                let random_u128: u128 = random();
                let device_owner = format!("0x{:040x}", random_u128);
                set_device_owner(device_owner);

                set_peers_count(random());
                add_upload(1);
                add_download(1);

                // Get data
                let _ = get_status(&client, url).await;
            }
        });
    }
}
