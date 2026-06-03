// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import "./styles/fonts.css";
import "./styles/reset.css";
import "./styles/variables.css";
import "./styles/global.css";
import "./styles/components.css";
import AdminRoute from "./components/AdminRoute";
import Layout from "./components/Layout";
import ProtectedRoute from "./components/ProtectedRoute";
import ForgotPassword from "./pages/ForgotPassword";
import InviteLanding from "./pages/InviteLanding";
import Login from "./pages/Login";
import ObserverAccept from "./pages/ObserverAccept";
import Register from "./pages/Register";
import ResetPassword from "./pages/ResetPassword";
import ShareAccept from "./pages/ShareAccept";
import SharedProtocol from "./pages/SharedProtocol";
import Welcome from "./pages/Welcome";

const Admin = lazy(() => import("./pages/Admin"));
const Analyze = lazy(() => import("./pages/Analyze"));
const Dashboard = lazy(() => import("./pages/Dashboard"));
const DataEntry = lazy(() => import("./pages/DataEntry"));
const Explore = lazy(() => import("./pages/Explore"));
const Friends = lazy(() => import("./pages/Friends"));
const FriendView = lazy(() => import("./pages/FriendView"));
const Genetics = lazy(() => import("./pages/Genetics"));
const ObserverPolls = lazy(() => import("./pages/ObserverPolls"));
const ProtocolBuilder = lazy(() => import("./pages/ProtocolBuilder"));
const Protocols = lazy(() => import("./pages/Protocols"));
const ProtocolView = lazy(() => import("./pages/ProtocolView"));
const Settings = lazy(() => import("./pages/Settings"));
const Sources = lazy(() => import("./pages/Sources"));

const queryClient = new QueryClient();

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("Root element not found");
ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Suspense fallback={<main className="op-page">Loading...</main>}>
          <Routes>
            <Route path="/login" element={<Login />} />
            <Route path="/register" element={<Register />} />
            <Route path="/invite/:code" element={<InviteLanding />} />
            <Route path="/forgot-password" element={<ForgotPassword />} />
            <Route path="/reset-password" element={<ResetPassword />} />
            <Route path="/protocols/shared/:token" element={<SharedProtocol />} />
            <Route element={<ProtectedRoute />}>
              <Route element={<Layout />}>
                <Route path="/" element={<Dashboard />} />
                <Route path="/explore" element={<Explore />} />
                <Route path="/explore/:chartId" element={<Explore />} />
                <Route path="/analyze" element={<Analyze />} />
                <Route path="/genetics" element={<Genetics />} />
                <Route path="/entry" element={<DataEntry />} />
                <Route path="/sources" element={<Sources />} />
                <Route path="/settings" element={<Settings />} />
                <Route path="/friends" element={<Friends />} />
                <Route path="/friends/:friendId" element={<FriendView />} />
                <Route path="/observer-polls" element={<ObserverPolls />} />
                <Route path="/protocols" element={<Protocols />} />
                <Route path="/protocols/new" element={<ProtocolBuilder />} />
                <Route path="/protocols/:id" element={<ProtocolView />} />
              </Route>
              <Route path="/welcome" element={<Welcome />} />
              <Route path="/share/accept" element={<ShareAccept />} />
              <Route path="/observe/accept" element={<ObserverAccept />} />
              <Route element={<AdminRoute />}>
                <Route element={<Layout />}>
                  <Route path="/admin" element={<Admin />} />
                </Route>
              </Route>
            </Route>
          </Routes>
        </Suspense>
      </BrowserRouter>
    </QueryClientProvider>
  </React.StrictMode>,
);
