// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { geneticsApi } from "../api/genetics";
import { InterpretationList } from "../components/genetics/InterpretationList";
import { SummaryCard } from "../components/genetics/SummaryCard";
import { UploadDropzone } from "../components/genetics/UploadDropzone";
import { VariantBrowser } from "../components/genetics/VariantBrowser";
import styles from "./Genetics.module.css";

export default function Genetics() {
  const queryClient = useQueryClient();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const {
    data: summary,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["genetics", "summary"],
    queryFn: () => geneticsApi.summary(),
  });

  const deleteMutation = useMutation({
    mutationFn: () => geneticsApi.deleteAll(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["genetics"] });
      setShowDeleteConfirm(false);
    },
  });

  const hasData = summary && summary.total_variants > 0;

  return (
    <main className="op-page">
      <div className="op-page-header">
        <h1>Genetics</h1>
      </div>

      {isLoading && <p className="op-empty">Loading genetic data...</p>}

      {error && (
        <p className="op-error-msg">Failed to load genetic data: {(error as Error).message}</p>
      )}

      {/* Upload section */}
      {!isLoading &&
        !error &&
        (hasData ? (
          <section className={styles.uploadSection}>
            <SummaryCard summary={summary} />
            <div className={styles.actions}>
              <UploadDropzone compact />
              <button
                type="button"
                className="op-btn op-btn-danger"
                onClick={() => setShowDeleteConfirm(true)}
              >
                Delete All Data
              </button>
            </div>
          </section>
        ) : (
          <UploadDropzone />
        ))}

      {/* Delete confirmation modal */}
      {showDeleteConfirm && (
        <div className={styles.modalOverlay} data-testid="delete-modal">
          <div className={`op-card ${styles.modal}`}>
            <h3 className={styles.modalTitle}>Delete all genetic data?</h3>
            <p className={styles.modalText}>
              This will permanently delete all uploaded genetic records and interpretations. This
              action cannot be undone.
            </p>
            <div className={styles.modalActions}>
              <button
                type="button"
                className="op-btn op-btn-ghost"
                onClick={() => setShowDeleteConfirm(false)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="op-btn op-btn-danger"
                onClick={() => deleteMutation.mutate()}
                disabled={deleteMutation.isPending}
              >
                {deleteMutation.isPending ? "Deleting..." : "Delete Everything"}
              </button>
            </div>
            {deleteMutation.isError && (
              <p className="op-error-msg">
                Delete failed: {(deleteMutation.error as Error).message}
              </p>
            )}
          </div>
        </div>
      )}

      {/* Interpretations section — only show when data exists */}
      {hasData && <InterpretationList />}

      {/* Raw variant browser — only show when data exists */}
      {hasData && <VariantBrowser />}
    </main>
  );
}
