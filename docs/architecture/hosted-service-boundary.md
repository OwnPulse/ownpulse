# Hosted Service Boundary

OwnPulse is designed so the application can be reviewed, self-hosted, and forked, while the managed service can charge a small fee for hosting and storage.

## Repository Responsibilities

| Repository | Responsibility |
|------------|----------------|
| `ownpulse` | Open application product: API, web app, iOS app, schema, documentation, and self-hosting charts. |
| `ownpulse-web` | Public website for explaining the project, collecting interest, and converting users to managed hosting. |
| `ownpulse-infra` | Private hosted-service operations: cloud resources, secrets, deployments, backups, runners, observability, and recovery. |
| `ownpulse-dev` | Developer workflow tooling only. |
| `gh-actions` | Shared reusable CI/CD actions. |

## Product Boundary

The hosted service must not rely on hidden application behavior. Users should be able to inspect the app code paths that handle health data, export their data, delete their account, and self-host the same product surface.

Managed hosting may provide:

- Operated API, web, and database infrastructure.
- Storage, backups, restore support, monitoring, and upgrades.
- Secure secret management and production incident response.
- Convenience onboarding and support.

Managed hosting must not provide:

- Proprietary-only health data features required to avoid lock-in.
- Hidden telemetry or data sharing behavior.
- Different export/delete semantics from the self-hosted app.
- Access to user data outside explicit operational needs and documented policy.

## Launch Implications

V1 launch readiness requires both product quality and operational credibility. The open app should pass its backend, web, iOS, contract, and self-hosting checks. The hosted service should prove deployment, backup, restore, monitoring, rollback, and incident-handling paths before paid storage is offered.
