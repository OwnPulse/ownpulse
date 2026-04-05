# Getting Started

This guide walks you through your first five minutes with OwnPulse -- from signing in to viewing your data.

## Step 1: Sign in

Open your OwnPulse web dashboard in a browser. If you have an invite link, open it to go directly to the registration page.

- **Google OAuth** -- tap **Sign in with Google** and authorize OwnPulse. Your account is created automatically on first login.
- **Username and password** -- choose a username and password on the registration page, or enter existing credentials on the login page.

!!! note "Invite codes"
    Registration requires an invite code. If you do not have one, request one at ownpulse.health or ask your administrator (self-hosted deployments).

## Step 2: Set up the iOS app (optional)

!!! note "iOS only"
    Skip this step if you do not have an iPhone.

1. Download the OwnPulse app from TestFlight or the App Store.
2. Sign in with Google (the same account you used on the web).
3. Go to **Settings > Request HealthKit Access** and grant permissions for the health data categories you want to sync.
4. The app begins syncing your Apple Health data automatically.

## Step 3: Enter your first check-in

On the web dashboard, go to the **Data Entry** page and open the **Check-ins** tab. Rate your energy, mood, focus, recovery, and libido on a scale of 1 to 10, then submit. You now have your first data point.

## Step 4: View your timeline

Go to the **Dashboard** page. You will see your recent check-in and, if you connected HealthKit, your sleep chart. As you add more data over the coming days, the timeline fills in and patterns become visible.

## Platform overview

OwnPulse runs on two platforms that share the same account and data.

**Web app** -- the full experience. The web dashboard gives you access to all data entry forms (check-ins, interventions, health records, observations, lab results), the timeline and charts, data export, source management, and account settings.

**iOS app** -- focused on HealthKit sync. The iOS app syncs your Apple Health data (sleep, heart rate, HRV, and more) in the background. It shows a sleep and HRV chart on its home screen and provides a **Sync Now** button for manual sync. Manual data entry is done on the web.

Both platforms use the same API and authentication. Data entered on the web is visible on iOS, and HealthKit data synced from iOS appears on the web dashboard.

## Next steps

- [Manual Data Entry](data-entry.md) -- learn about all the data types you can log
- [Apple Health](apple-health.md) -- detailed HealthKit setup and sync behavior
- [Dashboard & Timeline](timeline.md) -- understand the charts and visualizations
- [Privacy & Security](privacy.md) -- how your data is protected
