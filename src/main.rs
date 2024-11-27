use config::{Config, ConfigError, File};
use log::{error, info, warn};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::process;

const REST_URL: &str = "https://api.gandi.net/v5/livedns/";

#[derive(Debug)]
struct DnsConfig {
    key: String,
    domain: String,
    records: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum IpVersion {
    V4,
    V6,
}

async fn get_public_ip(version: IpVersion) -> Option<String> {
    let ip_type = match version {
        IpVersion::V4 => "",
        IpVersion::V6 => "6",
    };
    let str_ip_type = match version {
        IpVersion::V4 => "v4",
        IpVersion::V6 => "v6",
    };

    let url = format!("https://api{}.ipify.org?format=json", ip_type);
    let response = reqwest::get(&url).await.ok()?;

    if response.status().is_success() {
        let json: Value = response.json().await.ok()?;
        let ip = json["ip"].as_str().unwrap_or("").to_string();
        info!("Public IP{}: {}", str_ip_type, ip);
        Some(ip)
    } else {
        error!("Critical Error: Unable to get public IP!");
        error!("Status Code: {}", response.status());
        None
    }
}

async fn get_public_ips() -> (Option<String>, Option<String>) {
    let ip4 = get_public_ip(IpVersion::V4).await;
    let ip6 = get_public_ip(IpVersion::V6).await;
    (ip4, ip6)
}

async fn get_gandi_record(
    domain: &str,
    name: &str,
    dns_type: &str,
    headers: &HeaderMap,
) -> Option<Vec<String>> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}domains/{}/records/{}/{}",
        REST_URL, domain, name, dns_type
    );

    let response = client
        .get(&url)
        .headers(headers.clone())
        .send()
        .await
        .ok()?;

    if response.status().is_success() {
        let json: Value = response.json().await.ok()?;
        let values = json["rrset_values"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        Some(values)
    } else {
        error!(
            "Critical Error: Unable to retrieve the {} record for {}@{} from Gandi!",
            dns_type, name, domain
        );
        error!("Status Code: {}", response.status());
        None
    }
}

async fn update_gandi_record(
    domain: &str,
    name: &str,
    dns_type: &str,
    new_ip: &str,
    headers: &HeaderMap,
) -> Result<bool, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}domains/{}/records/{}/{}",
        REST_URL, domain, name, dns_type
    );

    let payload = serde_json::json!({
        "rrset_ttl": 1800,
        "rrset_values": [new_ip]
    });

    let response = client
        .put(&url)
        .headers(headers.clone())
        .json(&payload)
        .send()
        .await?;

    let changed = response.status().as_u16() == 201;
    if !changed {
        warn!("{} -> {}@{}: {}", dns_type, name, domain, response.status());
    }

    Ok(changed)
}

fn read_config() -> Result<DnsConfig, ConfigError> {
    let config = Config::builder()
        .add_source(File::with_name(".gandi.toml"))
        .build()?;

    let key = config.get_string("GANDI.key")?;
    let domain = config.get_string("DNS.domain")?;
    let records_str = config.get_string("DNS.records")?;
    let records: Vec<String> = records_str.split('\n').map(String::from).collect();

    Ok(DnsConfig {
        key,
        domain,
        records,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_level(log::Level::Info)?;

    let config = match read_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Invalid Configuration File! {}", e);
            process::exit(1);
        }
    };

    info!("Updating the records of {} ...", config.domain);

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.key))?,
    );

    let (ipv4, ipv6) = get_public_ips().await;

    let ip_configs = [
        (ipv4.as_ref(), "A"),
        (ipv6.as_ref(), "AAAA"),
    ];

    let mut n_changed = 0;
    for record in &config.records {
        info!("\tUpdating the entries of {}@{} ...", record, config.domain);

        for (ip, dns_type) in ip_configs.iter() {
            if let Some(ip) = ip {
                let gandi_record = get_gandi_record(&config.domain, record, dns_type, &headers).await;

                if let Some(gandi_record) = gandi_record {
                    if gandi_record.is_empty() {
                        warn!(
                            "Warning! The record {}/{} is empty, and thus cannot be updated!",
                            record, dns_type
                        );
                    } else {
                        if update_gandi_record(&config.domain, record, dns_type, ip, &headers).await? {
                            n_changed += 1;
                        }
                    }
                } else {
                    warn!(
                        "Warning! The record {}/{} does not exist, and thus cannot be updated!",
                        record, dns_type
                    );
                }
            }
        }
    }

    info!("Success! {} DNS records were changed.", n_changed);
    Ok(())
}
