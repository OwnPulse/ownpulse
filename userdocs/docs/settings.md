# Settings & Account

The Settings page is where you manage your account preferences, data sources, exports, and account lifecycle.

## Source preferences

When you have multiple data sources reporting the same metric (for example, heart rate from both Apple Health and a connected wearable), you can set which source is considered authoritative. Go to **Settings > Source Preferences** and select your preferred source for each metric type.

The preferred source is used for display on the Timeline and in analysis. Data from non-preferred sources is still stored, still appears in exports, and can be viewed in detailed data views. Changing your preference takes effect immediately for all future data display.

See [Integrations](integrations.md) for more on connecting and managing data sources.

### How source preferences work

When multiple sources report the same metric, the preferred source's value is shown on the dashboard and used in analysis. Non-preferred source data is kept in the database -- it is not deleted, just deprioritized. You can always see data from all sources in exports and detailed data views. Changing your preference takes effect immediately for all future display.

## Export data

The Export section lets you download your complete dataset in JSON or CSV format. Exports are streamed directly and follow the OwnPulse Open Schema for maximum portability.

Exports include health records, check-ins, interventions, observations, lab results, and sleep data. See [Data Export](export.md) for full details on export formats and the Open Schema.

## Audit log

OwnPulse logs access to sensitive operations for your records. Logged operations include data exports, account deletion, and bulk operations. The audit log is accessible via the API (`GET /account/audit-log`) and shows the last 100 entries. This log is for your reference only -- no one else has access to it.

## Linked accounts

You can link multiple sign-in methods to your account. Go to **Settings > Linked Accounts** to see which providers are currently connected.

### Linking a new provider

Tap **Link** next to the provider you want to add (Apple, Google, or password). For Apple and Google, you will be redirected to their sign-in flow. For password, you will be prompted to choose a password. Once linked, you can use any of your linked methods to sign in.

### Unlinking a provider

Tap **Unlink** next to the provider you want to remove. You must always have at least one sign-in method remaining -- OwnPulse will not let you unlink your last provider.

## Account management

### Changing your profile

If you signed in with Apple or Google OAuth, your display name and email are pulled from your provider account. These update automatically if you change them on the provider side.

For username/password accounts, contact your instance administrator to update your email or reset your password.

### Deleting your account

!!! warning "This action is permanent"
    Account deletion cannot be undone. Export your data first if you want to keep a copy.

To delete your account:

1. Go to **Settings**.
2. Scroll to **Delete Account**.
3. Tap **Delete Account**. You will be asked to confirm.
4. After confirmation, your account and all associated data are permanently removed from the system. This includes all health records, check-ins, interventions, observations, lab results, integration tokens, and export history.

Deletion is immediate. There is no grace period and no way to recover a deleted account. If you are on a shared instance, deletion only affects your data -- other users are not impacted.
