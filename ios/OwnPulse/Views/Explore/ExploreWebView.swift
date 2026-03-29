// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WebKit

struct ExploreWebView: View {
    @Environment(AppDependencies.self) private var dependencies

    var body: some View {
        WebViewContainer(keychainService: dependencies.keychainService)
            .navigationTitle("Explore")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        if let url = URL(string: "\(AppConfig.webDashboardURL)/explore") {
                            UIApplication.shared.open(url)
                        }
                    } label: {
                        Image(systemName: "safari")
                    }
                    .accessibilityIdentifier("openInSafariButton")
                }
            }
    }
}

struct WebViewContainer: UIViewRepresentable {
    let keychainService: KeychainServiceProtocol

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.accessibilityIdentifier = "exploreWebView"

        loadExplore(in: webView)
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        // No-op: initial load handles auth
    }

    private func loadExplore(in webView: WKWebView) {
        guard let urlString = URL(string: "\(AppConfig.webDashboardURL)/explore") else { return }

        // Inject JWT as a cookie so the web app can authenticate
        if let tokenData = try? keychainService.load(key: AuthService.accessTokenKey),
           let token = String(data: tokenData, encoding: .utf8) {
            let cookie = HTTPCookie(properties: [
                .name: "access_token",
                .value: token,
                .domain: urlString.host ?? "",
                .path: "/",
                .secure: "TRUE",
            ])

            if let cookie {
                webView.configuration.websiteDataStore.httpCookieStore.setCookie(cookie) {
                    webView.load(URLRequest(url: urlString))
                }
            } else {
                webView.load(URLRequest(url: urlString))
            }
        } else {
            webView.load(URLRequest(url: urlString))
        }
    }
}
