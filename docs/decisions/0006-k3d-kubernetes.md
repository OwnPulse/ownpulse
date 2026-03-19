# ADR-0006: k3s on Servers, k3d on Dev Machines

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse needs to run multiple services (API, web frontend, Postgres, CI runners, ingress controller, cert-manager, Sealed Secrets controller) in a managed, reproducible way. The deployment model must work across three distinct environments:

- **Production/staging servers** — DigitalOcean droplet (US), Hetzner (EU, Phase 2)
- **Dev machines** — macOS laptops where contributors iterate locally
- **End-user self-hosting** — someone's VPS, where they run their own OwnPulse instance

The deployment model must:
- Be fully reproducible from configuration in git
- Support multiple services with internal networking
- Handle TLS automatically
- Allow zero-downtime deploys
- Work on a single VPS (4GB RAM)
- Be manageable by AI agents via Helm chart updates
- Not require expensive managed Kubernetes services
- Give end-user self-hosters a simple path that doesn't require deep Kubernetes expertise

---

## Decision

Use **k3s** (lightweight Kubernetes) as the single deployment model across all environments:

- **On servers (DigitalOcean, Hetzner):** k3s installed directly via the official installer script. Single command, no Docker layer.
- **On dev machines (macOS):** k3d, which runs k3s inside Docker — the only practical way to run k3s on macOS.
- **For end-user self-hosting:** k3s directly on any Linux VPS, same single-command install, then the same `helm install` commands.

The Helm charts in the application repo work identically against all three. There is one deployment model, not two. Docker Compose is not used or maintained.

**k3s vs k3d clarified:**
- **k3s** — lightweight Kubernetes distribution that runs natively on Linux. Production-grade, single binary, low resource overhead. This is what runs on servers.
- **k3d** — a tool that runs k3s inside Docker containers, for use on macOS or other systems where k3s can't run natively. Development only.

---

## Alternatives Considered

### Managed Kubernetes (DigitalOcean Kubernetes, GKE, EKS)

Managed Kubernetes removes control plane operational overhead. Automatic upgrades, managed node pools.

Rejected because:
- DigitalOcean Kubernetes starts at ~$12/mo for the control plane on top of node costs — significantly more expensive than a raw droplet for a single-node cluster.
- Conflicts with the cost philosophy of minimizing monthly outlay.
- At OwnPulse's scale, operational complexity is the same whether managed or self-hosted.

Can revisit when the platform needs managed multi-node infrastructure.

### Docker Compose

Docker Compose is simpler than Kubernetes and well-understood. A `docker-compose.yml` with API, web, and Postgres would work for Phase 1.

Rejected because:
- Maintaining two deployment configurations (Helm + Docker Compose) doubles the surface area for configuration drift — the same environment variable, secret reference, or service name would need to stay in sync across both.
- ARC (CI runners) is a Kubernetes-native operator — it cannot run under Docker Compose.
- k3s end-user self-hosting (`curl -sfL https://get.k3s.io | sh -`, then `helm install`) is genuinely simple — comparable to Docker Compose for anyone comfortable with a VPS. The extra complexity of Kubernetes is largely hidden by Helm.
- Docker Compose has no equivalent to `helm upgrade`'s atomic, rollback-capable deploys.
- Dev/prod parity is stronger with k3d locally — contributors test against the same Helm charts running in k3s as production uses.

### Nomad (HashiCorp)

Lighter-weight workload orchestrator than Kubernetes.

Rejected because ARC requires Kubernetes, and the Helm ecosystem (Bitnami Postgres, cert-manager, ingress-nginx, Sealed Secrets) is Kubernetes-specific.

### Fly.io, Railway, Render (PaaS)

Fully managed deployment without infrastructure concerns.

Rejected because:
- Limited control over data residency — EU user data must stay on EU infrastructure for GDPR compliance.
- Vendor lock-in contradicts the self-hosting ethos.
- Cannot run ARC or Woodpecker.

### Single-binary deploy (no Kubernetes)

Compile the API, web frontend, and a migration runner into a single binary with an embedded SQLite or external Postgres connection. No orchestration.

Appealing for simplicity. Rejected because:
- Multiple services (API, web nginx, Postgres) genuinely benefit from orchestration even at small scale — health checks, restart policies, resource limits, rolling deploys.
- ARC and Woodpecker are Kubernetes-native and central to the CI strategy.
- The Helm chart model lets AI agents deploy changes without understanding the underlying infrastructure.

---

## Consequences

**Positive:**
- One deployment model across dev, production, and end-user self-hosting — no configuration drift between environments.
- k3d locally gives contributors a faithful replica of production (`k3d cluster create ownpulse-local`).
- k3s on servers installs with a single curl command — as simple as it gets for Kubernetes.
- Full Kubernetes API — ARC, cert-manager, Sealed Secrets, ingress-nginx all work without modification.
- Helm provides atomic, rollback-capable deploys — `helm rollback` if something goes wrong.
- Scaling to EU region (Phase 2) is a second identical k3s cluster, not an architectural change.
- End-user self-hosters use the same `helm install` commands as production — documented in `docs/guides/self-hosting.md`.

**Negative / tradeoffs:**
- k3s control plane uses ~512MB RAM. On a 4GB VPS this leaves ~3.5GB for application workloads — sufficient but not generous.
- Kubernetes has real operational complexity (CRDs, RBAC, networking). Mitigated by using Helm for all deployments and keeping everything in git.
- k3d on macOS requires Docker Desktop (or OrbStack) running. One extra thing for contributors to install.
- End-user self-hosting requires a Linux VPS — can't self-host on Windows easily. Acceptable given the target audience.

**Risks:**
- VPS runs out of memory as the cluster grows. Mitigate: monitor resource usage, upgrade droplet tier when Postgres + API + web + CI runners approach the limit.
- k3s upgrade breaks cluster state. Mitigate: treat the cluster as ephemeral — all persistent state is in Postgres. Daily pg_dump to DigitalOcean Spaces means recovery is a fresh k3s install + `helm install` + restore from backup.

---

## Self-Hosting Path for End Users

The self-hosting guide (`docs/guides/self-hosting.md`) documents this flow:

```bash
# 1. Install k3s (any Linux VPS, ~30 seconds)
curl -sfL https://get.k3s.io | sh -

# 2. Install Helm
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# 3. Add OwnPulse Helm repo
helm repo add ownpulse https://charts.ownpulse.health
helm repo update

# 4. Install OwnPulse
helm install ownpulse ownpulse/ownpulse \
  --set domain=health.yourdomain.com \
  --set postgres.password=<your-password> \
  --set jwt.secret=$(openssl rand -hex 32)
```

This is the only supported self-hosting path. It uses the same Helm charts as production.

---

## References

- k3s documentation: https://k3s.io
- k3d documentation: https://k3d.io
- Actions Runner Controller: https://github.com/actions/actions-runner-controller
- Sealed Secrets: https://github.com/bitnami-labs/sealed-secrets
- cert-manager: https://cert-manager.io
- Helm: https://helm.sh
