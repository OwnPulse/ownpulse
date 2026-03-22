// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { adminApi, AdminUser } from "../api/admin";

const tableStyle: React.CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  fontSize: "0.875rem",
};

const thStyle: React.CSSProperties = {
  textAlign: "left",
  padding: "0.75rem",
  borderBottom: "2px solid var(--color-border)",
  color: "var(--color-text-muted)",
  fontWeight: 600,
  fontSize: "0.8125rem",
};

const tdStyle: React.CSSProperties = {
  padding: "0.75rem",
  borderBottom: "1px solid var(--color-border)",
  color: "var(--color-text)",
};

export default function Admin() {
  const queryClient = useQueryClient();
  const { data: users, isLoading, isError } = useQuery({
    queryKey: ["admin-users"],
    queryFn: adminApi.listUsers,
  });

  const roleMutation = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) =>
      adminApi.updateRole(userId, role),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });

  if (isLoading) return <main style={{ padding: "1.5rem" }}>Loading...</main>;
  if (isError) return <main style={{ padding: "1.5rem" }}>Error loading users.</main>;

  return (
    <main style={{ padding: "1.5rem", fontFamily: "var(--font-family)" }}>
      <h1 style={{ fontSize: "1.5rem", color: "var(--color-text)", marginBottom: "1.5rem" }}>Admin</h1>

      <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-md)", border: "1px solid var(--color-border)", overflow: "hidden" }}>
        <table style={tableStyle}>
          <thead>
            <tr>
              <th style={thStyle}>Username</th>
              <th style={thStyle}>Email</th>
              <th style={thStyle}>Provider</th>
              <th style={thStyle}>Role</th>
              <th style={thStyle}>Created</th>
            </tr>
          </thead>
          <tbody>
            {users?.map((u: AdminUser) => (
              <tr key={u.id}>
                <td style={tdStyle}>{u.username}</td>
                <td style={tdStyle}>{u.email || "\u2014"}</td>
                <td style={tdStyle}>{u.auth_provider}</td>
                <td style={tdStyle}>
                  <select
                    value={u.role}
                    onChange={(e) => roleMutation.mutate({ userId: u.id, role: e.target.value })}
                    style={{
                      padding: "0.25rem 0.5rem",
                      borderRadius: "var(--radius-sm)",
                      border: "1px solid var(--color-border)",
                      fontSize: "0.8125rem",
                      fontFamily: "var(--font-family)",
                    }}
                  >
                    <option value="admin">admin</option>
                    <option value="user">user</option>
                  </select>
                </td>
                <td style={tdStyle}>{new Date(u.created_at).toLocaleDateString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </main>
  );
}
