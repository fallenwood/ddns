use crate::services::get_ipaddress;
use std::net::Ipv6Addr;

mod models;
mod services;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, PartialEq, Eq)]
enum DnsUpdatePlan {
    Unchanged {
        active_record_id: String,
    },
    DeleteDuplicates {
        active_record_id: String,
        duplicate_record_ids: Vec<String>,
    },
    Replace {
        old_record_ids: Vec<String>,
    },
    Create,
}

fn plan_dns_update(records: &[&models::DnsRecord], ip_address: &Ipv6Addr) -> DnsUpdatePlan {
    let matching_record = records.iter().copied().find(|record| {
        record
            .content
            .parse::<Ipv6Addr>()
            .is_ok_and(|record_ip| record_ip == *ip_address)
    });

    match matching_record {
        Some(record) => {
            let duplicate_record_ids: Vec<_> = records
                .iter()
                .filter(|candidate| candidate.id != record.id)
                .map(|candidate| candidate.id.clone())
                .collect();

            if duplicate_record_ids.is_empty() {
                DnsUpdatePlan::Unchanged {
                    active_record_id: record.id.clone(),
                }
            } else {
                DnsUpdatePlan::DeleteDuplicates {
                    active_record_id: record.id.clone(),
                    duplicate_record_ids,
                }
            }
        }
        None if records.is_empty() => DnsUpdatePlan::Create,
        None => DnsUpdatePlan::Replace {
            old_record_ids: records.iter().map(|record| record.id.clone()).collect(),
        },
    }
}

async fn delete_dns_records(
    dns_provider: &mut services::DnsProvider,
    record_ids: &[String],
) -> usize {
    let mut failed_delete_count = 0;

    for record_id in record_ids {
        println!("[DnsProvider] Deleting DNS record {}.", record_id);

        if !dns_provider.delete_dns_record(record_id).await {
            failed_delete_count += 1;
            println!("[DnsProvider] Failed to delete DNS record {}.", record_id);
        }
    }

    failed_delete_count
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
        let normalized_ip_address = ip_address.to_string();
        let current_records = dns_provider.get_dns_records(hostname).await;

        let aaaa_records: Vec<_> = current_records
            .iter()
            .filter(|record| record.r#type == "AAAA")
            .collect();
        let update_plan = plan_dns_update(&aaaa_records, &ip_address);

        if let DnsUpdatePlan::Unchanged { active_record_id } = &update_plan {
            println!(
                "[DnsProvider] No update needed for {} (AAAA): {} (record {}).",
                hostname, normalized_ip_address, active_record_id
            );

            delay = std::cmp::min(max_delay, delay * exp);
            continue;
        }

        println!(
            "[DnsProvider] Updating DNS record for {} (AAAA): {}",
            hostname, normalized_ip_address
        );

        let active_record_id = match update_plan {
            DnsUpdatePlan::DeleteDuplicates {
                active_record_id,
                duplicate_record_ids,
            } => {
                let duplicate_record_count = duplicate_record_ids.len();
                let failed_delete_count =
                    delete_dns_records(&mut dns_provider, &duplicate_record_ids).await;

                if failed_delete_count == 0 {
                    println!(
                        "[DnsProvider] DNS record {} is active and all {} duplicate record(s) were deleted.",
                        active_record_id, duplicate_record_count
                    );
                } else {
                    println!(
                        "[DnsProvider] DNS record {} is active, but failed to delete {}/{} duplicate record(s).",
                        active_record_id, failed_delete_count, duplicate_record_count
                    );
                }

                continue;
            }
            DnsUpdatePlan::Replace { old_record_ids } => {
                let old_record_count = old_record_ids.len();
                let failed_delete_count =
                    delete_dns_records(&mut dns_provider, &old_record_ids).await;

                if failed_delete_count > 0 {
                    println!(
                        "[DnsProvider] Aborting creation of the new record because {}/{} old record(s) could not be deleted.",
                        failed_delete_count, old_record_count
                    );
                    continue;
                }

                println!(
                    "[DnsProvider] Deleted all {} old record(s); creating the new record.",
                    old_record_count
                );
                dns_provider
                    .create_dns_record(
                        hostname,
                        &normalized_ip_address,
                        "AAAA",
                        Some("Created by DDNS client"),
                    )
                    .await
                    .map(|record| record.id)
            }
            DnsUpdatePlan::Create => dns_provider
                .create_dns_record(
                    hostname,
                    &normalized_ip_address,
                    "AAAA",
                    Some("Created by DDNS client"),
                )
                .await
                .map(|record| record.id),
            DnsUpdatePlan::Unchanged { .. } => unreachable!(),
        };

        let Some(active_record_id) = active_record_id else {
            println!(
                "[DnsProvider] Failed to create DNS record for {} (AAAA): {}.",
                hostname, normalized_ip_address
            );
            continue;
        };

        println!(
            "[DnsProvider] DNS record created successfully: {}.",
            active_record_id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dns_record(id: &str, content: &str) -> models::DnsRecord {
        models::DnsRecord {
            id: id.to_string(),
            name: "host.example.com".to_string(),
            r#type: "AAAA".to_string(),
            content: content.to_string(),
            proxied: false,
            ttl: 60,
            comment: None,
        }
    }

    #[test]
    fn replaces_all_records_when_the_ip_changes() {
        let first_record = dns_record("first", "2001:db8::1");
        let second_record = dns_record("second", "2001:db8::2");
        let records = [&first_record, &second_record];
        let target_ip = "2001:db8::3".parse().unwrap();

        assert_eq!(
            plan_dns_update(&records, &target_ip),
            DnsUpdatePlan::Replace {
                old_record_ids: vec!["first".to_string(), "second".to_string()]
            }
        );
    }

    #[test]
    fn deletes_only_duplicates_when_the_target_ip_exists() {
        let active_record = dns_record("active", "2001:db8::3");
        let old_record = dns_record("old", "2001:db8::1");
        let records = [&old_record, &active_record];
        let target_ip = "2001:db8::3".parse().unwrap();

        assert_eq!(
            plan_dns_update(&records, &target_ip),
            DnsUpdatePlan::DeleteDuplicates {
                active_record_id: "active".to_string(),
                duplicate_record_ids: vec!["old".to_string()]
            }
        );
    }

    #[test]
    fn treats_equivalent_ipv6_notation_as_unchanged() {
        let active_record = dns_record("active", "2001:0db8:0000:0000:0000:0000:0000:0001");
        let records = [&active_record];
        let target_ip = "2001:db8::1".parse().unwrap();

        assert_eq!(
            plan_dns_update(&records, &target_ip),
            DnsUpdatePlan::Unchanged {
                active_record_id: "active".to_string()
            }
        );
    }

    #[test]
    fn creates_a_record_when_none_exists() {
        let target_ip = "2001:db8::1".parse().unwrap();

        assert_eq!(plan_dns_update(&[], &target_ip), DnsUpdatePlan::Create);
    }
}
