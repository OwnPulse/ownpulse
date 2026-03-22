// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "appleAuth")

/// Wraps ASAuthorizationController delegate callbacks in a CheckedContinuation.
final class AppleAuthDelegate: NSObject, ASAuthorizationControllerDelegate, Sendable {
    private let continuation: CheckedContinuation<ASAuthorization, Error>

    init(continuation: CheckedContinuation<ASAuthorization, Error>) {
        self.continuation = continuation
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        continuation.resume(returning: authorization)
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        logger.error("Apple Sign-In error: \(error.localizedDescription, privacy: .public)")
        continuation.resume(throwing: error)
    }
}

/// Provides a window anchor for ASAuthorizationController.
final class ApplePresentationContext: NSObject,
    ASAuthorizationControllerPresentationContextProviding
{
    @MainActor
    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
    }
}

/// Key for storing the Apple auth delegate as an associated object.
nonisolated(unsafe) private var appleAuthDelegateKey: UInt8 = 0
/// Key for storing the ASAuthorizationController as an associated object on the delegate.
nonisolated(unsafe) private var appleAuthControllerKey: UInt8 = 0

/// Shared helper that runs ASAuthorizationController and returns the Apple ID credential.
@MainActor
enum AppleAuthHelper {
    /// Performs the Apple Sign-In flow and returns the credential.
    static func performAppleAuth(
        scopes: [ASAuthorization.Scope] = [.email]
    ) async throws -> ASAuthorizationAppleIDCredential {
        let provider = ASAuthorizationAppleIDProvider()
        let request = provider.createRequest()
        request.requestedScopes = scopes

        let authorization = try await performRequest(request)

        guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential else {
            throw AuthError.invalidCallback
        }
        return credential
    }

    /// Runs an ASAuthorizationAppleIDRequest via ASAuthorizationController and bridges the
    /// delegate callbacks into async/await.
    static func performRequest(_ request: ASAuthorizationAppleIDRequest) async throws -> ASAuthorization {
        let presentationContext = ApplePresentationContext()

        return try await withCheckedThrowingContinuation { continuation in
            let controller = ASAuthorizationController(authorizationRequests: [request])
            let delegate = AppleAuthDelegate(continuation: continuation)

            // Keep delegate alive for the duration of the auth flow.
            objc_setAssociatedObject(
                controller,
                &appleAuthDelegateKey,
                delegate,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )
            // Keep controller alive so it isn't deallocated before the delegate fires.
            objc_setAssociatedObject(
                delegate,
                &appleAuthControllerKey,
                controller,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )
            // Keep presentation context alive.
            objc_setAssociatedObject(
                controller,
                &applePresentationContextKey,
                presentationContext,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )

            controller.delegate = delegate
            controller.presentationContextProvider = presentationContext
            controller.performRequests()
        }
    }
}

nonisolated(unsafe) private var applePresentationContextKey: UInt8 = 0
