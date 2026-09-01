// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import type { AdherenceResponse, ProtocolRun, UpdateRunRequest } from "../api/protocols";
import { protocolsApi } from "../api/protocols";
import { DoseStatusGrid } from "../components/protocols/DoseStatusGrid";
import { StartRunModal } from "../components/protocols/StartRunModal";
import styles from "./ProtocolView.module.css";

function runStatusBadgeClass(status: ProtocolRun["status"]): string {
  if (status === "active") return styles.badgeActive;
  if (status === "paused") return styles.badgePaused;
  return styles.badgeCompleted;
}

function adherenceSummary(adherence: AdherenceResponse | undefined): string {
  if (!adherence) return "";
  if (adherence.adherence_pct == null) return "No closed days yet";
  const pct = Math.round(adherence.adherence_pct);
  return `${pct}% adherence · ${adherence.completed} done · ${adherence.skipped} skipped · ${adherence.missed} missed`;
}

export default function ProtocolView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [shareLink, setShareLink] = useState<string | null>(null);
  const [showStartRun, setShowStartRun] = useState(false);

  const {
    data: protocol,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["protocols", id],
    queryFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.get(id);
    },
    enabled: !!id,
  });

  const { data: runs } = useQuery({
    queryKey: ["protocol-runs", id],
    queryFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.listRuns(id);
    },
    enabled: !!id,
  });

  const activeRun = runs?.find((r) => r.status === "active") ?? null;

  const { data: adherence } = useQuery({
    queryKey: ["run-adherence", activeRun?.id],
    queryFn: () => {
      if (!activeRun) throw new Error("No active run");
      return protocolsApi.runAdherence(activeRun.id);
    },
    enabled: !!activeRun,
  });

  const shareMutation = useMutation({
    mutationFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.share(id);
    },
    onSuccess: (res) => {
      const link = `${window.location.origin}/protocols/shared/${res.token}`;
      setShareLink(link);
    },
  });

  const updateRunMutation = useMutation({
    mutationFn: ({ runId, data }: { runId: string; data: UpdateRunRequest }) =>
      protocolsApi.updateRun(runId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocol-runs", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["active-runs"] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.delete(id);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      navigate("/protocols");
    },
  });

  const handleExport = async () => {
    if (!id || !protocol) return;
    const data = await protocolsApi.exportProtocol(id);
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${protocol.name.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (!id) return <main className="op-page">Not found</main>;
  if (isLoading) return <main className="op-page">Loading...</main>;
  if (isError || !protocol) return <main className="op-page">Error loading protocol.</main>;

  const elapsedPct = activeRun ? Math.round(activeRun.progress_pct) : 0;

  return (
    <main className={`op-page ${styles.page}`}>
      {showStartRun && (
        <StartRunModal
          protocolId={protocol.id}
          protocolName={protocol.name}
          onClose={() => setShowStartRun(false)}
        />
      )}

      {/* Header */}
      <div className={styles.header}>
        <h1>{protocol.name}</h1>
      </div>

      <div className={styles.meta}>
        {protocol.duration_days} days
        {activeRun ? ` · Run started ${activeRun.start_date}` : ""}
      </div>

      {/* Adherence header — server-computed, closed-days-only */}
      {activeRun && (
        <div className={styles.progressSection}>
          <span className={styles.progressLabel}>{adherenceSummary(adherence)}</span>
        </div>
      )}

      {/* Elapsed-time progress bar (secondary to adherence) */}
      {activeRun && (
        <div className={styles.progressSection}>
          <span className={styles.progressLabel}>{elapsedPct}% of run elapsed</span>
          <div className={styles.progressBar}>
            <div className={styles.progressFill} style={{ width: `${elapsedPct}%` }} />
          </div>
        </div>
      )}

      {/* Runs section */}
      <section className={styles.runsSection}>
        <div className={styles.runsSectionHeader}>
          <h2>Runs</h2>
          <button
            type="button"
            className="op-btn op-btn-primary op-btn-sm"
            onClick={() => setShowStartRun(true)}
          >
            Start New Run
          </button>
        </div>
        {runs && runs.length > 0 ? (
          <div className={styles.runsList}>
            {runs.map((run) => (
              <div key={run.id} className={`op-card ${styles.runCard}`}>
                <div className={styles.runCardHeader}>
                  <span className={`${styles.badge} ${runStatusBadgeClass(run.status)}`}>
                    {run.status}
                  </span>
                  <span className={styles.runDate}>Started {run.start_date}</span>
                </div>
                <div className={styles.runActions}>
                  {run.status === "active" && (
                    <>
                      <button
                        type="button"
                        className="op-btn op-btn-ghost op-btn-sm"
                        onClick={() =>
                          updateRunMutation.mutate({
                            runId: run.id,
                            data: { status: "paused" },
                          })
                        }
                        disabled={updateRunMutation.isPending}
                      >
                        Pause
                      </button>
                      <button
                        type="button"
                        className="op-btn op-btn-ghost op-btn-sm"
                        onClick={() =>
                          updateRunMutation.mutate({
                            runId: run.id,
                            data: { status: "completed" },
                          })
                        }
                        disabled={updateRunMutation.isPending}
                      >
                        Complete
                      </button>
                    </>
                  )}
                  {run.status === "paused" && (
                    <button
                      type="button"
                      className="op-btn op-btn-ghost op-btn-sm"
                      onClick={() =>
                        updateRunMutation.mutate({
                          runId: run.id,
                          data: { status: "active" },
                        })
                      }
                      disabled={updateRunMutation.isPending}
                    >
                      Resume
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className={styles.emptyDoses}>No runs yet. Start your first run.</p>
        )}
      </section>

      {/* Dose status grid — backed by the server's per-day dose status;
          each scheduled cell is itself a Log/Skip/Undo control, so there is
          no separate "today's doses" list on this page anymore. */}
      <section className={styles.gridSection}>
        <h2>Schedule</h2>
        {activeRun ? (
          <DoseStatusGrid runId={activeRun.id} durationDays={protocol.duration_days} />
        ) : (
          <p className={styles.emptyDoses}>Start a run to see your schedule.</p>
        )}
      </section>

      {/* Actions */}
      <div className={styles.actions}>
        <button type="button" className="op-btn op-btn-ghost" onClick={handleExport}>
          Export
        </button>
        <button
          type="button"
          className="op-btn op-btn-ghost"
          onClick={() => shareMutation.mutate()}
          disabled={shareMutation.isPending}
        >
          Share
        </button>
        <button
          type="button"
          className="op-btn op-btn-danger"
          onClick={() => deleteMutation.mutate()}
          disabled={deleteMutation.isPending}
        >
          Delete
        </button>
      </div>

      {shareLink && (
        <div className={styles.shareLink}>
          <div className={styles.shareLinkRow}>
            <input type="text" readOnly value={shareLink} className={styles.shareLinkInput} />
            <button
              type="button"
              className="op-btn op-btn-ghost op-btn-sm"
              onClick={() => navigator.clipboard.writeText(shareLink)}
            >
              Copy
            </button>
          </div>
        </div>
      )}

      {/* Description */}
      {protocol.description && (
        <section>
          <h2>Description</h2>
          <p className={styles.description}>{protocol.description}</p>
        </section>
      )}
    </main>
  );
}
