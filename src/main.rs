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
        let current_records = dns_provider.get_dns_records(hostname).await;

        let aaaa_records: Vec<_> = current_records
            .iter()
            .filter(|record| record.r#type == "AAAA")
            .collect();
        let selected_record = aaaa_records
            .iter()
            .copied()
            .find(|record| record.content == ip_address)
            .or_else(|| aaaa_records.first().copied());

        if let Some(record) = selected_record
            && record.content == ip_address
            && aaaa_records.len() == 1
        {
            println!(
                "[DnsProvider] No update needed for {} (AAAA): {}",
                hostname, ip_address
            );

            delay = std::cmp::min(max_delay, delay * exp);
            continue;
        }

        println!(
            "[DnsProvider] Updating DNS record for {} (AAAA): {}",
            hostname, ip_address
        );

        let active_record_id = match selected_record {
            Some(record) if record.content == ip_address => {
                println!(
                    "[DnsProvider] Existing DNS record {} already has the target IP.",
                    record.id
                );
                Some(record.id.clone())
            }
            Some(record) => {
                println!("[DnsProvider] Updating existing DNS record {}.", record.id);
                dns_provider
                    .update_dns_record(
                        &record.id,
                        hostname,
                        &ip_address,
                        "AAAA",
                        Some("Created by DDNS client"),
                    )
                    .await
                    .map(|record| record.id)
            }
            None => dns_provider
                .create_dns_record(
                    hostname,
                    &ip_address,
                    "AAAA",
                    Some("Created by DDNS client"),
                )
                .await
                .map(|record| record.id),
        };

        let Some(active_record_id) = active_record_id else {
            println!(
                "[DnsProvider] Failed to update DNS record for {} (AAAA): {}. Selected record: {:?}. Current AAAA records: {:?}",
                hostname,
                ip_address,
                selected_record,
                aaaa_records
            );
            continue;
        };

        let duplicate_record_ids: Vec<_> = aaaa_records
            .iter()
            .filter(|record| record.id != active_record_id)
            .map(|record| record.id.clone())
            .collect();

        let duplicate_record_count = duplicate_record_ids.len();
        let mut failed_duplicate_delete_count = 0;

        for record_id in duplicate_record_ids {
            println!("[DnsProvider] Deleting duplicate DNS record {}.", record_id);

            if !dns_provider.delete_dns_record(&record_id).await {
                failed_duplicate_delete_count += 1;
                println!(
                    "[DnsProvider] Failed to delete duplicate DNS record {}.",
                    record_id
                );
            }
        }

        if failed_duplicate_delete_count == 0 {
            println!(
                "[DnsProvider] DNS record updated successfully: {}.",
                active_record_id
            );
        } else {
            println!(
                "[DnsProvider] DNS record {} is active, but failed to delete {}/{} duplicate record(s).",
                active_record_id, failed_duplicate_delete_count, duplicate_record_count
            );
        }
    }
}
