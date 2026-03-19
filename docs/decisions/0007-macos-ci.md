# ADR-0007: Mac Mini M4 + Tart VMs + nix-darwin for iOS CI

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

iOS builds require Apple Silicon macOS. There is no legal or practical way to run macOS virtualized on Linux or x86 hardware. GitHub's hosted macOS runners exist but are widely regarded as slow, flaky, and expensive relative to the performance they provide.

For a project committed to self-hosted infrastructure and low monthly costs, the question is how to run iOS CI reliably without paying GitHub's per-minute macOS runner rates.

Requirements:
- iOS CI must run on every pull request (Swift Testing, XCUITest, Maestro E2E)
- Build times should be under 10 minutes for a full test run
- The runner fleet must be fully managed — no manual SSH, no per-machine configuration drift
- Adding a new runner should be a single command, not an afternoon of manual setup
- The macOS environment must be reproducible — pinned Xcode version, consistent tooling

---

## Decision

Use a **Mac mini M4 (base model)** running **Tart** for VM-isolated CI jobs, with **nix-darwin** managing the host configuration declaratively. **Ansible** bootstraps new machines from factory state to nix-darwin-managed in a single playbook run. The Mac mini registers as a **GitHub Actions self-hosted runner** with the label `macos-tart`.

---

## Alternatives Considered

### GitHub-hosted macOS runners

GitHub provides `macos-latest` and `macos-14` runners. No hardware to buy or maintain.

Rejected because:
- Slow: 2-3x longer build times than Apple Silicon hardware.
- Expensive: macOS minutes are billed at 10x the rate of Linux minutes on GitHub Actions.
- Flaky: GitHub macOS runners have a notably higher failure rate than Linux runners due to resource contention on shared hardware.
- Xcode version control is limited — specific minor versions may not be available.
- Conflicts with the project's self-hosted infrastructure philosophy.

### MacStadium (hosted Mac mini or Orka)

MacStadium provides dedicated Mac mini rentals and the Orka Kubernetes-for-Mac platform.

Rejected because:
- Cost: dedicated Mac mini from MacStadium runs $99-150/mo. The one-time $599 Mac mini M4 purchase pays for itself in 4-6 months vs. MacStadium.
- Orka: designed for large teams with many parallel builds. Significant operational overhead and cost for a single-developer project.
- Conflicts with the cost philosophy of minimizing monthly outlay.

### Hetzner Apple Silicon (self-hosted cloud Mac)

Hetzner recently launched Apple Silicon Mac minis as cloud servers at ~$50-60/mo.

Rejected for now because:
- Still more expensive than owned hardware over any multi-month period.
- Less mature than Hetzner's x86 offering — reliability track record is shorter.
- Owning the hardware gives more flexibility (Tart VM management, direct Tailscale access).

Worth reconsidering if hardware management becomes burdensome or multiple geographic regions need CI runners.

### Buildkite Mac agents

Buildkite is a CI platform with strong macOS support. Self-hosted agents run on Mac hardware.

Rejected because:
- Requires switching from GitHub Actions to Buildkite, losing native PR integration.
- Adds a paid third-party service dependency.
- The ARC + self-hosted runner model already solves the problem for Linux; extending it to macOS via Tart keeps the CI model consistent.

### Bare runner without VM isolation

Register the Mac mini as a GitHub Actions runner without Tart. Each job runs directly on the host.

Rejected because:
- Jobs share host state — packages installed by one job can affect the next.
- Difficult to reproduce CI failures locally if the host has accumulated state.
- Malicious or buggy test code could affect the host permanently.
- Tart VMs add minimal overhead (< 30 seconds per job) and provide full isolation.

### nix-darwin alternatives (Ansible-only, Homebrew-only)

Managing the Mac mini with only Ansible playbooks or only Homebrew without nix-darwin.

Considered as a simpler alternative. Rejected because:
- Ansible-only management is procedural — it runs steps, but doesn't guarantee the end state. If a step fails halfway, the host is in an indeterminate state.
- nix-darwin is declarative — the configuration describes what the host should be, and `darwin-rebuild switch` converges to that state atomically.
- Xcode version management via nix-darwin activation scripts is more reliable than Ansible tasks.
- Adding a second Mac mini with nix-darwin is: copy a directory, change the hostname. With Ansible-only it requires careful idempotency engineering.

---

## Consequences

**Positive:**
- Apple Silicon M4 is the fastest hardware available for iOS builds. Full test suite runs in 3-5 minutes.
- Tart VMs provide hermetic per-job environments — no state accumulation between runs.
- nix-darwin ensures the host is always in the declared state — Xcode version, Homebrew packages, launchd services, system settings.
- Adding a new Mac mini is: physical setup, enable Remote Login, run one Ansible playbook. ~30 minutes of human time.
- One-time hardware cost ($599 + $70 UPS) vs. ongoing monthly cost. Pays for itself vs. GitHub hosted runners in under a year.
- Tailscale connects the Mac mini to the cluster without exposing it publicly.

**Negative / tradeoffs:**
- Upfront hardware cost of ~$670. Justified but real.
- Physical hardware is a single point of failure for iOS CI. If the Mac mini fails, iOS CI is down until it's repaired or replaced. Mitigate with a UPS and monitoring.
- ARC cannot manage macOS runners — the Mac mini runs as a persistent registered runner, not an ephemeral pod. Runner tokens require management (GitHub App authentication mitigates this).
- nix-darwin has a learning curve. The nix language is unfamiliar to most developers. Mitigate with well-documented configurations and comments in the nix files.
- Xcode requires Apple ID credentials for download. One-time interactive setup per machine (`xcodes signin`) cannot be automated. Document clearly in setup runbook.

**Risks:**
- Tart or nix-darwin fall behind macOS updates and break. Mitigate by monitoring both projects' issue trackers and using stable, tested image versions.
- Disk fills up with Tart VM clones. Mitigate with a weekly cleanup launchd job that removes old images.
- Mac mini is physically damaged, stolen, or fails. Recovery: replace hardware, run bootstrap playbook. No data loss (CI runners are stateless).

---

## References

- Tart: https://github.com/cirruslabs/tart
- nix-darwin: https://github.com/LnL7/nix-darwin
- xcodes: https://github.com/XcodesOrg/xcodes
- Actions Runner Controller: https://github.com/actions/actions-runner-controller
- Cirrus Labs macOS VM images: https://github.com/cirruslabs/macos-image-templates
