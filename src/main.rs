use crate::services::get_ipaddress;

mod models;
mod services;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let hostname = std::env::var("DDNS_HOSTNAME").expect("DDNS_HOSTNAME not set");
    let zone = std::env::var("DDNS_ZONE").expect("DDNS_ZONE not set");
    let token = std::env::var("DDNS_CLOUDFLARE_TOKEN").expect("DDNS_CLOUDFLARE_TOKEN not set");

    let hostname = hostname.as_str();

    let initial_delay = 2;
    let exp = 2;
    let max_delay = 60;

    let mut delay = initial_delay;

    let mut dns_provider = services::DnsProvider::new(zone, token);

    loop {
        tokio::time::sleep(std::time::Duration::from_mins(delay)).await;

        let ip_address = get_ipaddress("AAAA".to_string()).await;
        let current_records = dns_provider.get_dns_records(hostname).await;

        let existing_record = current_records.iter().find(|r| r.r#type == "AAAA");

        if let Some(record) = existing_record
            && record.content == ip_address
        {
            println!(
                "[DnsProvider] No update needed for {} (AAAA): {}",
                hostname, ip_address
            );

            delay = std::cmp::min(max_delay, delay * exp);
            continue;
        }

        println!(
            "[DnsProvider] Creating DNS record for {} (AAAA): {}",
            hostname, ip_address
        );

        dns_provider
            .upsert_dns_record(
                existing_record,
                hostname,
                &ip_address,
                "AAAA",
                Some("Created by DDNS client"),
            )
            .await;

        println!("[DnsProvider] DNS record created successfully.");
    }
}
