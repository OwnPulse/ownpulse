// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";
import type { UploadResult } from "../../api/genetics";
import { geneticsApi } from "../../api/genetics";
import styles from "./UploadDropzone.module.css";

interface UploadDropzoneProps {
  compact?: boolean;
}

export function UploadDropzone({ compact = false }: UploadDropzoneProps) {
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [uploadResult, setUploadResult] = useState<UploadResult | null>(null);

  const mutation = useMutation({
    mutationFn: (file: File) => geneticsApi.upload(file),
    onSuccess: (result) => {
      setUploadResult(result);
      setSelectedFile(null);
      queryClient.invalidateQueries({ queryKey: ["genetics"] });
    },
  });

  const handleFile = useCallback((file: File) => {
    setSelectedFile(file);
    setUploadResult(null);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const file = e.dataTransfer.files[0];
      if (file) handleFile(file);
    },
    [handleFile],
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
      if (file) handleFile(file);
    },
    [handleFile],
  );

  const handleUpload = () => {
    if (selectedFile) {
      mutation.mutate(selectedFile);
    }
  };

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className={compact ? styles.compact : undefined}>
      <label
        className={`${styles.dropzone}${dragOver ? ` ${styles.dragOver}` : ""}${compact ? ` ${styles.dropzoneCompact}` : ""}`}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        data-testid="upload-dropzone"
      >
        {!compact && <span className={styles.heading}>Upload your genetic data</span>}
        <span className={styles.subtitle}>
          {compact ? "Upload new file" : "Supports 23andMe, AncestryDNA, and VCF files"}
        </span>

        <input
          ref={fileInputRef}
          type="file"
          accept=".txt,.csv,.vcf,.tsv,.zip"
          onChange={handleFileInput}
          className={styles.hiddenInput}
          data-testid="file-input"
        />

        <span className={`op-btn op-btn-secondary ${styles.chooseBtn}`}>Choose file</span>
      </label>

      {selectedFile && (
        <div className={styles.selected}>
          <p className={styles.fileName}>
            {selectedFile.name} ({formatSize(selectedFile.size)})
          </p>
          <button
            type="button"
            className="op-btn op-btn-primary"
            onClick={handleUpload}
            disabled={mutation.isPending}
          >
            {mutation.isPending ? "Uploading..." : "Upload"}
          </button>
        </div>
      )}

      {mutation.isPending && (
        <div className={styles.progress} data-testid="upload-progress">
          <div className={styles.progressBar} />
          <p className={styles.progressText}>Processing genetic data...</p>
        </div>
      )}

      {mutation.isError && (
        <p className="op-error-msg" data-testid="upload-error">
          Upload failed: {(mutation.error as Error).message}
        </p>
      )}

      {uploadResult && (
        <div className={`op-card ${styles.result}`} data-testid="upload-result">
          <p className="op-success-msg">Upload successful!</p>
          <dl className={styles.resultStats}>
            <div>
              <dt>Format</dt>
              <dd>{uploadResult.format}</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>{uploadResult.source}</dd>
            </div>
            <div>
              <dt>Total variants</dt>
              <dd>{uploadResult.total_variants.toLocaleString()}</dd>
            </div>
            <div>
              <dt>New</dt>
              <dd>{uploadResult.new_variants.toLocaleString()}</dd>
            </div>
            <div>
              <dt>Duplicates skipped</dt>
              <dd>{uploadResult.duplicates_skipped.toLocaleString()}</dd>
            </div>
          </dl>
        </div>
      )}
    </div>
  );
}
