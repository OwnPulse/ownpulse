# ADR-0005: iOS as Data Pump First, Dashboard Later

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse has two client surfaces: a native iOS app and a web frontend. Both need to exist, but the question is what each does and in what order.

The iOS app has a unique capability no other surface has: direct HealthKit access. It can read and write health data from the Apple Health ecosystem without any user export step. This is the highest-value thing the iOS app can do.

The web frontend, running in a browser on a large screen, is a better surface for data exploration — timelines with multiple overlaid metrics, correlation scatter plots, lab result trend lines. These benefit from screen real estate, mouse interaction, and the flexibility of a full browser rendering engine.

Building a high-quality dashboard in both iOS and web simultaneously is significantly more work than building one well and the other partially. The question is which to prioritize.

---

## Decision

**Phase 1-2:** iOS app is a data pump and sync manager only. It handles HealthKit read/write, background sync, intervention logging, daily check-ins, and source configuration. Visualization is deferred to the web frontend with a prominent "Open Dashboard" link in the iOS app.

**Phase 3b:** iOS gets a native dashboard using Swift Charts. MacroFactor-style: a hero metric with trend line, a sparkline row of secondary metrics, a today card, and a weekly summary. The dashboard consumes the same API endpoints already built for the web — no new backend work.

---

## Alternatives Considered

### Build full dashboards in both iOS and web simultaneously from Phase 1

Would provide the best user experience from the start, but:
- Doubles the visualization work — every chart type needs implementing twice (unovis + Swift Charts).
- iOS chart development requires simulator testing and is slower to iterate than web.
- Delays Phase 1 delivery significantly.
- The web dashboard is a better first target because it works on any device (including iPhone via mobile browser).

Rejected in favor of sequencing.

### Web-only, no native iOS app

Some personal health platforms are web-only. The problem is HealthKit — Apple's Health app data is only accessible from a native iOS app. A web-only platform cannot read HealthKit data without an export step, which creates significant user friction for the core data collection use case.

Rejected because HealthKit native access is too valuable to forgo.

### iOS-primary, web as secondary

Build the full iOS dashboard first, add web later.

Rejected because:
- The web frontend is a better surface for the data exploration features (correlation explorer, lab trends) that create the platform's analytical value.
- Web reaches more users (Android users, desktop users) without requiring an iOS device.
- Web development iteration is faster.

---

## Consequences

**Positive:**
- Phase 1 iOS scope is small and achievable quickly — auth, sync, intervention log, check-in. No charting required.
- The web frontend gets all visualization investment in Phase 1-2, producing a polished analytical surface sooner.
- When the iOS dashboard is built in Phase 3b, it uses already-proven API endpoints — no backend changes needed.
- Swift Charts (native) keeps the iOS dependency count low — no third-party charting library.
- "Open Dashboard" link creates a natural habit of using the web frontend for exploration, which is the right long-term behavior anyway.

**Negative / tradeoffs:**
- Users who only want iOS for everything will find Phase 1 and 2 limited. They need to open a browser to see their data.
- The "Open Dashboard" UX is a seam — switching from iOS to web feels like context switching.
- Two codebases to maintain once the iOS dashboard exists.

**Risks:**
- Users churn before Phase 3b if they find the iOS-only experience too limited. Mitigate by making the web frontend work well on mobile browsers (responsive design) so the gap is smaller than it appears.
- Swift Charts has limitations compared to web charting libraries (fewer chart types, less customization). The MacroFactor-style dashboard is well within Swift Charts' capabilities; more exotic visualizations may need workarounds.

---

## References

- MacroFactor iOS app (dashboard design reference): https://macrofactorapp.com
- Swift Charts documentation: https://developer.apple.com/documentation/charts
- Apple HealthKit documentation: https://developer.apple.com/documentation/healthkit
