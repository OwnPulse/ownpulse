// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

func movingAverage(points: [DataPoint], window: Int) -> [DataPoint] {
    guard window > 0 else { return points }
    return points.enumerated().map { i, point in
        let start = max(0, i - window + 1)
        let windowSlice = points[start...i]
        let avg = windowSlice.reduce(0.0) { $0 + $1.v } / Double(windowSlice.count)
        return DataPoint(t: point.t, v: avg, n: point.n)
    }
}
