// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Testing
@testable import OwnPulse

@Suite("PKCEHelper")
struct PKCEHelperTests {
    @Test("code verifier is within RFC 7636 length bounds")
    func codeVerifierIsCorrectLength() {
        let verifier = PKCEHelper.generateCodeVerifier()
        #expect(verifier.count >= 43)
        #expect(verifier.count <= 128)
    }

    @Test("code verifier uses only base64url characters")
    func codeVerifierIsBase64URL() {
        let verifier = PKCEHelper.generateCodeVerifier()
        #expect(!verifier.contains("+"))
        #expect(!verifier.contains("/"))
        #expect(!verifier.contains("="))
    }

    @Test("code verifier is unique across calls")
    func codeVerifierIsUnique() {
        let v1 = PKCEHelper.generateCodeVerifier()
        let v2 = PKCEHelper.generateCodeVerifier()
        #expect(v1 != v2)
    }

    @Test("code challenge is deterministic for the same verifier")
    func codeChallengeIsDeterministic() {
        let verifier = "test-verifier-string"
        let challenge1 = PKCEHelper.codeChallenge(from: verifier)
        let challenge2 = PKCEHelper.codeChallenge(from: verifier)
        #expect(challenge1 == challenge2)
    }

    @Test("code challenge uses only base64url characters")
    func codeChallengeIsBase64URL() {
        let verifier = "test-verifier"
        let challenge = PKCEHelper.codeChallenge(from: verifier)
        #expect(!challenge.contains("+"))
        #expect(!challenge.contains("/"))
        #expect(!challenge.contains("="))
    }

    @Test("code challenge is not equal to the verifier")
    func codeChallengeIsTransformed() {
        let verifier = PKCEHelper.generateCodeVerifier()
        let challenge = PKCEHelper.codeChallenge(from: verifier)
        #expect(challenge != verifier)
    }

    @Test("code challenge matches known SHA256-S256 vector")
    func codeChallengeKnownVector() {
        // RFC 7636 §B test vector
        // verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        // expected  = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        let challenge = PKCEHelper.codeChallenge(from: verifier)
        #expect(challenge == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
    }
}
