use super::*;
use crate::{DnsUpdateOutcome, update_dns_record};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const HOSTNAME: &str = "host.example.com";
const TARGET_IP: &str = "2001:db8::3";

fn record(id: &str, name: &str, kind: &str, content: &str) -> Value {
    json!({
        "id": id, "name": name, "type": kind, "content": content,
        "proxied": false, "ttl": 60, "comment": null
    })
}

#[derive(Default)]
struct ApiState {
    records: Vec<Value>,
    mutations: Vec<String>,
    list_failure: Option<(u32, u16, Value)>,
    delete_failure: Option<(String, u16, bool)>,
}

struct MockApi {
    provider: DnsProvider,
    state: Arc<Mutex<ApiState>>,
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
}

impl MockApi {
    fn new(records: Vec<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(ApiState {
            records,
            ..ApiState::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let server_state = Arc::clone(&state);
        let server_stop = Arc::clone(&stop);
        let server = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                if server_stop.load(Ordering::Relaxed) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut request = line.split_whitespace();
                let method = request.next().unwrap().to_string();
                let url = reqwest::Url::parse(request.next().unwrap()).unwrap();
                let mut content_length = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((key, value)) = line.split_once(':')
                        && key.eq_ignore_ascii_case("content-length")
                    {
                        content_length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).unwrap();
                let (status, response) = server_state.lock().unwrap().respond(&method, &url, &body);
                let response = response.to_string();
                write!(stream, "HTTP/1.1 {} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, response.len(), response).unwrap();
            }
        });
        let provider = DnsProvider {
            zone_name: "example.com".to_string(),
            token: "test-token".to_string(),
            zone_id: Some("zone".to_string()),
            api_base_url: "http://cloudflare.test/client/v4",
            client: reqwest::Client::builder()
                .proxy(reqwest::Proxy::http(format!("http://{}", address)).unwrap())
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        };
        Self {
            provider,
            state,
            address,
            stop,
            server: Some(server),
        }
    }

    async fn update(&mut self) -> DnsUpdateOutcome {
        update_dns_record(&mut self.provider, HOSTNAME, TARGET_IP.parse().unwrap()).await
    }
}

impl Drop for MockApi {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        self.server.take().unwrap().join().unwrap();
    }
}

impl ApiState {
    fn respond(&mut self, method: &str, url: &reqwest::Url, body: &[u8]) -> (u16, Value) {
        match method {
            "GET" => {
                let page = url
                    .query_pairs()
                    .find(|(key, _)| key == "page")
                    .map(|(_, value)| value.parse::<u32>().unwrap())
                    .unwrap_or(1);
                if let Some((failed_page, status, response)) = &self.list_failure
                    && page == *failed_page
                {
                    return (*status, response.clone());
                }
                let name = url
                    .query_pairs()
                    .find(|(key, _)| key == "name.exact")
                    .map(|(_, value)| value.into_owned());
                let records: Vec<_> = self
                    .records
                    .iter()
                    .filter(|record| {
                        name.as_ref().is_none_or(|name| {
                            record["name"].as_str().unwrap().eq_ignore_ascii_case(name)
                        })
                    })
                    .collect();
                // One record per page forces the client to exhaust pagination.
                (
                    200,
                    json!({"success": true, "result": records.iter().skip((page - 1) as usize).take(1).collect::<Vec<_>>(),
                    "result_info": {"total_pages": records.len()}}),
                )
            }
            "DELETE" => {
                let id = url.path_segments().unwrap().next_back().unwrap();
                self.mutations.push(format!("DELETE {}", id));
                if let Some((failed_id, status, success)) = &self.delete_failure
                    && id == failed_id
                {
                    return (*status, json!({"success": success}));
                }
                self.records.retain(|record| record["id"] != id);
                (200, json!({"success": true, "result": {"id": id}}))
            }
            "POST" => {
                self.mutations.push("POST new".to_string());
                let mut record: Value = serde_json::from_slice(body).unwrap();
                // Creating before all old AAAA records are gone is a visible failure.
                if self
                    .records
                    .iter()
                    .any(|old| old["name"] == record["name"] && old["type"] == "AAAA")
                {
                    return (409, json!({"success": false}));
                }
                record["id"] = json!("new");
                self.records.push(record.clone());
                (200, json!({"success": true, "result": record}))
            }
            _ => panic!("Unexpected method: {}", method),
        }
    }
}

