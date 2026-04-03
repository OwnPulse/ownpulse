// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";
import type { ProtocolExport } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import styles from "./ImportModal.module.css";

interface ImportModalProps {
  onClose: () => void;
}

export function ImportModal({ onClose }: ImportModalProps) {
  const navigate = useNavigate();
  const [dragOver, setDragOver] = useState(false);
  const [parsed, setParsed] = useState<ProtocolExport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const importMutation = useMutation({
    mutationFn: (data: ProtocolExport) => protocolsApi.importFromFile(data),
    onSuccess: (protocol) => {
      onClose();
      navigate(`/protocols/${protocol.id}`);
    },
  });

  const handleFileContent = useCallback((content: string) => {
    try {
      const data = JSON.parse(content) as ProtocolExport;
      if (!data.name || !data.duration_days || !Array.isArray(data.lines)) {
        setError("Invalid protocol file: missing required fields.");
        setParsed(null);
        return;
      }
      setParsed(data);
      setError(null);
    } catch {
      setError("Invalid JSON file.");
      setParsed(null);
    }
  }, []);

  const readFile = useCallback(
    (file: File) => {
      const reader = new FileReader();
      reader.onload = () => {
        if (typeof reader.result === "string") {
          handleFileContent(reader.result);
        }
      };
      reader.readAsText(file);
    },
    [handleFileContent],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const file = e.dataTransfer.files[0];
      if (file) readFile(file);
    },
    [readFile],
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    setDragOver(false);
  }, []);

  const handleFileInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) readFile(file);
    },
    [readFile],
  );

  const handleOverlayKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  const substances = parsed?.lines.map((l) => l.substance).join(", ") ?? "";
  const dropzoneClass = `${styles.dropzone}${dragOver ? ` ${styles.dragOver}` : ""}`;

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: modal overlay backdrop
    <div
      className={styles.overlay}
      role="presentation"
      onClick={onClose}
      onKeyDown={handleOverlayKeyDown}
    >
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: stopPropagation prevents overlay dismiss */}
      <div className={`op-card ${styles.card}`} role="dialog" onClick={(e) => e.stopPropagation()}>
        <h2>Import Protocol</h2>

        <label
          className={dropzoneClass}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
        >
          <span className={styles.dropzoneText}>Drop a .json protocol file here</span>
          <input
            type="file"
            accept=".json"
            onChange={handleFileInput}
            className={styles.hiddenInput}
          />
          <span className={`op-btn op-btn-secondary ${styles.chooseBtn}`}>Choose file</span>
        </label>

        {error && <p className="op-error-msg">{error}</p>}

        {parsed && (
          <div className={styles.preview}>
            <p className={styles.previewName}>{parsed.name}</p>
            <p className={styles.previewMeta}>
              {parsed.duration_days} days &middot; {parsed.lines.length} line
              {parsed.lines.length !== 1 ? "s" : ""}
              <br />
              Substances: {substances}
            </p>
          </div>
        )}

        {importMutation.isError && (
          <p className="op-error-msg">Import failed: {(importMutation.error as Error).message}</p>
        )}

        <div className={styles.actions}>
          <button type="button" className="op-btn op-btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="op-btn op-btn-primary"
            disabled={!parsed || importMutation.isPending}
            onClick={() => parsed && importMutation.mutate(parsed)}
          >
            {importMutation.isPending ? "Importing..." : "Import"}
          </button>
        </div>
      </div>
    </div>
  );
}
