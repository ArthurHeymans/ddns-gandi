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

async fn get_public_ip(ipv4: bool) -> Result<String, Box<dyn std::error::Error>> {
    let ip_type = if ipv4 { "" } else { "64" };
    let str_ip_type = if ipv4 { "v4" } else { "v6" };
    
    let url = format!("https://api{}.ipify.org?format=json", ip_type);
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let json: Value = response.json().await?;
        let ip = json["ip"].as_str().unwrap_or("").to_string();
        info!("Public IP{}: {}", str_ip_type, ip);
        Ok(ip)
    } else {
        error!("Critical Error: Unable to get public IP!");
        error!("Status Code: {}", response.status());
        process::exit(1);
    }
}

async fn get_public_ips() -> Result<(String, String), Box<dyn std::error::Error>> {
    let ip4 = get_public_ip(true).await?;
    let ip6 = get_public_ip(false).await?;
    Ok((ip4, ip6))
}

async fn get_gandi_record(
    domain: &str,
    name: &str,
    dns_type: &str,
    headers: &HeaderMap,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!("{}domains/{}/records/{}/{}", REST_URL, domain, name, dns_type);
    
    let response = client.get(&url).headers(headers.clone()).send().await?;
    
    if response.status().is_success() {
        let json: Value = response.json().await?;
        let values = json["rrset_values"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        Ok(values)
    } else {
        error!("Critical Error: Unable to retrieve the {} record for {}@{} from Gandi!", dns_type, name, domain);
        error!("Status Code: {}", response.status());
        process::exit(1);
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
    let url = format!("{}domains/{}/records/{}/{}", REST_URL, domain, name, dns_type);
    
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
        .add_source(File::with_name(".gandi"))
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
        HeaderValue::from_str(&format!("Apikey {}", config.key))?,
    );
    
    let (ipv4, ipv6) = get_public_ips().await?;
    
    let mut n_changed = 0;
    for record in &config.records {
        info!("\tUpdating the entries of {}@{} ...", record, config.domain);
        
        let gandi_ipv4 = get_gandi_record(&config.domain, record, "A", &headers).await?;
        let gandi_ipv6 = get_gandi_record(&config.domain, record, "AAAA", &headers).await?;
        
        if gandi_ipv4.is_empty() && gandi_ipv6.is_empty() {
            warn!("Warning! The record {} does not exist, and thus cannot be updated!", record);
            continue;
        }
        
        if !gandi_ipv4.is_empty() && ipv4 != gandi_ipv4[0] {
            if update_gandi_record(&config.domain, record, "A", &ipv4, &headers).await? {
                n_changed += 1;
            }
        }
        
        if !gandi_ipv6.is_empty() && ipv6 != gandi_ipv6[0] {
            if update_gandi_record(&config.domain, record, "AAAA", &ipv6, &headers).await? {
                n_changed += 1;
            }
        }
    }
    
    info!("Success! {} DNS records were changed.", n_changed);
    Ok(())
}
