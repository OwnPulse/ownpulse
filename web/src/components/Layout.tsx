// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { NavLink, Outlet } from "react-router-dom";
import { logout } from "../api/auth";
import { useSSE } from "../hooks/useSSE";
import { useTheme } from "../hooks/useTheme";
import { useAuthStore } from "../store/auth";
import styles from "./Layout.module.css";

const THEME_LABELS = {
  light: "Theme: Light",
  dark: "Theme: Dark",
  system: "Theme: System",
} as const;

const THEME_CYCLE = {
  light: "dark",
  dark: "system",
  system: "light",
} as const;

export default function Layout() {
  const role = useAuthStore((s) => s.role);
  const { theme, setTheme } = useTheme();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  useSSE();

  const handleLogout = async () => {
    await logout();
    window.location.href = "/login";
  };

  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    `${styles.navLink}${isActive ? ` ${styles.navLinkActive}` : ""}`;

  const closeSidebar = () => setSidebarOpen(false);

  return (
    <div className={styles.layout}>
      <button
        type="button"
        className={styles.menuBtn}
        onClick={() => setSidebarOpen(!sidebarOpen)}
        aria-label="Toggle menu"
      >
        &#9776;
      </button>

      {/* biome-ignore lint: overlay click is intentional for closing sidebar */}
      <div
        className={`${styles.overlay}${sidebarOpen ? ` ${styles.overlayVisible}` : ""}`}
        onClick={closeSidebar}
      />

      <aside className={`${styles.sidebar}${sidebarOpen ? ` ${styles.sidebarOpen}` : ""}`}>
        <NavLink to="/" className={styles.wordmark} onClick={closeSidebar}>
          <span className={styles.wordmarkOwn}>Own</span>
          <span className={styles.wordmarkPulse}>Pulse</span>
        </NavLink>

        <nav className={styles.nav}>
          <NavLink to="/" end className={navLinkClass} onClick={closeSidebar}>
            Dashboard
          </NavLink>
          <NavLink to="/explore" className={navLinkClass} onClick={closeSidebar}>
            Explore
          </NavLink>
          <NavLink to="/analyze" className={navLinkClass} onClick={closeSidebar}>
            Analyze
          </NavLink>
          <NavLink to="/entry" className={navLinkClass} onClick={closeSidebar}>
            Data Entry
          </NavLink>
          <NavLink to="/sources" className={navLinkClass} onClick={closeSidebar}>
            Sources
          </NavLink>
          <NavLink to="/friends" className={navLinkClass} onClick={closeSidebar}>
            Friends
          </NavLink>
          <NavLink to="/observer-polls" className={navLinkClass} onClick={closeSidebar}>
            Observer Polls
          </NavLink>
          <NavLink to="/settings" className={navLinkClass} onClick={closeSidebar}>
            Settings
          </NavLink>
          {role === "admin" && (
            <NavLink to="/admin" className={navLinkClass} onClick={closeSidebar}>
              Admin
            </NavLink>
          )}
        </nav>

        <div className={styles.bottom}>
          <button
            type="button"
            className={styles.themeToggle}
            onClick={() => setTheme(THEME_CYCLE[theme])}
          >
            {THEME_LABELS[theme]}
          </button>
          <button type="button" className={styles.logoutBtn} onClick={handleLogout}>
            Logout
          </button>
        </div>
      </aside>

      <main className={styles.main}>
        <Outlet />
      </main>
    </div>
  );
}
