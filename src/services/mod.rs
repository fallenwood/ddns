const CF_BASE_URL: &str = "https://api.cloudflare.com/client/v4";
const ECHO_URL: &str = "https://echo.fallenwood.net/";

use crate::models::{self, DnsRecord};

pub async fn get_ipaddress(r#type: String) -> String {
    let client = reqwest::Client::builder()
        .local_address(
            (match r#type.as_str() {
                "A" => "0.0.0.0",
                "AAAA" => "[::]:0",
                _ => panic!("[get_ipaddress] Unsupported IP type: {}", r#type),
            })
            .parse()
            .ok(),
        )
        .build()
        .expect("[get_ipaddress] Failed to build HTTP client");

    let request = client
        .get(ECHO_URL)
        .build()
        .expect("[get_ipaddress] Failed to build IPv6 request");

    client
        .execute(request)
        .await
        .expect("[get_ipaddress] Failed to send request")
        .headers()
        .get("X-Client-IP")
        .and_then(|x| x.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub struct DnsProvider {
    zone_name: String,
    token: String,
    zone_id: Option<String>,
    client: reqwest::Client,
}

impl DnsProvider {
    pub fn new(zone_name: String, token: String, cf_proxy: Option<String>) -> Self {
        let mut client_builder = reqwest::Client::builder();

        if let Some(proxy) = cf_proxy
            .as_deref()
            .map(str::trim)
            .filter(|proxy| !proxy.is_empty())
        {
            client_builder = client_builder.proxy(
                reqwest::Proxy::all(proxy).expect("[DnsProvider] Invalid DDNS_CF_PROXY value"),
            );
        }

        let client = client_builder
            .build()
            .expect("[DnsProvider] Failed to build Cloudflare HTTP client");

        DnsProvider {
            zone_name,
            token,
            zone_id: None,
            client,
        }
    }

    async fn get_zone_id(&mut self) -> Option<String> {
        let url = format!("{}/zones", CF_BASE_URL);

        let request = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .build()
            .expect("[get_zone_id] Failed to build request");

        let response = self
            .client
            .execute(request)
            .await
            .expect("[get_zone_id] Failed to send request");

        let zone_response: Result<models::GetZonesResponse, _> = response.json().await;

        let zone_response = zone_response.expect("[get_zone_id] API returned an error");

        return zone_response
            .result
            .into_iter()
            .find(|e| e.name == self.zone_name)
            .map(|e| e.id);
    }

    pub async fn get_dns_records(&mut self, hostname: &str) -> Vec<models::DnsRecord> {
        if self.zone_id.is_none() {
            self.zone_id = self.get_zone_id().await;
        }

        let zone_id = self
            .zone_id
            .clone()
            .expect("[get_dns_records] Failed to find Zone ID for zone: {}");

        let url = format!("{}/zones/{}/dns_records", CF_BASE_URL, zone_id);

        let request = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .build()
            .expect("[get_dns_records] Failed to build request");

        let response = self
            .client
            .execute(request)
            .await
            .expect("[get_dns_records] Failed to send request");

        let dns_records_response: Result<models::GetDnsRecordsResponse, _> = response.json().await;

        let dns_records_response =
            dns_records_response.expect("[get_dns_records] API returned an error");

        let records: Vec<models::DnsRecord> = dns_records_response
            .result
            .into_iter()
            .filter(|e| e.name == hostname)
            .collect();

        return records;
    }

    pub async fn create_dns_record(
        &mut self,
        host_name: &str,
        ip_address: &str,
        ip_type: &str,
        comment: Option<&str>,
    ) -> Option<DnsRecord> {
        if self.zone_id.is_none() {
            self.zone_id = self.get_zone_id().await;
        }

        let zone_id = match &self.zone_id {
            Some(id) => id,
            None => {
                panic!(
                    "[create_dns_record] Failed to find Zone ID for zone: {}",
                    self.zone_name
                );
            }
        };

        let url = format!("{}/zones/{}/dns_records", CF_BASE_URL, zone_id);
        let request = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&models::PostOrPutDnsRecordRequest {
                name: host_name.to_string(),
                r#type: ip_type.to_string(),
                content: ip_address.to_string(),
                proxied: false,
                ttl: 60,
                comment: comment.map(|c| c.to_string()),
            })
            .build()
            .expect("[create_dns_record] Failed to build POST request");

        let response = self
            .client
            .execute(request)
            .await
            .expect("[create_dns_record] Failed to send request");

        let dns_record_response: Result<models::PostOrPutDnsRecordResponse, _> =
            response.json().await;

        match dns_record_response {
            Ok(r) if r.success => Some(r.result),
            _ => None,
        }
    }

    pub async fn update_dns_record(
        &mut self,
        record_id: &str,
        host_name: &str,
        ip_address: &str,
        ip_type: &str,
        comment: Option<&str>,
    ) -> Option<DnsRecord> {
        if self.zone_id.is_none() {
            self.zone_id = self.get_zone_id().await;
        }

        let zone_id = match &self.zone_id {
            Some(id) => id,
            None => {
                panic!(
                    "[update_dns_record] Failed to find Zone ID for zone: {}",
                    self.zone_name
                );
            }
        };

        let url = format!(
            "{}/zones/{}/dns_records/{}",
            CF_BASE_URL, zone_id, record_id
        );
        let request = self
            .client
            .put(&url)
            .bearer_auth(&self.token)
            .json(&models::PostOrPutDnsRecordRequest {
                name: host_name.to_string(),
                r#type: ip_type.to_string(),
                content: ip_address.to_string(),
                proxied: false,
                ttl: 60,
                comment: comment.map(|c| c.to_string()),
            })
            .build()
            .expect("[update_dns_record] Failed to build PUT request");

        let response = self
            .client
            .execute(request)
            .await
            .expect("[update_dns_record] Failed to send request");

        let dns_record_response: Result<models::PostOrPutDnsRecordResponse, _> =
            response.json().await;

        match dns_record_response {
            Ok(r) if r.success => Some(r.result),
            _ => None,
        }
    }

    pub async fn delete_dns_record(&mut self, record_id: &str) -> bool {
        if self.zone_id.is_none() {
            self.zone_id = self.get_zone_id().await;
        }

        let zone_id = match &self.zone_id {
            Some(id) => id,
            None => {
                panic!(
                    "[delete_dns_record] Failed to find Zone ID for zone: {}",
                    self.zone_name
                );
            }
        };

        let url = format!(
            "{}/zones/{}/dns_records/{}",
            CF_BASE_URL, zone_id, record_id
        );

        let request = self
            .client
            .delete(&url)
            .bearer_auth(&self.token)
            .build()
            .expect("[delete_dns_record] Failed to build DELETE request");

        let response = self
            .client
            .execute(request)
            .await
            .expect("[delete_dns_record] Failed to send request");

        response.status().is_success()
    }
}
