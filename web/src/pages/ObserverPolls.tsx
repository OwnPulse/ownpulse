// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  type CreatePollRequest,
  type ObserverPollView,
  type OwnerResponseView,
  observerPollsApi,
  type Poll,
} from "../api/observer-polls";
import styles from "./ObserverPolls.module.css";

const DIMENSIONS = [
  { value: "energy", label: "Energy" },
  { value: "mood", label: "Mood" },
  { value: "focus", label: "Focus" },
  { value: "recovery", label: "Recovery" },
  { value: "libido", label: "Libido" },
  { value: "appearance", label: "Appearance" },
];

function todayDate() {
  return new Date().toISOString().slice(0, 10);
}

function CreatePollForm({ onCreated }: { onCreated: () => void }) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [customPrompt, setCustomPrompt] = useState("");
  const [selectedDimensions, setSelectedDimensions] = useState<string[]>(["energy", "mood"]);
  const [error, setError] = useState<string | null>(null);

  const createMutation = useMutation({
    mutationFn: (data: CreatePollRequest) => observerPollsApi.create(data),
    onSuccess: () => {
      setError(null);
      setName("");
      setCustomPrompt("");
      setSelectedDimensions(["energy", "mood"]);
      queryClient.invalidateQueries({ queryKey: ["observer-polls"] });
      onCreated();
    },
    onError: (err: Error) => {
      setError(err.message);
    },
  });

  const toggleDimension = (value: string) => {
    setSelectedDimensions((prev) =>
      prev.includes(value) ? prev.filter((v) => v !== value) : [...prev, value],
    );
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError("Name is required.");
      return;
    }
    if (trimmedName.length > 100) {
      setError("Name must be 100 characters or fewer.");
      return;
    }
    if (customPrompt.length > 500) {
      setError("Custom prompt must be 500 characters or fewer.");
      return;
    }
    if (selectedDimensions.length === 0) {
      setError("Select at least one dimension.");
      return;
    }
    createMutation.mutate({
      name: trimmedName,
      custom_prompt: customPrompt.trim() || undefined,
      dimensions: selectedDimensions,
    });
  };

  return (
    <form onSubmit={handleSubmit} className={`op-card ${styles.createForm}`}>
      <h3>Create Poll</h3>
      <div className="op-form-field">
        <label htmlFor="poll-name" className="op-label">
          Name
        </label>
        <input
          id="poll-name"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g., Daily mood check"
          className="op-input"
          maxLength={100}
          required
        />
      </div>
      <div className="op-form-field">
        <label htmlFor="poll-prompt" className="op-label">
          Custom Prompt (optional)
        </label>
        <textarea
          id="poll-prompt"
          value={customPrompt}
          onChange={(e) => setCustomPrompt(e.target.value)}
          placeholder="e.g., How did I seem today?"
          className="op-textarea"
          maxLength={500}
        />
      </div>
      <div className="op-form-field">
        <span className="op-label">Dimensions</span>
        <div className={styles.checkboxGroup}>
          {DIMENSIONS.map((d) => (
            <label key={d.value} className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={selectedDimensions.includes(d.value)}
                onChange={() => toggleDimension(d.value)}
              />
              {d.label}
            </label>
          ))}
        </div>
      </div>
      <button type="submit" className="op-btn op-btn-primary" disabled={createMutation.isPending}>
        {createMutation.isPending ? "Creating..." : "Create"}
      </button>
      {error && <p className={`op-error-msg ${styles.errorMsg}`}>{error}</p>}
    </form>
  );
}

