# Integrations

OwnPulse can pull data from external services to complement your manual entries and Apple Health data. The web **Sources** page lists the integrations you've connected.

## Viewing connected sources

The web **Sources** page lists the integrations you've connected, each shown as **Connected** with a **Disconnect** button and, if a sync has ever failed, the error from that attempt. Garmin, Oura, and Google Calendar also get a **Sync now** button there to trigger a fetch immediately instead of waiting for the next scheduled run. It only lists sources you've already authorized -- it does not show a full catalog of available-but-not-yet-connected integrations, except for Google Calendar, which always gets a row (with connecting from the web not available yet -- see below).

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

Connected Garmin, Oura, and Google Calendar integrations sync automatically every 15 minutes, fetching data since your last successful sync (or the last 7 days, on first connect). You don't need to manually trigger syncs, but the web Sources page has a **Sync now** button next to each of these if you don't want to wait for the next scheduled run.

If a sync attempt fails (for example, the third-party service is temporarily unavailable), OwnPulse leaves your last-synced timestamp where it was, so nothing from that time window is lost -- the next scheduled sync picks up from the same point and retries automatically.

## Google Calendar

Google Calendar integration reads your meeting schedule to compute two numbers per day -- **meeting count** and **total meeting minutes** -- so you can correlate schedule load with your health metrics. It is strictly read-only (OwnPulse never modifies your calendar) and strictly aggregate: event titles, descriptions, attendees, and locations are never read into or stored by OwnPulse -- OwnPulse asks Google to not even send them, rather than fetching and discarding them. All-day entries (holidays, out-of-office blocks), out-of-office/focus-time/working-location events, and meetings you declined aren't counted. Days are grouped by UTC date, which may occasionally shift a late-evening meeting onto the next day if you're west of UTC.

This is separate from linking your Google account for sign-in (**Settings > Linked Accounts**) -- Google Calendar access is authorized independently, since it requires its own permission scope. Once connected, Google Calendar syncs automatically every 15 minutes (same schedule as Garmin/Oura) and the integration appears on the Sources page like any other connected source.

!!! note "Connecting from the web isn't available yet"
    The Sources page shows Google Calendar's Connect button as disabled with a "coming soon" label. Web browser navigation can't carry your session token the way the app's own API calls do, and the backend's connect route currently requires one on both the initial request and the return leg -- there's no working path today, regardless of how or when you signed in. This is tracked for a fix; once it lands, Connect will work from the web for any signed-in session.

## MyChart and other patient portals

OwnPulse can import lab results from MyChart (Epic) and other patient portals that support the SMART-on-FHIR standard. This is currently an **iOS-only** feature: open **Settings > Lab Results** in the iOS app to connect a provider. You'll be sent through your provider's own authorization page, then returned to the app. Imported labs appear alongside any you entered manually or uploaded as a PDF, and re-syncing skips results you've already imported. Requires the server operator to have configured MyChart support (`MYCHART_CLIENT_ID`) -- it may not be available on every self-hosted instance.
