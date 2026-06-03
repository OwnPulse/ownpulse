// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WebKit
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "oauth-webview")

/// Outcome of an in-app OAuth flow run inside ``OAuthWebView``.
enum OAuthWebResult: Equatable, Sendable {
    /// The backend completed the flow and redirected to the success URL
    /// (`/settings?connected=<provider>`).
    case connected(provider: String)
    /// The user dismissed the sheet before completing the flow.
    case cancelled
    /// The flow failed. The associated message is safe to display — it never
    /// contains tokens or health data.
    case failed(message: String)
}

/// SwiftUI wrapper around `WKWebView` that runs a provider OAuth flow against
/// the OwnPulse backend's real `/auth/<provider>/login` and
/// `/auth/<provider>/callback` routes.
///
/// The backend login/callback routes are JWT-protected (`AuthUser` extractor)
/// and rely on short-lived httpOnly cookies to carry the OAuth request-token
/// secret / CSRF state between the two hops. To make that work inside an in-app
/// web view we:
///
/// 1. Inject `Authorization: Bearer <jwt>` on every navigation request that
///    targets our own API origin (the provider's own pages never see it).
/// 2. Use the default (persistent) `WKWebsiteDataStore` so the `Set-Cookie`
///    from the login redirect is replayed on the callback navigation.
/// 3. Detect the terminal redirect to `/settings?connected=<provider>` and
///    report success.
///
/// No token is ever logged or persisted by this view — the JWT is read from the
/// Keychain only for the duration of the flow.
struct OAuthWebView: UIViewRepresentable {
    /// Provider key as used by the backend route segment, e.g. `"garmin"`.
    let provider: String
    /// Absolute URL of the backend login endpoint to start the flow.
    let startURL: URL
    /// API origin (scheme + host + port). Requests to this origin get the JWT.
    let apiOrigin: URL
    /// Bearer token to attach to same-origin API requests. `nil` disables
    /// injection (the flow will then fail auth — surfaced as `.failed`).
    let bearerToken: String?
    /// Called once when the flow reaches a terminal state.
    let onResult: (OAuthWebResult) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(
            provider: provider,
            apiOrigin: apiOrigin,
            bearerToken: bearerToken,
            onResult: onResult
        )
    }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        // Default (persistent) data store so the backend's httpOnly OAuth
        // cookies survive between the login redirect and the callback.
        config.websiteDataStore = .default()
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.accessibilityIdentifier = "oauthWebView-\(provider)"

        var request = URLRequest(url: startURL)
        context.coordinator.applyAuthHeaderIfSameOrigin(to: &request)
        webView.load(request)
        return webView
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}

    // MARK: - Coordinator

    final class Coordinator: NSObject, WKNavigationDelegate {
        private let provider: String
        private let apiOrigin: URL
        private let bearerToken: String?
        private let onResult: (OAuthWebResult) -> Void
        /// Guards against firing `onResult` more than once.
        private var didFinish = false

        init(
            provider: String,
            apiOrigin: URL,
            bearerToken: String?,
            onResult: @escaping (OAuthWebResult) -> Void
        ) {
            self.provider = provider
            self.apiOrigin = apiOrigin
            self.bearerToken = bearerToken
            self.onResult = onResult
        }

        /// Adds the Bearer header to `request` only when it targets our API
        /// origin. Cross-origin requests (the provider's auth pages) are left
        /// untouched so the JWT never leaves our backend.
        func applyAuthHeaderIfSameOrigin(to request: inout URLRequest) {
            guard let token = bearerToken,
                  let url = request.url,
                  isSameOrigin(url) else { return }
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        func isSameOrigin(_ url: URL) -> Bool {
            url.scheme == apiOrigin.scheme
                && url.host == apiOrigin.host
                && url.port == apiOrigin.port
        }

        /// `true` once the URL is the backend's terminal success redirect
        /// `<origin>/settings?connected=<provider>`.
        func isSuccessRedirect(_ url: URL) -> Bool {
            guard isSameOrigin(url), url.path == "/settings" else { return false }
            let connected = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                .queryItems?
                .first(where: { $0.name == "connected" })?
                .value
            return connected == provider
        }

        // MARK: WKNavigationDelegate

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            guard let url = navigationAction.request.url else {
                decisionHandler(.allow)
                return
            }

            // Terminal success: backend finished and bounced us to settings.
            if isSuccessRedirect(url) {
                decisionHandler(.cancel)
                finish(.connected(provider: provider))
                return
            }

            // Same-origin API request that is missing the Bearer header
            // (e.g. the callback navigation triggered by the provider's
            // redirect). Re-issue it with the header attached.
            if isSameOrigin(url),
               bearerToken != nil,
               navigationAction.request.value(forHTTPHeaderField: "Authorization") == nil {
                decisionHandler(.cancel)
                var authed = navigationAction.request
                applyAuthHeaderIfSameOrigin(to: &authed)
                webView.load(authed)
                return
            }

            decisionHandler(.allow)
        }

        func webView(
            _ webView: WKWebView,
            didFailProvisionalNavigation navigation: WKNavigation!,
            withError error: Error
        ) {
            // A cancelled navigation (our own `.cancel` above) reports
            // NSURLErrorCancelled — that is expected and not a failure.
            let nsError = error as NSError
            if nsError.domain == NSURLErrorDomain && nsError.code == NSURLErrorCancelled {
                return
            }
            logger.error("OAuth navigation failed: \(nsError.code, privacy: .public)")
            finish(.failed(message: "Couldn't reach the connection page. Check your network and try again."))
        }

        private func finish(_ result: OAuthWebResult) {
            guard !didFinish else { return }
            didFinish = true
            onResult(result)
        }
    }
}