function PollCard({ poll }: { poll: Poll }) {
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState(false);
  const [inviteUrl, setInviteUrl] = useState<string | null>(null);

  const inviteMutation = useMutation({
    mutationFn: () => observerPollsApi.invite(poll.id),
    onSuccess: (data) => {
      setInviteUrl(data.invite_url);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => observerPollsApi.delete(poll.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["observer-polls"] });
    },
  });

  const responsesQuery = useQuery({
    queryKey: ["observer-polls", poll.id, "responses"],
    queryFn: () => observerPollsApi.getResponses(poll.id),
    enabled: expanded,
  });

  const handleDelete = () => {
    if (window.confirm(`Delete poll "${poll.name}"? This cannot be undone.`)) {
      deleteMutation.mutate();
    }
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <div className={`op-card ${styles.pollCard}`}>
      <button
        type="button"
        className={styles.pollCardHeader}
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <div className={styles.pollCardInfo}>
          <h3>{poll.name}</h3>
          <div className={styles.pills}>
            {poll.dimensions.map((d) => (
              <span key={d} className="op-pill">
                {d}
              </span>
            ))}
          </div>
          <div className={styles.pollCardMeta}>
            {poll.members.length} member{poll.members.length !== 1 ? "s" : ""} &middot; Created{" "}
            {new Date(poll.created_at).toLocaleDateString()}
          </div>
        </div>
      </button>

      {expanded && (
        <div className={styles.pollCardDetails}>
          {/* Members */}
          <div className={styles.memberList}>
            <strong>Members</strong>
            {poll.members.length === 0 && (
              <p className={styles.emptyText}>No members yet. Generate an invite link.</p>
            )}
            {poll.members.map((m) => (
              <div key={m.id} className={styles.memberItem}>
                <span>{m.observer_email}</span>
                <span
                  className={`op-badge ${m.accepted_at ? "op-badge-success" : "op-badge-error"}`}
                >
                  {m.accepted_at ? "accepted" : "pending"}
                </span>
              </div>
            ))}
          </div>

          {/* Invite */}
          <button
            type="button"
            className="op-btn op-btn-secondary op-btn-sm"
            onClick={() => inviteMutation.mutate()}
            disabled={inviteMutation.isPending}
          >
            {inviteMutation.isPending ? "Generating..." : "Generate Invite Link"}
          </button>
          {inviteUrl && (
            <div className={styles.inviteLinkBox}>
              <span className={styles.inviteLinkLabel}>
                Invite link (share with your observer):
              </span>
              <div className={styles.inviteLinkRow}>
                <input type="text" readOnly value={inviteUrl} className={styles.inviteLinkInput} />
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  onClick={() => copyToClipboard(inviteUrl)}
                >
                  Copy
                </button>
              </div>
            </div>
          )}

          {/* Responses */}
          <div className={styles.responsesTable}>
            <strong>Responses</strong>
            {responsesQuery.isLoading && <p>Loading responses...</p>}
            {responsesQuery.isError && <p className="op-error-msg">Error loading responses.</p>}
            {responsesQuery.data && responsesQuery.data.responses.length === 0 && (
              <p className={styles.emptyText}>No responses yet.</p>
            )}
            {responsesQuery.data && responsesQuery.data.responses.length > 0 && (
              <ResponsesTable
                responses={responsesQuery.data.responses}
                dimensions={poll.dimensions}
              />
            )}
          </div>

          {/* Delete */}
          <div className={styles.deleteSection}>
            <button
              type="button"
              className="op-btn op-btn-danger op-btn-sm"
              onClick={handleDelete}
              disabled={deleteMutation.isPending}
            >
              {deleteMutation.isPending ? "Deleting..." : "Delete Poll"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function ResponsesTable({
  responses,
  dimensions,
}: {
  responses: OwnerResponseView[];
  dimensions: string[];
}) {
  return (
    <table className="op-table">
      <thead>
        <tr>
          <th>Date</th>
          <th>Observer</th>
          {dimensions.map((d) => (
            <th key={d}>{d}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {responses.map((r) => (
          <tr key={r.id}>
            <td>{r.date}</td>
            <td>{r.observer_email}</td>
            {dimensions.map((d) => (
              <td key={d}>{r.scores[d] ?? "-"}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function ObserverResponseForm({ poll }: { poll: ObserverPollView }) {
  const queryClient = useQueryClient();
  const [date, setDate] = useState(todayDate);
  const [scores, setScores] = useState<Record<string, string>>(() =>
    Object.fromEntries(poll.dimensions.map((d) => [d, "5"])),
  );
  const [success, setSuccess] = useState(false);

  const mutation = useMutation({
    mutationFn: () =>
      observerPollsApi.respond(poll.id, {
        date,
        scores: Object.fromEntries(Object.entries(scores).map(([k, v]) => [k, parseInt(v, 10)])),
      }),
    onSuccess: () => {
      setSuccess(true);
      queryClient.invalidateQueries({
        queryKey: ["observer-polls-mine"],
      });
      queryClient.invalidateQueries({
        queryKey: ["observer-polls", poll.id, "my-responses"],
      });
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setSuccess(false);
    mutation.mutate();
  };

  const prompt = poll.custom_prompt || `How would you rate ${poll.owner_display} today?`;

  return (
    <form
      onSubmit={handleSubmit}
      className={styles.responseForm}
      aria-label={`Response form for ${poll.name}`}
    >
      <h4>{prompt}</h4>
      <div className="op-form-field">
        <label htmlFor={`response-date-${poll.id}`} className="op-label">
          Date
        </label>
        <input
          id={`response-date-${poll.id}`}
          type="date"
          value={date}
          onChange={(e) => setDate(e.target.value)}
          required
          className="op-input"
          style={{ maxWidth: "12rem" }}
        />
      </div>
      {poll.dimensions.map((d) => (
        <div key={d} className={styles.sliderField}>
          <div className={styles.sliderLabel}>
            <span className={styles.sliderLabelText}>{d}</span>
            <span className={styles.sliderValue}>{scores[d]}/10</span>
          </div>
          <input
            type="range"
            min="1"
            max="10"
            value={scores[d]}
            onChange={(e) => setScores((prev) => ({ ...prev, [d]: e.target.value }))}
            className="op-slider"
            aria-label={d}
          />
        </div>
      ))}
      <button type="submit" className="op-btn op-btn-primary" disabled={mutation.isPending}>
        {mutation.isPending ? "Submitting..." : "Submit"}
      </button>
      {mutation.isError && (
        <p className={`op-error-msg ${styles.errorMsg}`}>Error: {mutation.error.message}</p>
      )}
      {success && <p className={styles.successMsg}>Response saved!</p>}
    </form>
  );
}

function ObserverCard({ poll }: { poll: ObserverPollView }) {
  const [showForm, setShowForm] = useState(false);
  const [showResponses, setShowResponses] = useState(false);

  const responsesQuery = useQuery({
    queryKey: ["observer-polls", poll.id, "my-responses"],
    queryFn: () => observerPollsApi.myResponses(poll.id),
    enabled: showResponses,
  });

  const exportMutation = useMutation({
    mutationFn: () => observerPollsApi.exportResponses(),
    onSuccess: (data) => {
      const blob = new Blob([JSON.stringify(data, null, 2)], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "observer-responses.json";
      a.click();
      URL.revokeObjectURL(url);
    },
  });

  return (
    <div className={`op-card ${styles.observerCard}`}>
      <div className={styles.observerCardHeader}>
        <h3>{poll.name}</h3>
        <div className={styles.observerCardMeta}>by {poll.owner_display}</div>
        {poll.custom_prompt && (
          <div className={styles.observerCardPrompt}>{poll.custom_prompt}</div>
        )}
        <div className={styles.pills}>
          {poll.dimensions.map((d) => (
            <span key={d} className="op-pill">
              {d}
            </span>
          ))}
        </div>
      </div>
      <div className={styles.observerActions}>
        <button
          type="button"
          className="op-btn op-btn-primary op-btn-sm"
          onClick={() => setShowForm(!showForm)}
        >
          {showForm ? "Hide Form" : "Respond"}
        </button>
        <button
          type="button"
          className="op-btn op-btn-ghost op-btn-sm"
          onClick={() => setShowResponses(!showResponses)}
        >
          {showResponses ? "Hide Responses" : "View My Responses"}
        </button>
        <button
          type="button"
          className="op-btn op-btn-ghost op-btn-sm"
          onClick={() => exportMutation.mutate()}
          disabled={exportMutation.isPending}
        >
          {exportMutation.isPending ? "Exporting..." : "Export"}
        </button>
      </div>
      {showForm && <ObserverResponseForm poll={poll} />}
      {showResponses && (
        <div className={styles.responsesTable}>
          {responsesQuery.isLoading && <p>Loading responses...</p>}
          {responsesQuery.isError && <p className="op-error-msg">Error loading responses.</p>}
          {responsesQuery.data && responsesQuery.data.responses.length === 0 && (
            <p className={styles.emptyText}>No responses yet.</p>
          )}
          {responsesQuery.data && responsesQuery.data.responses.length > 0 && (
            <table className="op-table">
              <thead>
                <tr>
                  <th>Date</th>
                  {poll.dimensions.map((d) => (
                    <th key={d}>{d}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {responsesQuery.data.responses.map((r) => (
                  <tr key={r.id}>
                    <td>{r.date}</td>
                    {poll.dimensions.map((d) => (
                      <td key={d}>{r.scores[d] ?? "-"}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </div>
  );
}

export default function ObserverPolls() {
  const [activeTab, setActiveTab] = useState<"my-polls" | "observe">("my-polls");
  const [showCreateForm, setShowCreateForm] = useState(false);

  const pollsQuery = useQuery({
    queryKey: ["observer-polls"],
    queryFn: () => observerPollsApi.list(),
    enabled: activeTab === "my-polls",
  });

  const myPollsQuery = useQuery({
    queryKey: ["observer-polls-mine"],
    queryFn: () => observerPollsApi.myPolls(),
    enabled: activeTab === "observe",
  });

  return (
    <main className={`op-page ${styles.page}`}>
      <h1>Observer Polls</h1>

      <div className="op-tab-bar">
        <button
          type="button"
          className={`op-tab${activeTab === "my-polls" ? " active" : ""}`}
          onClick={() => setActiveTab("my-polls")}
        >
          My Polls
        </button>
        <button
          type="button"
          className={`op-tab${activeTab === "observe" ? " active" : ""}`}
          onClick={() => setActiveTab("observe")}
        >
          Polls I Observe
        </button>
      </div>

      <div className={styles.tabContent}>
        {activeTab === "my-polls" && (
          <>
            <button
              type="button"
              className="op-btn op-btn-primary"
              onClick={() => setShowCreateForm(!showCreateForm)}
              style={{ marginBottom: "1rem" }}
            >
              {showCreateForm ? "Cancel" : "Create Poll"}
            </button>

            {showCreateForm && <CreatePollForm onCreated={() => setShowCreateForm(false)} />}

            {pollsQuery.isLoading && <p>Loading...</p>}
            {pollsQuery.isError && <p className="op-error-msg">Error loading polls.</p>}
            {pollsQuery.data && pollsQuery.data.length === 0 && (
              <p className={styles.emptyText}>No polls yet. Create one to get started.</p>
            )}
            {pollsQuery.data?.map((poll) => (
              <PollCard key={poll.id} poll={poll} />
            ))}
          </>
        )}

        {activeTab === "observe" && (
          <>
            {myPollsQuery.isLoading && <p>Loading...</p>}
            {myPollsQuery.isError && <p className="op-error-msg">Error loading polls.</p>}
            {myPollsQuery.data && myPollsQuery.data.length === 0 && (
              <p className={styles.emptyText}>
                You are not observing any polls. Accept an invite link to get started.
              </p>
            )}
            {myPollsQuery.data?.map((poll) => (
              <ObserverCard key={poll.id} poll={poll} />
            ))}
          </>
        )}
      </div>
    </main>
  );
}
