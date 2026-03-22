# User Management & Invites

This page covers admin-only features for managing users and invite codes. You need the **admin** role to access these features.

## Accessing the admin panel

On the web dashboard, go to **Settings**. If you are an admin, you will see a **User Management** section. On iOS, go to **Settings > User Management**.

## Invite system

By default, new users need an invite code to create an account. As an admin, you create invite codes and share them with people you want to grant access.

### Creating an invite code

1. Go to **Settings > User Management > Invites**.
2. Tap **Create Invite**.
3. Optionally set a **label** (for your reference, such as "for Alex"), a **maximum number of uses**, and an **expiry time**.
4. Tap **Create**. The invite code is generated.
5. Copy the invite link and send it to the person you want to invite.

The invite link looks like `https://app.yourdomain.com/register?invite=XXXXXX`. The recipient opens it, chooses a username and password (or signs in with Google), and their account is created.

### Invite limits and expiry

- **Max uses** -- limits how many people can register with a single code. Leave blank for unlimited uses.
- **Expiry** -- sets how long the code remains valid. After it expires, it cannot be used. Leave blank for no expiry.
- A code that has reached its max uses or has expired is automatically rejected during registration.

### Revoking an invite

To immediately prevent a code from being used, tap **Revoke** next to the invite in the list. Revoking a code does not affect users who already registered with it.

### Viewing invite usage

The invite list shows each code's label, how many times it has been used (out of the maximum, if set), when it expires, and whether it has been revoked.

## Managing users

### Viewing all users

The Users section lists all registered users with their username, authentication method, role, and status.

### Changing user roles

Use the role dropdown next to a user to change their role between **admin** and **user**. Role changes take effect on the user's next API request.

!!! warning "Admin access"
    Granting admin access gives full control over all users and invite codes. Only assign admin to people you trust with instance-level management.

### Disabling a user

To lock someone out without deleting their data, change their status to **Disabled**. A disabled user is rejected immediately on their next request -- they cannot log in or use the API until re-enabled.

To re-enable a user, change their status back to **Active**. Their access is restored immediately and all their data is intact.

!!! note "You cannot disable yourself"
    To prevent accidental lockout, admins cannot disable or delete their own account through the admin panel. Use the account deletion option in your own Settings if you want to remove your own account.

### Deleting a user

!!! warning "This action is permanent"
    Deleting a user permanently removes their account and all associated data. This cannot be undone.

!!! note "Disable before deleting"
    Before deleting a user, consider disabling their account first. This locks them out immediately while preserving their data, giving them time to export it. Once you are satisfied, you can proceed with deletion.

To delete a user:

1. Go to **Settings > User Management > Users**.
2. Tap **Delete** next to the user.
3. Confirm the deletion.

All of the user's data is permanently removed, including health records, check-ins, interventions, observations, lab results, integration tokens, and export history.

## Disabling vs. deleting

| | Disabled | Deleted |
|---|---|---|
| User can log in | No | No (account gone) |
| Data preserved | Yes | No |
| Reversible | Yes -- set status back to Active | No |
| Use case | Temporary lockout, policy violation, investigate suspicious activity | User requests deletion, or permanent removal |

If you are unsure, disable first. You can always delete later, but you cannot undo a deletion.

## Registration flow

When a new user receives an invite link:

1. They open the link in a browser. The registration page loads with the invite code pre-filled.
2. They choose a username and password, or tap **Sign in with Google**.
3. Their account is created and they are logged in immediately.

On iOS, users can enter the invite code manually during sign-up if they do not have the link.

!!! note "Open signups"
    If your instance administrator has set `REQUIRE_INVITE` to `false`, anyone can register without an invite code. The invite system is still available for tracking who invited whom, but it is not enforced.
