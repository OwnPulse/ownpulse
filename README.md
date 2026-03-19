# OwnPulse

OwnPulse is an open source health data platform owned by its users as a cooperative. You track interventions, wearable data, labs, and subjective scores; the platform finds correlations and lets you export everything in open formats. Licensed AGPL-3.0; the open data schema is CC0.

## Quick Start: Self-Hosting

```bash
# Install k3s and Helm
curl -sfL https://get.k3s.io | sh -
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# Add the OwnPulse Helm repo and install
helm repo add ownpulse https://charts.ownpulse.health
helm repo update
helm install ownpulse ownpulse/ownpulse \
  --set domain=health.yourdomain.com \
  --set postgres.password=<your-password> \
  --set jwt.secret=$(openssl rand -hex 32)
```

Minimum requirements: Linux VPS with 2 GB RAM. See [docs/guides/self-hosting.md](docs/guides/self-hosting.md) for the full guide.

## Quick Start: Contributors

```bash
# Backend (Rust)
docker run -d -e POSTGRES_PASSWORD=dev -p 5432:5432 --name pg postgres:16
export DATABASE_URL=postgres://postgres:dev@localhost:5432/health
cd db && sqlx migrate run
cd backend && cargo test

# Web (React + Vite)
cd web && npm install && npm run dev

# iOS (Swift + SwiftUI)
open ios/OwnPulse.xcodeproj
xcodebuild test -scheme OwnPulse \
  -destination 'platform=iOS Simulator,name=iPhone 16'
```

Read [CLAUDE.md](CLAUDE.md) for conventions, then [AGENTS.md](AGENTS.md) for workspace boundaries.

## Documentation

- [Architecture overview](docs/architecture/overview.md)
- [API reference](docs/architecture/api.md)
- [Data model](docs/architecture/data-model.md)
- [Architecture Decision Records](docs/decisions/)
- [Contributing guide](docs/guides/contributing.md)
- [Agent guide](docs/guides/agent-guide.md)
- [Testing strategy](docs/guides/testing.md)
- [Self-hosting guide](docs/guides/self-hosting.md)
- [Local development](docs/guides/local-dev.md)
- [Cooperative governance](docs/cooperative/governance.md)
- [Data sharing](docs/cooperative/data-sharing.md)
- [Privacy principles](docs/cooperative/privacy-principles.md)

## License

Code: [AGPL-3.0](https://www.gnu.org/licenses/agpl-3.0.html)

Open data schema (`schema/`): [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/)
