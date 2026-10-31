# Agent Note: Replace all old DNS addresses before creating one

## Problem

The DNS lookup reads only the first page of the zone's records and filters by hostname locally. Existing records outside that page are invisible to the updater, allowing it to create another address without deleting all old addresses. The duplicate-cleanup branch also keeps an existing target address rather than implementing the requested delete-all-then-create behavior.

## Decision

The Cloudflare lookup uses the case-insensitive `name.exact` filter and reads every page using `result_info.total_pages`. HTTP failures, unsuccessful API responses, malformed responses, and missing pagination metadata abort the update without mutating DNS. A failed later page never exposes a partial record list to the updater.

The updater manages only the configured hostname's AAAA records. A single record whose parsed IPv6 address equals the target remains unchanged, including equivalent IPv6 spellings. Otherwise, the updater attempts to delete every old AAAA record, including any record already containing the target address. Only after every deletion succeeds does it create exactly one new AAAA record. Any deletion failure prevents creation. Other hostnames and record types remain untouched.

The update operation is separate from the polling loop so stateful HTTP regression scenarios exercise the same implementation as the binary. The provider retains the production Cloudflare API URL by default; tests use a private alternate endpoint with a local HTTP proxy. No endpoint configuration is added to the CLI or environment contract.

Cloudflare's filter and pagination contract: [List DNS Records](https://developers.cloudflare.com/api/resources/dns/subresources/records/methods/list/).

## Alternatives considered

**Keep a matching record and delete only duplicates.** This avoids a temporary DNS gap but does not implement the requested deletion of all old records before creating a new one.

**Request one larger page.** This reduces the likelihood of missing old records but does not guarantee a complete list. Exhausting pagination is required before deletion starts.

**Update one record in place.** This leaves other old records behind and cannot establish the single-address invariant.

## Consequences

Delete-before-create introduces a temporary gap with no AAAA record. If creation fails after successful deletion, no AAAA record remains until a later polling attempt succeeds. Failed deletions can leave some old records, but the client does not add another address alongside them. This is not a transaction and does not coordinate concurrent writers.

## Testing

- Before the fix, the regression for an already-existing target address fails: the planner selects duplicate cleanup instead of replacing all records.
- `cargo test`: five stateful HTTP tests pass, covering paginated replacement with and without an existing target, preservation of unrelated records, equivalent IPv6 spellings, initial creation, later-page failures, and deletion failures.
- `cargo build`: succeeds.
- The actual debug binary runs against isolated local HTTPS endpoints through its existing proxy configuration. The successful scenario observes IP discovery, zone discovery, three list pages, both old-record deletions, and one creation, leaving one target AAAA and the original A record. A second scenario observes an HTTP 403 deletion failure, attempts the remaining deletion, and creates nothing.
- `cargo clippy --all-targets`: completes with the pre-existing `needless_return` warning in `get_zone_id`.
