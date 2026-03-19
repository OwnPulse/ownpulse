# ADR-0001: Rust and Axum for the Backend API

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse needs a backend API that will handle continuous health data ingest from multiple sources (HealthKit, Garmin, Oura, Dexcom), background sync jobs, streaming data exports, correlation computation, and eventually aggregate queries across cooperative members. The API must be self-hostable on modest hardware — a single VPS with 4GB RAM serving one or a handful of users is the target deployment.

Key forces:
- **Performance matters for the self-hosting case.** A resource-hungry runtime (JVM, .NET CLR) is a real barrier when running on a $48/mo droplet alongside Postgres and nginx.
- **Correctness matters for health data.** Data loss or silent corruption in a health tracking platform erodes trust permanently. Strong type safety reduces the surface area for bugs.
- **The correlation engine is compute-intensive.** Pearson correlation, lag analysis, and rolling averages over years of 5-minute glucose readings need to be fast.
- **Export must stream.** Full data exports for users with years of data cannot be buffered in memory. Async streaming is a first-class requirement.
- **SQLx compile-time query checking** is a specific capability we want — the ability to verify SQL queries against the actual schema at compile time, catching mistakes before they reach production.
- **Long-term maintainability.** The codebase will be worked on by AI agents and human contributors who may not know each other. Strict type safety and explicit error handling reduce the amount of implicit knowledge required.

---

## Decision

Use **Rust** (stable channel, edition 2021) with **Axum 0.7** as the HTTP framework, **SQLx 0.7** for database access, and **Tokio** as the async runtime.

---

## Alternatives Considered

### Go + standard library or Chi/Echo
Go was the strongest alternative. It compiles fast, has excellent tooling, strong concurrency primitives, and is widely known. The standard library is rich enough that a Go API needs fewer dependencies than Rust.

We didn't choose Go because:
- No equivalent to SQLx's compile-time query verification. `database/sql` catches errors at runtime.
- The type system is less expressive — no sum types, no exhaustive pattern matching. Handling the many variants of health data records cleanly requires more boilerplate.
- Memory usage is higher than Rust (GC pauses, larger runtime overhead), which matters for the self-hosting target.
- Error handling via `(value, error)` return pairs is more verbose and easier to mishandle than Rust's `Result<T, E>`.

Go remains a good choice and we would not have made a serious mistake by choosing it.

### Python (FastAPI or Django)
FastAPI is pleasant to work with and has a large ecosystem. Rejected because:
- Runtime performance is an order of magnitude below Rust or Go.
- Memory usage makes it poorly suited for small VPS deployments.
- Type hints are checked by external tools (mypy), not the compiler — much weaker guarantee.
- Async story in Python is still fragmented compared to Tokio.

### Node.js (Fastify or Hapi)
Rejected for similar reasons as Python. Strong ecosystem but poor performance characteristics and runtime type safety.

### Java (Spring Boot) or Kotlin (Ktor)
Mature ecosystems and genuinely good type systems. Rejected because:
- JVM startup time and memory footprint are incompatible with self-hosting on a small droplet.
- Operational complexity (JVM tuning, GC configuration) adds overhead.

---

## Consequences

**Positive:**
- Compile-time SQL query verification via SQLx catches schema mismatches before deployment.
- Memory-efficient — the API binary uses significantly less RAM than a JVM or interpreted language runtime, leaving more headroom for Postgres on the same droplet.
- Fearless concurrency — the borrow checker prevents data races in the background sync jobs.
- `Result<T, E>` forces explicit error handling everywhere; errors cannot be silently swallowed.
- Fast enough that the correlation engine can run synchronously on request without a separate job queue in Phase 1.
- Single self-contained binary — no runtime to install on the target host, just copy the binary.

**Negative / tradeoffs:**
- Slower compile times than Go or interpreted languages. Cold builds take 60-90 seconds; incremental builds are faster but still slower than Go.
- Steeper learning curve for contributors unfamiliar with Rust's ownership model. AI agents (Claude Code, Cursor) handle Rust well, but human contributors may find it harder to contribute than a Go or Python codebase.
- Smaller talent pool than Go, Python, or Node if the project eventually needs to hire.
- The async ecosystem (Tokio, Axum) is mature but younger than Go's or Python's. Some crates are less polished than their Go equivalents.

**Risks:**
- Compile time increases as the codebase grows. Mitigate with workspace caching in CI and careful module splitting.
- Borrow checker fights during complex async code patterns. Mitigate with clear ownership boundaries between modules.

---

## References

- Axum 0.7 documentation: https://docs.rs/axum
- SQLx: https://github.com/launchbadge/sqlx
- Tokio: https://tokio.rs
- Comparison that influenced this decision: https://www.techempower.com/benchmarks/ (framework benchmarks)
