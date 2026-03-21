# Data Export

You can export your complete OwnPulse dataset at any time. There are no restrictions, no waiting periods, and no limits on how often you export. Your data is yours.

## How to export

1. Go to **Settings**.
2. Under **Export Data**, choose your format:
    - **JSON** -- a complete structured dump of all your data. Best for backups, migration to another OwnPulse instance, or programmatic analysis.
    - **CSV** -- tabular format with one file per data type. Best for opening in spreadsheet applications like Excel or Google Sheets.
3. Tap **Export**. The download begins immediately.

## Streaming exports

Exports are streamed directly to your device. OwnPulse never buffers your complete dataset in memory on the server. This means exports work reliably regardless of how much data you have -- whether you have a week of check-ins or years of continuous health records.

## OwnPulse Open Schema

All exports follow the **OwnPulse Open Schema**, a documented, versioned data format licensed under CC0 (public domain). This means:

- Any tool or service can read your exported data without permission from OwnPulse.
- You can write scripts to process your exports using the published schema documentation.
- If you migrate to a different platform, your data is already in a portable, well-documented format.

The schema specification is published in the repository at `schema/open-schema.json` and documented in `schema/open-schema.md`.

## What is included

An export contains all data associated with your account:

- Daily check-ins (energy, mood, focus, recovery, libido)
- Interventions (substances, medications, supplements)
- Health records (all metrics from all sources)
- Observations (events, scales, symptoms, notes, context tags, environmental)
- Lab results
- Sleep data
- Source preferences

!!! note "Export audit"
    Every export you perform is logged for your records. You can see your export history in Settings. This is for your reference only -- no one else has access to this log.
