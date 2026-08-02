# Integrations

OwnPulse can pull data from external services to complement your manual entries and Apple Health data. The web **Sources** page lists the integrations you've connected.

## Viewing connected sources

The web **Sources** page lists the integrations you've connected, each shown as **Connected** with a **Disconnect** button. It only lists sources you've already authorized -- it does not show a catalog of available-but-not-yet-connected integrations, and it does not surface a separate "error" or sync-failure status.

## Connecting a new source

Connections are initiated from the iOS app, not the web Sources page:

- **Garmin and Oura:** open **Settings > Wearables** in the iOS app to connect either wearable. The authorization page opens in a secure in-app browser; once you finish, the app shows the source as **Connected**. The first time you connect a wearable, OwnPulse offers to resolve any metrics that overlap with Apple Health so you can pick a source of truth.
- **Google Calendar:** see [Google Calendar](#google-calendar) below.
- **MyChart:** see [MyChart and other patient portals](#mychart-and-other-patient-portals) below.

Once connected, the source appears on the web Sources page, where you can disconnect it.

!!! note "OAuth tokens"
    OwnPulse stores integration tokens encrypted with AES-256-GCM. Tokens are only used to fetch your data and are never shared or transmitted to any other service.

## Disconnecting a source

Tap **Disconnect** next to any connected integration on the Sources page. This immediately stops all future syncs for that source. Data that was already synced remains in your OwnPulse account -- disconnecting does not delete historical data. If you want to remove the data as well, use the data export feature to review what exists and contact support or your administrator (self-hosted) for selective deletion.

## Source preferences

When multiple sources report the same metric (for example, heart rate from both Apple Health and a Garmin watch), OwnPulse needs to know which source is authoritative. Go to **Settings > Source Preferences** to configure priority per metric type. The preferred source is used for display and analysis; data from other sources is still stored and available in exports.

## Sync schedule

Connected Garmin and Oura integrations sync automatically every 15 minutes, fetching data since your last successful sync (or the last 7 days, on first connect). You do not need to manually trigger syncs. A `POST /integrations/garmin/sync` / `POST /integrations/oura/sync` endpoint exists for triggering a sync immediately instead of waiting for the next scheduled run, but there is no web or iOS button for it yet.

If a sync attempt fails (for example, the third-party service is temporarily unavailable), OwnPulse leaves your last-synced timestamp where it was, so nothing from that time window is lost -- the next scheduled sync picks up from the same point and retries automatically.

## Google Calendar

Google Calendar integration reads your meeting schedule to compute two numbers per day -- **meeting count** and **total meeting minutes** -- so you can correlate schedule load with your health metrics. It is strictly read-only (OwnPulse never modifies your calendar) and strictly aggregate: event titles, descriptions, attendees, and locations are never read into or stored by OwnPulse, only the count and duration of timed meetings. All-day entries (holidays, out-of-office blocks) aren't counted.

This is separate from linking your Google account for sign-in (**Settings > Linked Accounts**) -- Google Calendar access is authorized independently, since it requires its own permission scope. There is no web or iOS **Connect** button for it yet; connecting currently requires visiting `/api/v1/auth/google-calendar/login` on your OwnPulse instance while signed in. Once connected, Google Calendar syncs automatically every 15 minutes (same schedule as Garmin/Oura) and the integration appears on the Sources page like any other connected source.

## MyChart and other patient portals

OwnPulse can import lab results from MyChart (Epic) and other patient portals that support the SMART-on-FHIR standard. This is currently an **iOS-only** feature: open **Settings > Lab Results** in the iOS app to connect a provider. You'll be sent through your provider's own authorization page, then returned to the app. Imported labs appear alongside any you entered manually or uploaded as a PDF, and re-syncing skips results you've already imported. Requires the server operator to have configured MyChart support (`MYCHART_CLIENT_ID`) -- it may not be available on every self-hosted instance.
