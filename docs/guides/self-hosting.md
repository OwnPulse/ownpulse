# Self-Hosting Guide

OwnPulse is designed to run on your own infrastructure. The same Helm charts used in production work on any Linux VPS.

## Requirements

- Linux VPS with at least 2 GB RAM (4 GB recommended)
- A domain name with DNS control
- SSH access to the VPS

## Step 1: Install k3s

```bash
curl -sfL https://get.k3s.io | sh -
```

This installs a lightweight Kubernetes distribution. Takes about 30 seconds. After installation, `kubectl` is available at `/usr/local/bin/kubectl`.

Verify:

```bash
kubectl get nodes
```

## Step 2: Install Helm

```bash
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

## Step 3: DNS Setup

Point two DNS A records to your VPS IP:

```
api.yourdomain.com  →  <VPS IP>
app.yourdomain.com  →  <VPS IP>
```

TLS certificates are provisioned automatically by cert-manager (installed with the Helm chart).

## Step 4: Install OwnPulse

```bash
# Clone the repo and deploy from local Helm charts
git clone https://github.com/OwnPulse/ownpulse.git
cd ownpulse

helm install postgres helm/postgres -n ownpulse --create-namespace
helm install api helm/api -n ownpulse \
  --set domain=yourdomain.com \
  --set postgres.password=$(openssl rand -hex 16) \
  --set jwt.secret=$(openssl rand -hex 32) \
  --set encryption.key=$(openssl rand -hex 32)
helm install web helm/web -n ownpulse --set domain=yourdomain.com
```

> **Note:** A public Helm chart repository will be available in a future release. For now, deploy directly from the cloned repo.

This deploys:
- PostgreSQL 16
- OwnPulse API (Rust binary)
- OwnPulse web frontend (nginx)
- Ingress controller with TLS via Let's Encrypt

## Step 5: Create Your Account

Once the pods are running:

```bash
kubectl get pods -n ownpulse
```

Open `https://app.yourdomain.com` in your browser and create your account.

## Backups

Set up a daily PostgreSQL backup:

```bash
# Example: daily pg_dump to a local file
kubectl exec -n ownpulse deploy/postgres -- \
  pg_dump -U postgres ownpulse | gzip > /backups/ownpulse-$(date +%F).sql.gz
```

For offsite backups, sync to S3-compatible storage (DigitalOcean Spaces, Backblaze B2, etc.).

## Upgrades

```bash
cd ownpulse
git pull
helm upgrade postgres helm/postgres -n ownpulse
helm upgrade api helm/api -n ownpulse
helm upgrade web helm/web -n ownpulse
```

Helm performs atomic, rollback-capable upgrades. If something goes wrong:

```bash
helm rollback ownpulse -n ownpulse
```

## Troubleshooting

Check pod status:

```bash
kubectl get pods -n ownpulse
kubectl logs -n ownpulse deploy/api
kubectl logs -n ownpulse deploy/web
```

Check resource usage:

```bash
kubectl top pods -n ownpulse
```

## References

- [k3s documentation](https://k3s.io)
- [Helm documentation](https://helm.sh)
- [ADR-0006: k3s deployment model](../decisions/0006-k3d-kubernetes.md)
