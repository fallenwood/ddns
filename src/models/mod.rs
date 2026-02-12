#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GetZonesResponse {
    pub result: Vec<Zone>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub content: String,
    pub proxied: bool,
    pub ttl: i32,
    pub comment: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GetDnsRecordsResponse {
    pub success: bool,
    pub result: Vec<DnsRecord>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PostOrPutDnsRecordRequest {
    pub name: String,
    pub r#type: String,
    pub content: String,
    pub proxied: bool,
    pub ttl: i32,
    pub comment: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PostOrPutDnsRecordResponse {
    pub success: bool,
    pub result: DnsRecord,
}