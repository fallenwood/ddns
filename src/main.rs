use crate::services::get_ipaddress;
use std::net::Ipv6Addr;

mod models;
mod services;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, PartialEq, Eq)]
enum DnsUpdateOutcome {
    Unchanged,
    Updated,
    Failed,
}

async fn update_dns_record(
    dns_provider: &mut services::DnsProvider,
    hostname: &str,
    ip_address: Ipv6Addr,
) -> DnsUpdateOutcome {
    let Some(mut records) = dns_provider.get_dns_records(hostname).await else {
        println!(
            "[DnsProvider] Aborting update: could not list all records for {}.",
            hostname
        );
        return DnsUpdateOutcome::Failed;
    };
    records.retain(|record| record.r#type == "AAAA");

    if let [record] = records.as_slice()
        && record.content.parse::<Ipv6Addr>().ok() == Some(ip_address)
    {
        println!(
            "[DnsProvider] No update needed for {} (AAAA): {} (record {}).",
            hostname, ip_address, record.id
        );
        return DnsUpdateOutcome::Unchanged;
    }

    let mut failed_delete_count = 0;
    for record in &records {
        println!("[DnsProvider] Deleting DNS record {}.", record.id);
        if !dns_provider.delete_dns_record(&record.id).await {
            failed_delete_count += 1;
        }
    }

    if failed_delete_count > 0 {
        println!(
            "[DnsProvider] Aborting creation of the new record because {}/{} old record(s) could not be deleted.",
            failed_delete_count,
            records.len()
        );
        return DnsUpdateOutcome::Failed;
    }

    println!(
        "[DnsProvider] Deleted all {} old record(s); creating {} (AAAA): {}.",
        records.len(),
        hostname,
        ip_address
    );
    match dns_provider
        .create_dns_record(
            hostname,
            &ip_address.to_string(),
            "AAAA",
            Some("Created by DDNS client"),
        )
        .await
    {
        Some(record) => {
            println!(
                "[DnsProvider] DNS record created successfully: {}.",
                record.id
            );
            DnsUpdateOutcome::Updated
        }
        None => {
            println!(
                "[DnsProvider] Failed to create DNS record for {} (AAAA): {}.",
                hostname, ip_address
            );
            DnsUpdateOutcome::Failed
        }
    }
}

#[tokio::main]
async fn main() {
    let hostname = std::env::var("DDNS_HOSTNAME").expect("DDNS_HOSTNAME not set");
    let zone = std::env::var("DDNS_ZONE").expect("DDNS_ZONE not set");
    let token = std::env::var("DDNS_CLOUDFLARE_TOKEN").expect("DDNS_CLOUDFLARE_TOKEN not set");
    let cf_proxy = std::env::var("DDNS_CF_PROXY")
        .ok()
        .filter(|proxy| !proxy.trim().is_empty());

    let hostname = hostname.as_str();

    let initial_delay = 2;
    let exp = 2;
    let max_delay = 60;

    let mut delay = initial_delay;

    let mut dns_provider = services::DnsProvider::new(zone, token, cf_proxy);

    loop {
        tokio::time::sleep(std::time::Duration::from_mins(delay)).await;

        let ip_address = get_ipaddress("AAAA".to_string()).await;
        let ip_address = match ip_address.parse::<Ipv6Addr>() {
            Ok(ip_address) => ip_address,
            Err(error) => {
                println!(
                    "[DnsProvider] Refusing to update {} (AAAA) with invalid IPv6 address '{}': {}",
                    hostname, ip_address, error
                );
                continue;
            }
        };
        if update_dns_record(&mut dns_provider, hostname, ip_address).await
            == DnsUpdateOutcome::Unchanged
        {
            delay = std::cmp::min(max_delay, delay * exp);
        }
    }
}
