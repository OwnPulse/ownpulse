# Integrations

OwnPulse can pull data from external services to complement your manual entries and Apple Health data. The **Sources** page shows all available and connected integrations.

## Viewing connected sources

The Sources page displays each integration with its current status:

- **Connected** -- the integration is active and syncing data on schedule.
- **Disconnected** -- the integration was removed or its authorization expired.
- **Error** -- the last sync attempt failed. Check the error message for details.

Each connected source also shows the timestamp of its last successful sync.

## Connecting a new source

To connect an integration, tap the **Connect** button next to the source name. You will be redirected to the third-party service to authorize OwnPulse. After granting access, you are returned to OwnPulse and the initial data sync begins automatically.

!!! note "OAuth tokens"
    OwnPulse stores integration tokens encrypted with AES-256-GCM. Tokens are only used to fetch your data and are never shared or transmitted to any other service.

## Disconnecting a source

Tap **Disconnect** next to any connected integration. This immediately stops all future syncs for that source. Data that was already synced remains in your OwnPulse account -- disconnecting does not delete historical data. If you want to remove the data as well, use the data export feature to review what exists and contact your instance administrator for selective deletion.

## Source preferences

When multiple sources report the same metric (for example, heart rate from both Apple Health and a Garmin watch), OwnPulse needs to know which source is authoritative. Go to **Settings > Source Preferences** to configure priority per metric type. The preferred source is used for display and analysis; data from other sources is still stored and available in exports.

## Sync schedule

Connected integrations sync automatically on a recurring schedule. The exact interval depends on the integration. You do not need to manually trigger syncs, but you can force an immediate sync from the Sources page if needed.

## Troubleshooting

If an integration shows an error status, try disconnecting and reconnecting it. This refreshes the OAuth token. If the error persists, verify that your account on the third-party service is still active and that you have not revoked OwnPulse's access from the third-party settings.
