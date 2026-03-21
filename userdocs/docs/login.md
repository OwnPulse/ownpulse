# Login & Account Setup

OwnPulse supports two authentication methods. Both work on the web dashboard and the iOS app.

## Google OAuth

The fastest way to get started. On the login screen, tap **Sign in with Google**. You will be redirected to Google's consent screen, where you authorize OwnPulse to use your Google account for authentication. OwnPulse only requests your email and profile name -- it does not access your Google data.

After authorization, you are redirected back to OwnPulse and logged in immediately. If this is your first login, your account is created automatically.

## Username and password

For instances that do not use Google OAuth, the instance administrator creates your account and provides your credentials. Enter your username and password on the login screen and tap **Sign in**.

## Sessions and tokens

OwnPulse uses JWT (JSON Web Token) authentication. Your session token expires periodically for security. When it does, you will be redirected to the login screen and need to sign in again. The refresh token extends your session transparently in most cases, so you should not need to re-authenticate frequently during normal use.

!!! note "Security details"
    On the web, your JWT is held in memory and your refresh token is stored as an httpOnly cookie -- it is never accessible to JavaScript. On iOS, tokens are stored in the system Keychain. OwnPulse never stores authentication tokens in localStorage or UserDefaults.

## Multi-device access

You can be logged in on multiple devices simultaneously. Each device maintains its own session. Logging out on one device does not affect sessions on other devices.

## Troubleshooting

If you are stuck on the login screen after clicking **Sign in with Google**, check that your browser allows popups and redirects from your OwnPulse domain. If you are using a username/password account and cannot log in, contact your instance administrator to reset your password.
