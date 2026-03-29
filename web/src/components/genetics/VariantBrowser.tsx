// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { geneticsApi } from "../../api/genetics";
import styles from "./VariantBrowser.module.css";

const CHROMOSOMES = ["All", ...Array.from({ length: 22 }, (_, i) => String(i + 1)), "X", "Y", "MT"];
const PER_PAGE = 50;

export function VariantBrowser() {
  const [expanded, setExpanded] = useState(false);
  const [page, setPage] = useState(1);
  const [chromosome, setChromosome] = useState("All");
  const [rsidSearch, setRsidSearch] = useState("");

  const { data, isLoading, error } = useQuery({
    queryKey: [
      "genetics",
      "list",
      page,
      chromosome === "All" ? undefined : chromosome,
      rsidSearch || undefined,
    ],
    queryFn: () =>
      geneticsApi.list({
        page,
        per_page: PER_PAGE,
        chromosome: chromosome === "All" ? undefined : chromosome,
        rsid: rsidSearch || undefined,
      }),
    enabled: expanded,
  });

  const totalPages = data ? Math.ceil(data.total / data.per_page) : 0;

  return (
    <section className={styles.section}>
      <button
        type="button"
        className={styles.toggle}
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <h2 className={styles.heading}>Raw Data Browser</h2>
        <span className={styles.arrow}>{expanded ? "Collapse" : "Expand"}</span>
      </button>

      {expanded && (
        <div className={styles.content}>
          <div className={styles.filters}>
            <div className={styles.filterGroup}>
              <label htmlFor="chrom-filter" className="op-label">
                Chromosome
              </label>
              <select
                id="chrom-filter"
                className="op-select"
                value={chromosome}
                onChange={(e) => {
                  setChromosome(e.target.value);
                  setPage(1);
                }}
              >
                {CHROMOSOMES.map((c) => (
                  <option key={c} value={c}>
                    {c === "All" ? "All chromosomes" : `Chr ${c}`}
                  </option>
                ))}
              </select>
            </div>

            <div className={styles.filterGroup}>
              <label htmlFor="rsid-search" className="op-label">
                Search rsID
              </label>
              <input
                id="rsid-search"
                type="text"
                className="op-input"
                placeholder="e.g. rs1801133"
                value={rsidSearch}
                onChange={(e) => {
                  setRsidSearch(e.target.value);
                  setPage(1);
                }}
              />
            </div>
          </div>

          {isLoading && <p className="op-empty">Loading variants...</p>}

          {error && (
            <p className="op-error-msg">Failed to load variants: {(error as Error).message}</p>
          )}

          {data && data.records.length === 0 && <p className="op-empty">No variants found.</p>}

          {data && data.records.length > 0 && (
            <>
              <table className="op-table">
                <thead>
                  <tr>
                    <th>rsID</th>
                    <th>Chromosome</th>
                    <th>Position</th>
                    <th>Genotype</th>
                  </tr>
                </thead>
                <tbody>
                  {data.records.map((record) => (
                    <tr key={`${record.rsid}-${record.position}`}>
                      <td>{record.rsid}</td>
                      <td>{record.chromosome}</td>
                      <td>{record.position.toLocaleString()}</td>
                      <td>
                        <code>{record.genotype}</code>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <div className={styles.pagination}>
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  disabled={page <= 1}
                  onClick={() => setPage((p) => p - 1)}
                >
                  Previous
                </button>
                <span className={styles.pageInfo}>
                  Page {data.page} of {totalPages} ({data.total.toLocaleString()} total)
                </span>
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  disabled={page >= totalPages}
                  onClick={() => setPage((p) => p + 1)}
                >
                  Next
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </section>
  );
}
