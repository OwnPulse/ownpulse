// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import type { TemplateListItem } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import styles from "./TemplateCard.module.css";

interface TemplateCardProps {
  template: TemplateListItem;
}

export function TemplateCard({ template }: TemplateCardProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [startDate, setStartDate] = useState(() => new Date().toISOString().slice(0, 10));
  const [showDate, setShowDate] = useState(false);

  const copyMutation = useMutation({
    mutationFn: () => protocolsApi.copyTemplate(template.id, startDate),
    onSuccess: (protocol) => {
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      navigate(`/protocols/${protocol.id}`);
    },
  });

  return (
    <div className={`op-card ${styles.card}`}>
      <p className={styles.name}>{template.name}</p>
      {template.description && <p className={styles.description}>{template.description}</p>}
      <div className={styles.tags}>
        {template.tags.map((tag) => (
          <span key={tag} className={styles.tag}>
            {tag}
          </span>
        ))}
      </div>
      <span className={styles.meta}>
        {template.duration_days} days &middot; {template.line_count} substance
        {template.line_count !== 1 ? "s" : ""}
      </span>
      {showDate ? (
        <div className={styles.footer}>
          <input
            type="date"
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className={styles.dateInput}
          />
          <button
            type="button"
            className="op-btn op-btn-primary op-btn-sm"
            disabled={copyMutation.isPending}
            onClick={() => copyMutation.mutate()}
          >
            {copyMutation.isPending ? "Copying..." : "Start"}
          </button>
          <button
            type="button"
            className="op-btn op-btn-ghost op-btn-sm"
            onClick={() => setShowDate(false)}
          >
            Cancel
          </button>
        </div>
      ) : (
        <div className={styles.footer}>
          <button
            type="button"
            className="op-btn op-btn-primary op-btn-sm"
            onClick={() => setShowDate(true)}
          >
            Use This Protocol
          </button>
        </div>
      )}
      {copyMutation.isError && (
        <p className="op-error-msg">{(copyMutation.error as Error).message}</p>
      )}
    </div>
  );
}
