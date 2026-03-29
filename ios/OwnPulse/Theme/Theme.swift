// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

enum OPColor {
    static let terracotta = Color(red: 194 / 255, green: 101 / 255, blue: 74 / 255)
    static let teal = Color(red: 61 / 255, green: 139 / 255, blue: 139 / 255)
    static let gold = Color(red: 196 / 255, green: 154 / 255, blue: 60 / 255)
    static let sage = Color(red: 90 / 255, green: 138 / 255, blue: 90 / 255)
    static let warmBg = Color(red: 250 / 255, green: 246 / 255, blue: 241 / 255)
    static let darkBg = Color(red: 26 / 255, green: 26 / 255, blue: 26 / 255)
    static let cardDark = Color(red: 34 / 255, green: 34 / 255, blue: 34 / 255)
    static let cardLight = Color.white

    static let trendUp = Color.red
    static let trendDown = Color.green
    static let trendFlat = Color.secondary
}

struct OPCardModifier: ViewModifier {
    @Environment(\.colorScheme) private var colorScheme

    func body(content: Content) -> some View {
        content
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(colorScheme == .dark ? .ultraThinMaterial : .regularMaterial)
            )
            .shadow(color: .black.opacity(colorScheme == .dark ? 0.3 : 0.08), radius: 8, y: 4)
    }
}

extension View {
    func opCard() -> some View {
        modifier(OPCardModifier())
    }
}
