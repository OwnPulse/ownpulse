# Login & Account Setup

OwnPulse supports three authentication methods. All work on both the web dashboard and the iOS app.

## Apple Sign-In

On the login screen, tap **Sign in with Apple**. You will be redirected to Apple's authentication flow. You can choose to share your real email address or use Apple's private relay address -- either works.

After authorization, you are returned to OwnPulse and logged in immediately. If this is your first login, your account is created automatically.

## Google OAuth

On the login screen, tap **Sign in with Google**. You will be redirected to Google's consent screen, where you authorize OwnPulse to use your Google account for authentication. OwnPulse only requests your email and profile name -- it does not access your Google data.

After authorization, you are redirected back to OwnPulse and logged in immediately. If this is your first login, your account is created automatically.

## Username and password

Enter your username and password on the login screen and tap **Sign in**.

## Registering a new account

If you have an invite link, open it in your browser. The registration page loads with the invite code pre-filled. Choose a username and password, or tap **Sign in with Apple** or **Sign in with Google** to register with your Apple or Google account.

If you have an invite code but not a link, go to the registration page manually and enter the code along with your chosen username and password.

!!! note "Invite codes"
    Most OwnPulse instances require an invite code to register. Ask your instance administrator for one. See [User Management & Invites](admin.md) for more on how invite codes work.

## Sessions and tokens

OwnPulse uses JWT (JSON Web Token) authentication. Your session token expires periodically for security. When it does, you will be redirected to the login screen and need to sign in again. The refresh token extends your session transparently in most cases, so you should not need to re-authenticate frequently during normal use.

!!! note "Security details"
    On the web, your JWT is held in memory and your refresh token is stored as an httpOnly cookie -- it is never accessible to JavaScript. On iOS, tokens are stored in the system Keychain. OwnPulse never stores authentication tokens in localStorage or UserDefaults.

## Multi-device access

You can be logged in on multiple devices simultaneously. Each device maintains its own session. Logging out on one device does not affect sessions on other devices.

## Troubleshooting

If you are stuck on the login screen after clicking **Sign in with Apple** or **Sign in with Google**, check that your browser allows popups and redirects from your OwnPulse domain. If you are using a username/password account and cannot log in, contact your instance administrator to reset your password.

## Security notes

- Login attempts are rate limited to 5 per minute per IP address to prevent brute-force attacks.
- Refresh tokens rotate automatically -- you do not need to do anything. If you are logged out unexpectedly, simply sign in again. The previous refresh token is invalidated on rotation, so there is no window for token reuse.
