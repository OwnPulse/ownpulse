// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { Link, Outlet } from "react-router-dom";
import { logout } from "../api/auth";

const navStyle: React.CSSProperties = {
  display: "flex",
  gap: "1rem",
  alignItems: "center",
  padding: "0.75rem 1.5rem",
  borderBottom: "1px solid #ddd",
};

const linkStyle: React.CSSProperties = {
  textDecoration: "none",
  color: "#333",
};

export default function Layout() {
  const handleLogout = async () => {
    await logout();
    window.location.href = "/login";
  };

  return (
    <div>
      <nav style={navStyle}>
        <Link to="/" style={linkStyle}>
          Timeline
        </Link>
        <Link to="/entry" style={linkStyle}>
          Data Entry
        </Link>
        <Link to="/sources" style={linkStyle}>
          Sources
        </Link>
        <Link to="/settings" style={linkStyle}>
          Settings
        </Link>
        <div style={{ marginLeft: "auto" }}>
          <button onClick={handleLogout}>Logout</button>
        </div>
      </nav>
      <Outlet />
    </div>
  );
}
