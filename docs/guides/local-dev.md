# Local Development Setup

Two options: a full local Kubernetes cluster (matches production) or individual services (faster iteration).

See `CLAUDE.md` in the repository root for full conventions.

## Option A: k3d Cluster (Full Local Kubernetes)

k3d runs k3s inside Docker on your dev machine. Use this when testing Helm chart changes, ingress config, or infrastructure.

### Prerequisites

- Docker Desktop or OrbStack
- k3d: `brew install k3d`
- Helm: `brew install helm`
- kubectl: `brew install kubectl`

### Setup

```bash
# Create local cluster
k3d cluster create ownpulse-local --port "8080:80@loadbalancer"

# Deploy all services via Helm
helm upgrade --install postgres helm/postgres -n ownpulse --create-namespace
helm upgrade --install api helm/api -n ownpulse
helm upgrade --install web helm/web -n ownpulse

# Run migrations
export DATABASE_URL=postgres://postgres:dev@localhost:5432/ownpulse
cd db && sqlx migrate run
```

The web frontend is at `http://localhost:8080` and the API at `http://localhost:8080/api/`.

### Teardown

```bash
k3d cluster delete ownpulse-local
```

## Option B: Services Only (Faster Iteration)

Run Postgres in Docker, services directly on your machine. Use this for day-to-day backend and web development.

### Prerequisites

- Docker (for Postgres)
- Rust toolchain: `rustup`
- Node.js 20+
- SQLx CLI: `cargo install sqlx-cli`

### Setup

```bash
# Start Postgres
docker run -d -e POSTGRES_PASSWORD=dev -p 5432:5432 --name pg postgres:17

# Run migrations
export DATABASE_URL=postgres://postgres:dev@localhost:5432/ownpulse
cd db && sqlx migrate run

# Prepare SQLx offline data
cd backend && cargo sqlx prepare --workspace

# Run the API (terminal 1)
cargo run -p api    # API on :8080

# Run the web frontend (terminal 2)
cd web && npm install && npm run dev    # Web on :5173

# iOS (Xcode)
open ios/OwnPulse.xcodeproj    # Point to http://localhost:8080
```

### Running Tests

Tests do not require a running database or cluster. Testcontainers handles Postgres automatically.

```bash
# Backend
cargo test

# Web
cd web && npm test
cd web && npm run test:e2e

# iOS
xcodebuild test -scheme OwnPulse \
  -destination 'platform=iOS Simulator,name=iPhone 16'
maestro test ios/maestro/flows/
```

## Environment Variables

Copy the example environment file and fill in values:

```bash
cp .env.example .env
```

See `CLAUDE.md` for the full list of environment variables and their descriptions.

## API Credentials for Integrations

Register developer accounts early -- some have approval delays:

- **Garmin Connect API** -- developer.garmin.com -- human review, 1-2 weeks
- **Oura API** -- cloud.ouraring.com/personal-access-tokens -- instant
- **Dexcom Developer** -- developer.dexcom.com -- approval required, a few days
- **Google Cloud Console** -- instant