#[tokio::test]
async fn replaces_all_paginated_addresses_before_creating_one() {
    // Cover both IP changes and an already-existing target IP among duplicates.
    for second_ip in ["2001:db8::2", TARGET_IP] {
        let unrelated = [
            record("a", HOSTNAME, "A", "192.0.2.1"),
            record("txt", HOSTNAME, "TXT", "keep"),
            record("other", "other.example.com", "AAAA", "2001:db8::9"),
        ];
        let mut records = unrelated.to_vec();
        records.push(record("first", HOSTNAME, "AAAA", "2001:db8::1"));
        records.push(record("second", HOSTNAME, "AAAA", second_ip));
        let mut api = MockApi::new(records);
        assert_eq!(api.update().await, DnsUpdateOutcome::Updated);
        let state = api.state.lock().unwrap();
        assert_eq!(
            state.mutations,
            ["DELETE first", "DELETE second", "POST new"]
        );
        assert_eq!(&state.records[..3], &unrelated);
        assert_eq!(state.records.len(), 4);
        assert_eq!(state.records[3]["name"], HOSTNAME);
        assert_eq!(state.records[3]["type"], "AAAA");
        assert_eq!(state.records[3]["content"], TARGET_IP);
    }
}

#[tokio::test]
async fn keeps_a_single_equivalent_ipv6_address() {
    let records = vec![record(
        "active",
        "HOST.EXAMPLE.COM",
        "AAAA",
        "2001:0db8:0000:0000:0000:0000:0000:0003",
    )];
    let mut api = MockApi::new(records.clone());
    assert_eq!(api.update().await, DnsUpdateOutcome::Unchanged);
    let state = api.state.lock().unwrap();
    assert_eq!(state.records, records);
    assert!(state.mutations.is_empty());
}

#[tokio::test]
async fn creates_when_no_aaaa_record_exists() {
    let existing = record("a", HOSTNAME, "A", "192.0.2.1");
    let mut api = MockApi::new(vec![existing.clone()]);
    assert_eq!(api.update().await, DnsUpdateOutcome::Updated);
    let state = api.state.lock().unwrap();
    assert_eq!(state.records[0], existing);
    assert_eq!(state.records[1]["content"], TARGET_IP);
    assert_eq!(state.mutations, ["POST new"]);
}

#[tokio::test]
async fn does_not_mutate_dns_when_any_list_page_fails() {
    for (status, response) in [
        (
            403,
            json!({"success": true, "result": [], "result_info": {"total_pages": 2}}),
        ),
        (
            200,
            json!({"success": false, "result": [], "result_info": {"total_pages": 2}}),
        ),
        (200, json!({"success": true, "result": []})),
    ] {
        let records = vec![
            record("first", HOSTNAME, "AAAA", "2001:db8::1"),
            record("second", HOSTNAME, "AAAA", TARGET_IP),
        ];
        let mut api = MockApi::new(records.clone());
        api.state.lock().unwrap().list_failure = Some((2, status, response));
        assert_eq!(api.update().await, DnsUpdateOutcome::Failed);
        let state = api.state.lock().unwrap();
        assert_eq!(state.records, records);
        assert!(state.mutations.is_empty());
    }
}

#[tokio::test]
async fn attempts_all_deletes_but_never_creates_after_a_failed_delete() {
    for (status, success) in [(200, false), (403, true)] {
        let failed = record("first", HOSTNAME, "AAAA", "2001:db8::1");
        let mut api = MockApi::new(vec![
            failed.clone(),
            record("second", HOSTNAME, "AAAA", TARGET_IP),
        ]);
        api.state.lock().unwrap().delete_failure = Some(("first".to_string(), status, success));
        assert_eq!(api.update().await, DnsUpdateOutcome::Failed);
        let state = api.state.lock().unwrap();
        assert_eq!(state.records, [failed]);
        assert_eq!(state.mutations, ["DELETE first", "DELETE second"]);
    }
}
