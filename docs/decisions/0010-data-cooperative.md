# ADR-0010: Data Cooperative Model and Privacy Compliance

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse handles sensitive personal health data. The platform must decide: what legal and ethical framework governs how data is collected, stored, used, and shared? This decision shapes the product model, the business model, the legal structure, and the technical architecture simultaneously.

Several models exist for health data platforms:

- **Traditional SaaS:** company owns and monetizes user data; users accept a ToS
- **Privacy-focused SaaS:** company stores data but doesn't sell it; revenue from subscriptions
- **Open source self-hosted:** code is open, users run their own instance, no shared data
- **Data cooperative:** users collectively own and govern the platform; opt-in aggregate data sharing with user compensation; nonprofit or cooperative legal structure

The platform also has regulatory obligations. Health data is subject to specific protections in multiple jurisdictions: GDPR (EU), CCPA (California), PIPEDA (Canada). Non-compliance exposes the cooperative and its users to legal risk and undermines trust.

---

## Decision

Operate as a **data cooperative** under a nonprofit structure (legal structure TBD with counsel). Implement full GDPR, CCPA, and PIPEDA compliance at launch. Apply these principles:

1. **Data sovereignty:** user health data is stored on the user's infrastructure (self-hosted) or on the cooperative's infrastructure in the user's chosen region. Nothing leaves the user's instance without explicit opt-in.

2. **Full portability:** users can export 100% of their data at any time in open JSON, CSV, or FHIR R4 format. No friction, no delays, no data held hostage.

3. **Opt-in cooperative sharing:** users may choose to contribute anonymized aggregate data to a research marketplace. Consent is per-dataset, granular, and revocable immediately. Genetic data requires a separate, stricter consent.

4. **User compensation:** when cooperative data is sold to researchers, 80% flows back to consenting users (as hosting credits or charitable donations); 20% covers cooperative operating costs.

5. **Non-judgmental:** the platform stores what users choose to track, including grey-market supplements, off-label medications, and experimental protocols. No validation or flagging of substance names.

---

## Alternatives Considered

### Traditional subscription SaaS (privacy-focused)

Charge users a monthly fee; don't sell their data. Simple business model, clear value exchange.

Rejected because:
- Doesn't differentiate from existing health tracking apps.
- Users don't truly own their data — the company does.
- No mechanism for users to benefit from the collective value of their data.
- Harder to build community trust when the company controls everything.

### Fully decentralized (no cooperative hosting)

Self-hosted only. No cooperative instance. No shared data.

Rejected because:
- Self-hosting is a barrier for most users. The majority of people who would benefit from OwnPulse cannot run a VPS.
- Without any shared layer, there's no cooperative value — no aggregate insights, no research data marketplace, no collective governance.
- The open source project needs a sustainable funding model. Cooperative hosting provides it.

### Open core with proprietary hosted tier

Open source the core, charge for hosted features or enterprise integrations.

Rejected because:
- Creates misaligned incentives — the best features migrate to the paid tier.
- Undermines the cooperative model — the organization benefits from user data without sharing that benefit with users.
- AGPL-3.0 is a better fit than open core for a cooperative.

### HIPAA-compliant medical data platform

Target clinical use cases, obtain HIPAA Business Associate Agreement status, integrate with EHRs.

Rejected for Phase 1 because:
- HIPAA compliance significantly increases legal and operational overhead.
- OwnPulse is consumer-facing (personal health tracking), not a covered entity.
- HIPAA becomes relevant only if clinicians use OwnPulse in a treatment context. Phase 1 targets individuals.
- Can revisit if clinician use cases emerge organically.

---

## GDPR Compliance Architecture

The GDPR compliance model is built into the data schema and API, not bolted on:

**Lawful basis:** explicit consent (Article 9 for special category health data). Every data type requires a separate consent at signup. Consent stored with timestamp and privacy policy version.

**Data subject rights implemented as API endpoints:**
- Access: `GET /api/v1/export/json` — full export, immediate, no delay
- Erasure: `DELETE /api/v1/account` — anonymizes all records within 72 hours, audit trail preserved
- Portability: export in machine-readable open schema
- Restriction: `POST /api/v1/processing/restrict/:dataset` — suspends processing without deletion
- Objection: `DELETE /api/v1/sharing/consent/:dataset` — stops cooperative sharing immediately

**Data residency:** EU users' data stays on EU infrastructure (Hetzner, Germany). Enforced via user's `data_region` field set at signup. No cross-region personal data replication.

**Sub-processor DPAs:** required with Hetzner, DigitalOcean, and any transactional email provider before accepting EU users.

**Article 27 EU representative:** required before accepting EU users. Can be a third-party service (~$300/yr).

---

## Anonymization Standard for Research Data

When cooperative data is sold to researchers, the anonymization standard applied:

- k-anonymity with k ≥ 50: no record is distinguishable from at least 49 others in the dataset
- Differential privacy noise added to aggregate query results
- No quasi-identifiers: age capped to decade, geography capped to country
- Time resolution reduced to weekly aggregates
- Buyers receive aggregate query access only — no raw exports

This standard is defined in `docs/cooperative/data-sharing.md` and is reviewed annually.

---

## Consequences

**Positive:**
- Users have genuine ownership and control — trust is built on architecture, not promises.
- GDPR compliance is a feature for EU users and a marketing differentiator.
- The non-judgmental data model opens the platform to a community (quantified self, longevity, peptides) that is underserved by mainstream health apps.
- Cooperative structure creates alignment — the platform succeeds when users succeed.
- Research data marketplace creates a sustainable revenue stream that doesn't depend on advertising or selling user data without consent.

**Negative / tradeoffs:**
- Legal and compliance overhead is significant, especially at launch. Requires qualified legal counsel.
- The cooperative/nonprofit structure limits certain commercial strategies.
- GDPR compliance (DPAs, EU representative, data residency) adds operational complexity and cost.
- Full data portability (anytime, any format) removes lock-in — a deliberate tradeoff against user retention.

**Risks:**
- Legal costs exceed early revenue. Mitigate with grants (RWJF, Mozilla Foundation, Knight Foundation) targeted at health data cooperatives.
- The research data marketplace is legally complex (CCPA "sale of data" question, IRB requirements). Mitigate by deferring marketplace launch to Phase 3, after legal review.
- Genetic data sharing raises ethical and legal concerns beyond standard health data. Mitigate with a separate, stricter consent tier and by never selling individual genetic records.

---

## References

- GDPR text: https://gdpr-info.eu
- CCPA: https://oag.ca.gov/privacy/ccpa
- PIPEDA: https://www.priv.gc.ca/en/privacy-topics/privacy-laws-in-canada/the-personal-information-protection-and-electronic-documents-act-pipeda/
- Open Humans Foundation (reference cooperative): https://www.openhumans.org
- Data cooperatives overview: https://thedataeconomylab.com/2020/06/16/a-typology-of-data-intermediaries/
- GDPR Article 27 (EU representative): https://gdpr-info.eu/art-27-gdpr/
