// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { invitesApi } from "../api/invites";
import styles from "./InviteLanding.module.css";

const ERROR_MESSAGES: Record<string, string> = {
  expired: "This invite has expired. Ask the person who invited you for a new one.",
  revoked: "This invite is no longer valid. It was revoked by the admin.",
  exhausted: "This invite has already been used the maximum number of times.",
  not_found: "This invite code was not found. Double-check the link you received.",
};

function isExpiringSoon(expiresAt: string): boolean {
  const expiry = new Date(expiresAt);
  const now = new Date();
  const hoursRemaining = (expiry.getTime() - now.getTime()) / (1000 * 60 * 60);
  return hoursRemaining > 0 && hoursRemaining <= 24;
}

function formatExpiryDate(expiresAt: string): string {
  return new Date(expiresAt).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

export default function InviteLanding() {
  const { code } = useParams<{ code: string }>();

  const {
    data: invite,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["invite-check", code],
    queryFn: () => invitesApi.check(code ?? ""),
    enabled: !!code,
    retry: false,
  });

  return (
    <div className={styles.page}>
      <main className={styles.card}>
        <div className={styles.brand}>
          <div className={styles.brandName}>
            <span className={styles.brandNameOwn}>Own</span>
            <span className={styles.brandNamePulse}>Pulse</span>
          </div>
        </div>

        {isLoading && <p className={styles.loading}>Checking invite...</p>}

        {isError && (
          <>
            <h1 className={styles.errorTitle}>Something went wrong</h1>
            <p className={styles.errorMessage}>
              We could not verify this invite. Please try again later.
            </p>
            <Link to="/login" className={styles.footerLink}>
              Go to sign in
            </Link>
          </>
        )}

        {invite && !invite.valid && (
          <>
            <h1 className={styles.errorTitle}>Invite unavailable</h1>
            <p className={styles.errorMessage}>{ERROR_MESSAGES[invite.reason ?? "not_found"]}</p>
            <div className={styles.footer}>
              <Link to="/login" className={styles.footerLink}>
                Already have an account? Sign in
              </Link>
            </div>
          </>
        )}

        {invite?.valid && (
          <>
            <h1 className={styles.title}>You've been invited to OwnPulse</h1>
            <p className={styles.description}>
              OwnPulse is a personal health data platform that puts you in control of your health
              data.
            </p>

            <div className={styles.inviteDetails}>
              {invite.inviter_name && (
                <div className={styles.inviteDetail}>
                  <span className={styles.detailLabel}>Invited by</span>
                  <span className={styles.detailValue}>{invite.inviter_name}</span>
                </div>
              )}
              {invite.label && (
                <div className={styles.inviteDetail}>
                  <span className={styles.detailLabel}>Note</span>
                  <span className={styles.detailValue}>{invite.label}</span>
                </div>
              )}
              {invite.expires_at && (
                <div className={styles.inviteDetail}>
                  <span className={styles.detailLabel}>Expires</span>
                  <span className={styles.detailValue}>
                    {formatExpiryDate(invite.expires_at)}
                    {isExpiringSoon(invite.expires_at) && (
                      <span className={styles.expiresSoon}>Expires soon</span>
                    )}
                  </span>
                </div>
              )}
            </div>

            <Link to={`/register?invite=${code}`} className={styles.ctaButton}>
              Create Account
            </Link>

            <div className={styles.footer}>
              <Link to="/login" className={styles.footerLink}>
                Already have an account? Sign in
              </Link>
            </div>
          </>
        )}
      </main>
    </div>
  );
}
