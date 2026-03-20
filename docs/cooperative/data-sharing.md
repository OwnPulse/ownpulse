# Cooperative Data Sharing

OwnPulse enables users to contribute anonymized health data to a research marketplace. Participation is voluntary, granular, and revocable.

See [ADR-0010](../decisions/0010-data-cooperative.md) for the full rationale.

## Consent Model

- **Opt-in only.** No data is shared by default. Users explicitly grant consent per dataset.
- **Per-dataset granularity.** Users can consent to share health records but not genetic data, or vice versa. Each dataset requires a separate consent action.
- **Immediate revocation.** Users can revoke consent at any time. Revocation takes effect immediately -- no grace period. Already-shared aggregate data cannot be un-shared, but no new data is included.
- **Genetic data requires separate consent.** Genetic records (`dataset = 'genetics'`) have a stricter, independent consent tier. Health data consent does not imply genetic data consent.

## Anonymization Standard

Before any data enters the research marketplace, it is anonymized:

- **k-anonymity with k >= 50:** No record is distinguishable from at least 49 others in the dataset.
- **Differential privacy:** Noise is added to aggregate query results.
- **No quasi-identifiers:** Age is capped to decade, geography is capped to country.
- **Time resolution reduced:** Weekly aggregates only. No individual timestamps.
- **Aggregate query access only:** Buyers receive query access to aggregate data. No raw individual records are ever exported.

## Revenue Distribution

When cooperative data is sold to researchers:

- **80%** flows back to consenting users (as hosting credits or charitable donations, user's choice).
- **20%** covers cooperative operating costs (infrastructure, legal, administration).

Revenue is distributed proportionally based on the volume of data each user contributed to the datasets involved.

## Technical Implementation

### `sharing_consents` Table

The `sharing_consents` table is the trust boundary. Every cooperative aggregate query checks this table before including a user's data.

- `dataset`: which data category (e.g., `health`, `genetics`)
- `consented_at`: when consent was granted
- `revoked_at`: when consent was revoked (null if active)
- `privacy_policy_version`: version of the privacy policy the user consented under

### Query-Time Enforcement

Consent is checked at query time, not at ingest. This means:
- Revoking consent immediately removes the user from future queries.
- No data is deleted on revocation -- the user's data remains on their instance but is excluded from cooperative aggregates.

### Audit Trail

All consent grants and revocations are logged with timestamps. Users can view their consent history via the API.

## What Is Not Shared

- Individual records are never shared. Only aggregates.
- Genetic data is never included in general health aggregates.
- Intervention names are anonymized in aggregates (grouped by category, not individual substance names).
- No data from users who have not explicitly opted in.
