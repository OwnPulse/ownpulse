# Users and Use Cases

This document defines who OwnPulse is for, what they're trying to do, and how the platform serves them. Every feature should trace back to a real user need described here. If a proposed feature doesn't serve any of these users, question whether it belongs.

---

## Target Users

OwnPulse is for people who take health optimization seriously and are frustrated that no single platform treats all their data as legitimate.

### Who OwnPulse is for

**Health optimizers** — people running structured protocols (peptides, supplement stacks, fasting regimens) who want to correlate objective metrics with subjective outcomes. They track more variables than mainstream apps support and need everything on one timeline.

**Quantified self practitioners** — people who measure extensively and want to run their own analysis. They have data spread across Apple Health, Garmin, Oura, CGMs, lab results, and manual logs. They want overlays and correlations, not siloed dashboards.

**Chronic condition managers** — people dialing in interventions for conditions that require ongoing tuning. They need to track what they're taking, how they feel, and what their biomarkers say — and see whether changes are working over weeks and months.

**Self-hosters and privacy-conscious users** — people who want full control over their health data. They'll run their own instance before trusting a third-party service. AGPL-3.0 and `helm upgrade --install` are features, not implementation details.

**Cooperative-minded contributors** — people willing to share anonymized data to help the community learn. They want to see what interventions work for people like them, and they want their contribution to be opt-in, transparent, and revocable.

### Who OwnPulse is NOT for

- **Casual fitness trackers** — people satisfied with step counts and sleep scores. OwnPulse solves deeper questions.
- **People who need clinical-grade tools** — OwnPulse is not a medical device and does not provide clinical decision support.
- **People who want a managed, zero-setup experience** — OwnPulse is opinionated about data ownership; hosted is available, but the user is always in control.

---

## Core Use Cases

### 1. Protocol tracking and correlation

**User:** Running a peptide protocol (BPC-157, TB-500), a supplement stack, or an off-label intervention.

**What they do:**
- Log interventions: substance, dose, unit, route, timing, fasted state
- Track objective metrics from wearables: HRV, resting HR, sleep quality, glucose
- Complete daily check-ins: energy, mood, focus, recovery, libido (1-10 scales, <30 sec)
- View the timeline to see if metrics shift after protocol changes
- (Phase 3) Run before/after analysis on specific interventions

**Why OwnPulse:** No mainstream app lets you log BPC-157 as a valid intervention and correlate it with HRV from your Apple Watch. The platform is non-judgmental — all substance names are valid data.

### 2. Multi-source health timeline

**User:** Wears an Apple Watch and an Oura Ring, has a Dexcom CGM, gets quarterly blood work, and wants to see it all together.

**What they do:**
- Connect data sources via OAuth (Garmin, Oura, Dexcom) or native sync (HealthKit)
- Resolve deduplication conflicts when multiple sources report the same metric
- Set source-of-truth preferences per metric type
- View a unified timeline with all metrics overlaid, toggleable by source
- Zoom into specific date ranges to investigate patterns

**Why OwnPulse:** These data sources don't talk to each other. Apple Health is the closest to a unified view, but it can't show your calendar load, lab results, or intervention timing alongside your HRV.

### 3. Meeting load and lifestyle correlation

**User:** Knowledge worker who suspects their meeting schedule affects sleep, HRV, and energy.

**What they do:**
- Connect Google Calendar
- View meeting count and total meeting minutes per day on the timeline
- Overlay with HRV, sleep quality, and energy check-in scores
- (Phase 3) See correlation analysis: "days with 4+ hours of meetings correlate with 15% lower next-day HRV"

**Why OwnPulse:** No health app treats calendar data as a health signal. Meeting load is an intervention — it affects recovery, sleep, and subjective well-being.

### 4. Lab result tracking

**User:** Gets regular blood work (quarterly, or more frequently during protocol changes).

**What they do:**
- (Phase 2) Upload PDF from Quest or LabCorp — parser extracts structured results
- Or enter results manually with reference ranges
- View per-marker trend lines over time
- See out-of-range flagging against reference ranges
- Correlate lab markers with interventions and lifestyle data

**Why OwnPulse:** Lab portals show one panel at a time. OwnPulse shows marker trends across panels and correlates them with everything else.

### 5. Subjective and contextual tracking

**User:** Wants to track things that don't come from a device — symptoms, events, environmental factors, freeform notes.

**What they do:**
- Log observations: events (cold plunge, sauna), scales (pain 1-10), symptoms (brain fog, GI distress), context tags (travel, high altitude), environmental data (temperature, AQI), notes
- View observations on the same timeline as objective metrics
- Autocomplete suggests names used by the community (anonymized counts)

**Why OwnPulse:** The `observations` system is extensible without schema changes. Users create their own tracking vocabulary. Community-wide autocomplete creates de facto standards without mandating them.

### 6. Full data ownership and export

**User:** Wants to own their data and be able to leave at any time.

**What they do:**
- Export everything: JSON, CSV, or FHIR R4
- Export is streaming — works for any data volume
- Delete their account and all data (hard delete, not soft)
- Self-host their own instance for maximum control

**Why OwnPulse:** This is a core principle, not a feature. Data portability and self-hosting are non-negotiable.

### 7. Cooperative learning (Phase 2+)

**User:** Willing to share anonymized data to learn from the community's collective experiments.

**What they do:**
- Opt in to sharing, per dataset, with explicit consent
- See aggregate insights: "47 members track BPC-157; average HRV change after 4 weeks is +X"
- Revoke consent at any time — takes effect immediately
- Genetic data sharing requires separate, stricter consent

**Why OwnPulse:** Individual n=1 experiments are noisy. A cooperative of people running similar protocols and sharing anonymized data creates a community research capability that no single user has alone.

---

## User Journeys by Phase

### Phase 1 (current)

1. Sign up (web) or sign in (iOS)
2. Connect data sources: HealthKit (iOS), Google Calendar, Garmin, Oura
3. Resolve deduplication if sources overlap
4. Log interventions, check-ins, and observations (web + iOS)
5. View unified timeline (web)
6. Export data (web)

### Phase 2

All of Phase 1, plus:
- Connect Dexcom CGM
- Upload lab PDFs
- Upload genetic data (23andMe, VCF)
- Opt into cooperative data sharing
- FHIR R4 export

### Phase 3

All of Phase 2, plus:
- Correlation explorer (web)
- Intervention before/after reports
- Insight cards ("your HRV drops after high-meeting days")
- iOS dashboard with sparklines and trend views

---

## Designing for These Users

When planning a feature, ask:

1. **Which user(s) does this serve?** Reference the use cases above by number.
2. **What does the user see?** Describe the interaction from their perspective before touching technical details.
3. **Does this work for self-hosters?** If it requires a cloud service, it's not ready.
4. **Is this non-judgmental?** No substance validation, no health warnings, no moralizing.
5. **Does this respect data ownership?** Can the user export this data? Delete it? Revoke sharing?
