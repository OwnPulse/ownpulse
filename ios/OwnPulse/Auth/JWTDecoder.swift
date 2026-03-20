// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

enum JWTDecoder {
    struct Payload {
        let sub: String
        let exp: Date
    }

    static func decode(_ token: String) -> Payload? {
        let parts = token.split(separator: ".")
        guard parts.count == 3 else { return nil }

        var base64 = String(parts[1])
        // Pad to multiple of 4
        while base64.count % 4 != 0 {
            base64.append("=")
        }
        // Replace URL-safe characters
        base64 = base64
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")

        guard let data = Data(base64Encoded: base64),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let sub = json["sub"] as? String,
              let exp = json["exp"] as? TimeInterval else {
            return nil
        }

        return Payload(sub: sub, exp: Date(timeIntervalSince1970: exp))
    }

    static func isExpired(_ token: String, buffer: TimeInterval = 60) -> Bool {
        guard let payload = decode(token) else { return true }
        return payload.exp.timeIntervalSinceNow < buffer
    }
}
