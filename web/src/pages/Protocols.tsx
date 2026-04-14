// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type { ActiveRunResponse } from "../api/protocols";
import { protocolsApi } from "../api/protocols";
import { ImportModal } from "../components/protocols/ImportModal";
import { StartRunModal } from "../components/protocols/StartRunModal";
import { TemplateCard } from "../components/protocols/TemplateCard";
import styles from "./Protocols.module.css";

function doseBadgeText(run: ActiveRunResponse): string {
  if (run.doses_today === 0) return "No doses today";
  if (run.doses_completed_today >= run.doses_today) return "All done";
  const pending = run.doses_today - run.doses_completed_today;
  return `${pending} dose${pending !== 1 ? "s" : ""} pending`;
}

export default function Protocols() {
  const [showImport, setShowImport] = useState(false);
  const [startRunProtocol, setStartRunProtocol] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [selectedTag, setSelectedTag] = useState<string | null>(null);

  const {
    data: protocols,
    isLoading: protocolsLoading,
    isError: protocolsError,
  } = useQuery({
    queryKey: ["protocols"],
    queryFn: () => protocolsApi.list(),
  });

  const {
    data: activeRuns,
    isLoading: runsLoading,
    isError: runsError,
  } = useQuery({
    queryKey: ["active-runs"],
    queryFn: () => protocolsApi.activeRuns(),
  });

  const { data: templates } = useQuery({
    queryKey: ["protocol-templates"],
    queryFn: () => protocolsApi.listTemplates(),
  });

  const isLoading = protocolsLoading || runsLoading;
  const isError = protocolsError || runsError;

  const allTags = useMemo(() => {
    if (!templates) return [];
    const set = new Set<string>();
    for (const t of templates) {
      for (const tag of t.tags) set.add(tag);
    }
    return Array.from(set).sort();
  }, [templates]);

  const filteredTemplates = useMemo(() => {
    if (!templates) return [];
    if (!selectedTag) return templates;
    return templates.filter((t) => t.tags.includes(selectedTag));
  }, [templates, selectedTag]);

  return (
    <main className={`op-page ${styles.page}`}>
      <div className={styles.headerRow}>
        <h1>Protocols</h1>
        <div className={styles.headerActions}>
          <button type="button" className="op-btn op-btn-ghost" onClick={() => setShowImport(true)}>
            Import
          </button>
          <Link to="/protocols/new" className="op-btn op-btn-primary">
            New Protocol
          </Link>
        </div>
      </div>

      {showImport && <ImportModal onClose={() => setShowImport(false)} />}
      {startRunProtocol && (
        <StartRunModal
          protocolId={startRunProtocol.id}
          protocolName={startRunProtocol.name}
          onClose={() => setStartRunProtocol(null)}
        />
      )}

      {isLoading && <p>Loading...</p>}
      {isError && <p>Error loading protocols.</p>}

      {/* Active Runs section */}
      {!isLoading && !isError && activeRuns && activeRuns.length > 0 && (
        <section className={styles.activeRunsSection}>
          <h2>Active Runs</h2>
          <div className={styles.cardList}>
            {activeRuns.map((ar) => {
              const pct = Math.round(ar.progress_pct);
              const hasPending = ar.doses_today > 0 && ar.doses_completed_today < ar.doses_today;
              return (
                <Link
                  key={ar.id}
                  to={`/protocols/${ar.protocol_id}`}
                  className={`op-card ${styles.protocolCard} ${hasPending ? styles.activeRunHighlight : ""}`}
                >
                  <div className={styles.cardHeader}>
                    <span className={styles.cardName}>{ar.protocol_name}</span>
                    <span className={`${styles.badge} ${styles.badgeActive}`}>
                      {"\u25CF "}active
                    </span>
                    <span
                      className={`${styles.doseBadge} ${hasPending ? styles.doseBadgePending : styles.doseBadgeDone}`}
                    >
                      {doseBadgeText(ar)}
                    </span>
                  </div>
                  <div className={styles.progressBar}>
                    <div className={styles.progressFill} style={{ width: `${pct}%` }} />
                  </div>
                  <span className={styles.nextDose}>
                    Started {ar.start_date} &middot; {pct}% complete
                  </span>
                </Link>
              );
            })}
          </div>
        </section>
      )}

      {/* My Protocols section */}
      {!isLoading && !isError && (
        <section className={styles.myProtocolsSection}>
          <h2>My Protocols</h2>
          {(!protocols || protocols.length === 0) && (
            <p className={styles.emptyText}>No protocols yet. Create your first dosing protocol.</p>
          )}
          <div className={styles.cardList}>
            {protocols?.map((p) => (
              <div key={p.id} className={`op-card ${styles.protocolCard} ${styles.recipeCard}`}>
                <Link to={`/protocols/${p.id}`} className={styles.recipeLink}>
                  <div className={styles.cardHeader}>
                    <span className={styles.cardName}>{p.name}</span>
                  </div>
                  <span className={styles.nextDose}>{p.duration_days} days</span>
                </Link>
                <button
                  type="button"
                  className="op-btn op-btn-primary op-btn-sm"
                  onClick={() => setStartRunProtocol({ id: p.id, name: p.name })}
                >
                  Start
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      {templates && templates.length > 0 && (
        <section className={styles.templatesSection}>
          <h2>Community Templates</h2>
          {allTags.length > 0 && (
            <div className={styles.tagFilter}>
              <button
                type="button"
                className={`${styles.filterBtn} ${selectedTag === null ? styles.filterBtnActive : ""}`}
                onClick={() => setSelectedTag(null)}
              >
                All
              </button>
              {allTags.map((tag) => (
                <button
                  key={tag}
                  type="button"
                  className={`${styles.filterBtn} ${selectedTag === tag ? styles.filterBtnActive : ""}`}
                  onClick={() => setSelectedTag(selectedTag === tag ? null : tag)}
                >
                  {tag}
                </button>
              ))}
            </div>
          )}
          <div className={styles.templateGrid}>
            {filteredTemplates.map((t) => (
              <TemplateCard key={t.id} template={t} />
            ))}
          </div>
        </section>
      )}
    </main>
  );
}
