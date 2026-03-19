# ADR-0003: AGPL-3.0 License

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse is a data cooperative built on the principle that users own their health data. The license choice is a direct expression of the platform's values — it determines who can use the code, whether commercial operators can build proprietary services on top of it, and whether the community's contributions can be captured by a well-resourced competitor.

The specific risk we're designing against: a company takes the codebase, deploys it as a hosted service, adds proprietary integrations, and competes with the cooperative using the cooperative's own work — without giving anything back. This has happened repeatedly in open source (MongoDB → DocumentDB, Elasticsearch → OpenSearch, Redis → Valkey, Terraform → OpenTofu).

The cooperative's sustainability model depends on being the canonical hosted offering. If a larger operator can run a closed-source fork, it undermines the cooperative's ability to fund itself through the hosted tier and research data marketplace.

Secondary consideration: the license signals values to the community. Health data is sensitive. A permissive license (MIT, Apache) that allows proprietary forks sends a different signal than a copyleft license about how seriously we take user data sovereignty.

---

## Decision

License the codebase under **GNU Affero General Public License v3.0 (AGPL-3.0)**.

The open data schema (`schema/open-schema.json` and `schema/open-schema.md`) is licensed separately under **Creative Commons CC0** (public domain). Any application can read a valid OwnPulse export without license obligation.

---

## Alternatives Considered

### MIT or Apache 2.0 (permissive)

Maximum contributor friendliness. Anyone can use the code for anything. Widely used in open source health projects (Open Humans uses Apache-style licensing for some components).

Rejected because it permits exactly the capture scenario we're trying to prevent. A cloud provider could fork OwnPulse, add their own HealthKit sync and proprietary integrations, and offer a "better" hosted service without contributing back. The cooperative cannot compete with a company that has more engineering resources if that company can use our work for free.

### GPL-3.0 (copyleft, but not network copyleft)

GPL-3.0 requires source disclosure when you distribute the software. But "distribution" under GPL doesn't include running a web service — a company can run a GPL-licensed application as a SaaS and never release their modifications. This is the "ASP loophole."

Rejected because OwnPulse is primarily a web service. GPL-3.0 provides no protection against the SaaS capture scenario. AGPL closes the ASP loophole by requiring source disclosure when the software is used over a network.

### SSPL (Server Side Public License, MongoDB's license)

SSPL goes further than AGPL — it requires that anyone offering the software as a service must also open-source their entire service stack, including all tools used to run it. This is maximally protective.

Rejected because:
- SSPL is not approved by the Open Source Initiative (OSI) and is not considered an open source license by the OSI definition.
- It creates friction for academic researchers and legitimate self-hosters who might use adjacent proprietary tools.
- It sends a hostile signal to the open source community.
- AGPL is sufficient for our purposes — it protects against the SaaS capture scenario while remaining a recognized open source license.

### Business Source License (BSL / BUSL)

A time-delayed license — code is initially proprietary (or restricted), then converts to open source after a period (typically 4 years). Used by HashiCorp (Terraform), CockroachDB, and others.

Rejected because:
- It is not open source and OwnPulse's cooperative model requires genuine openness.
- It contradicts the "users own their data" principle — how can users trust a platform that restricts how others can run it?
- It creates uncertainty for contributors who don't know what they're contributing to.

### Open Core (proprietary extensions on an open core)

Keep the core open source (MIT or Apache) but build proprietary integrations or features for the hosted tier.

Rejected because it creates misaligned incentives — the most valuable features migrate to the proprietary layer, and self-hosters get an increasingly crippled product. This is incompatible with the cooperative model.

---

## Consequences

**Positive:**
- Any company offering OwnPulse as a hosted service must publish their modifications under AGPL-3.0. Improvements flow back to the community.
- The cooperative is the natural home for hosted OwnPulse — competitors cannot offer a closed-source "better" version.
- AGPL is a recognized OSI-approved open source license — the project is unambiguously open source.
- Signals alignment with user data sovereignty values.
- Compatible with contributions from individuals and most academic institutions.

**Negative / tradeoffs:**
- Some companies have internal policies against using AGPL software, even as a dependency. This may reduce corporate contributions.
- More restrictive than MIT/Apache — some developers and companies avoid AGPL projects on principle.
- Requires AGPL license headers in every source file — a small operational overhead.
- Incompatible with some permissive-licensed dependencies (check before adding any new dep).

**Risks:**
- A company argues their modifications are not "conveyed" under AGPL and refuses to publish source. AGPL enforcement requires legal action and is difficult in practice without an organization dedicated to it (like the Software Freedom Conservancy). Mitigate by building a community where cooperation is more valuable than defection.
- A well-resourced actor forks before significant community investment, rebrands, and captures the market. Mitigate with strong branding, cooperative governance, and being the first-mover for the hosted tier.

---

## License Compatibility Notes

Dependencies must be compatible with AGPL-3.0. Compatible licenses include MIT, Apache 2.0, BSD, ISC, MPL-2.0, LGPL, GPL, and AGPL itself. Incompatible licenses include proprietary/commercial licenses and SSPL.

Before adding any new dependency, verify its license is AGPL-compatible. The CI pipeline should include a license audit step (`cargo deny check licenses` for Rust, `license-checker` for npm).

---

## References

- AGPL-3.0 full text: https://www.gnu.org/licenses/agpl-3.0.html
- OSI AGPL page: https://opensource.org/license/agpl-v3
- AGPL and SaaS: https://www.gnu.org/licenses/why-affero-gpl.html
- Open Humans Foundation license approach (reference): https://github.com/OpenHumans/open-humans
