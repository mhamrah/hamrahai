# Deployment Guide

## Cloud Run service

The API is deployed to Google Cloud Run through Cloud Deploy.

- **Project**: `hamrah-ai`
- **Region**: `us-central1`
- **Service**: `hamrah-api`
- **URL**: https://hamrah-api-a7tefmgk7q-uc.a.run.app
- **Billing mode**: Request-based billing (`cpu-throttling: true`)

## Declarative delivery configuration

Deployment is YAML-only. The checked-in configuration is the source of truth:

| File                            | Responsibility                                                                                  |
| ------------------------------- | ----------------------------------------------------------------------------------------------- |
| `deploy/cloud-run-service.yaml` | Complete Cloud Run service: scaling, probes, non-secret settings, and Secret Manager references |
| `deploy/skaffold.yaml`          | Tells Cloud Deploy to render and deploy the Cloud Run manifest                                  |
| `deploy/clouddeploy.yaml`       | Defines the production delivery pipeline and its target                                         |

GitHub Actions builds and pushes an image, resolves its immutable digest, then
creates a Cloud Deploy release. Cloud Deploy replaces the `hamrah-api` image
placeholder in the service manifest and records the rendered manifest.

`APPLE_TEAM_ID`, `SPOTIFY_CLIENT_ID`, and `TIDAL_CLIENT_ID` are non-secret
GitHub repository variables. They are Cloud Deploy release parameters, declared
with `from-param` directives in the service manifest. They are not interpolated
into a shell-generated environment file. Other non-secret production settings
live directly in `cloud-run-service.yaml`.

Do not configure the service in the Cloud Run console or with ad-hoc `gcloud run
deploy` flags; the next release replaces service configuration with the
checked-in manifest.

## Secret Manager configuration

Create the runtime secrets:

```bash
for SECRET_NAME in DATABASE_URL JWT_SECRET SPOTIFY_CLIENT_SECRET MUSIC_TOKEN_ENCRYPTION_KEY; do
  gcloud secrets create "$SECRET_NAME" \
    --project=hamrah-ai \
    --replication-policy=automatic
done
```

Add values without placing them in source control:

```bash
printf '%s' 'postgresql://user:pass@host/db' | \
  gcloud secrets versions add DATABASE_URL --data-file=-
printf '%s' 'replace-with-a-secure-jwt-key' | \
  gcloud secrets versions add JWT_SECRET --data-file=-
printf '%s' 'spotify-client-secret' | \
  gcloud secrets versions add SPOTIFY_CLIENT_SECRET --data-file=-
printf '%s' '32-byte-base64url-encryption-key' | \
  gcloud secrets versions add MUSIC_TOKEN_ENCRYPTION_KEY --data-file=-
```

The Cloud Run service identity must access every secret:

```bash
PROJECT_NUMBER=66020219411
RUNTIME_SERVICE_ACCOUNT="${PROJECT_NUMBER}-compute@developer.gserviceaccount.com"

for SECRET_NAME in DATABASE_URL JWT_SECRET SPOTIFY_CLIENT_SECRET MUSIC_TOKEN_ENCRYPTION_KEY; do
  gcloud secrets add-iam-policy-binding "$SECRET_NAME" \
    --project=hamrah-ai \
    --member="serviceAccount:${RUNTIME_SERVICE_ACCOUNT}" \
    --role="roles/secretmanager.secretAccessor"
done
```

Secrets use `secretKeyRef` in `cloud-run-service.yaml`. Never pass their values
as Cloud Deploy parameters, GitHub variables, or Docker build arguments.

## Music sync configuration

The API alone communicates with music providers.

| Runtime setting              | Source                                                                |
| ---------------------------- | --------------------------------------------------------------------- |
| `SPOTIFY_CLIENT_ID`          | GitHub repository variable passed as a Cloud Deploy release parameter |
| `SPOTIFY_CLIENT_SECRET`      | Secret Manager reference in the service manifest                      |
| `SPOTIFY_REDIRECT_URI`       | `https://api.hamrah.app/v1/music/connections/spotify/callback`        |
| `TIDAL_CLIENT_ID`            | GitHub repository variable passed as a Cloud Deploy release parameter |
| `TIDAL_REDIRECT_URI`         | `https://api.hamrah.app/v1/music/connections/tidal/callback`          |
| `MUSIC_TOKEN_ENCRYPTION_KEY` | Secret Manager reference used to encrypt provider tokens at rest      |
| `WEB_APP_URL`                | `https://hamrah.app`                                                  |

Register both callback URLs exactly with their providers. Spotify development
mode users must be allowlisted. Run the first live TIDAL import against a test
account after enabling `playlists.write`, `collection.write`, `search.read`, and
`user.read`. Existing TIDAL connections must reconnect after new scopes deploy.
See [MUSIC_IMPORT.md](MUSIC_IMPORT.md) for the full import contract.

## Authentication runtime configuration

The service manifest defines the WebAuthn RP ID and origin, OAuth audience
allowlists, and CORS origin allowlist. Do not allow these values to fall back to
localhost in Cloud Run.

| Runtime setting             | Production value                                        |
| --------------------------- | ------------------------------------------------------- |
| `WEBAUTHN_RP_ID`            | `hamrah.app`                                            |
| `WEBAUTHN_RP_ORIGIN`        | `https://hamrah.app`                                    |
| `GOOGLE_ALLOWED_CLIENT_IDS` | The comma-separated web and iOS Google OAuth client IDs |
| `APPLE_ALLOWED_CLIENT_IDS`  | `app.hamrah.ios,app.hamrah.web`                         |
| `CORS_ALLOWED_ORIGINS`      | `https://hamrah.app`                                    |

The Google audience allowlist must include the exact client ID in
`packages/web/wrangler.toml` and the iOS `GIDClientID`.

`hamrah.app` is the canonical web origin. If `www.hamrah.app` is added in
Cloudflare, configure it as a DNS/redirect rule to the apex—do not serve the
application from it, because the production WebAuthn origin is
`https://hamrah.app`.

## Artifact Registry

```bash
gcloud artifacts repositories create hamrah \
  --repository-format=docker \
  --location=us-central1 \
  --project=hamrah-ai
```

## GitHub Actions setup

### Required repository variables

| Variable             | Value                                                                                               |
| -------------------- | --------------------------------------------------------------------------------------------------- |
| `GCP_PROJECT_ID`     | `hamrah-ai`                                                                                         |
| `GCP_PROJECT_NUMBER` | `66020219411`                                                                                       |
| `GCP_REGION`         | `us-central1`                                                                                       |
| `CLOUD_RUN_SERVICE`  | `hamrah-api`                                                                                        |
| `GAR_LOCATION`       | `us-central1`                                                                                       |
| `GAR_REPOSITORY`     | `hamrah`                                                                                            |
| `GAR_REGISTRY`       | `us-central1-docker.pkg.dev`                                                                        |
| `WIF_PROVIDER`       | `projects/66020219411/locations/global/workloadIdentityPools/github-pool/providers/github-provider` |
| `APPLE_TEAM_ID`      | Apple Developer Team ID for App Attest app IDs                                                      |
| `SPOTIFY_CLIENT_ID`  | Spotify developer application client ID                                                             |
| `TIDAL_CLIENT_ID`    | TIDAL developer application client ID                                                               |
| `NEON_PROJECT_ID`    | Neon project ID for release validation branches                                                     |
| `NEON_PARENT_BRANCH` | Sanitized Neon CI-branch parent                                                                     |

### Required repository secrets

| Secret         | Purpose                                                  |
| -------------- | -------------------------------------------------------- |
| `NEON_API_KEY` | Creates and deletes short-lived Neon validation branches |

### Cloud Deploy bootstrap and IAM

Enable Cloud Deploy once before the first release:

```bash
gcloud services enable clouddeploy.googleapis.com --project=hamrah-ai
```

The GitHub workload-identity principal applies the delivery-pipeline YAML and
creates releases. Cloud Deploy uses the default Compute Engine service account
to render and deploy the service.

```bash
WORKLOAD_IDENTITY_POOL_ID="projects/66020219411/locations/global/workloadIdentityPools/github-pool"
GITHUB_REPO="mhamrah/hamrahai"
PRINCIPAL_SET="principalSet://iam.googleapis.com/${WORKLOAD_IDENTITY_POOL_ID}/attribute.repository/${GITHUB_REPO}"
EXECUTION_SERVICE_ACCOUNT="66020219411-compute@developer.gserviceaccount.com"

# GitHub Actions: update Cloud Deploy configuration and create releases.
gcloud projects add-iam-policy-binding hamrah-ai \
  --member="${PRINCIPAL_SET}" \
  --role="roles/clouddeploy.admin"

gcloud iam service-accounts add-iam-policy-binding "${EXECUTION_SERVICE_ACCOUNT}" \
  --project=hamrah-ai \
  --member="${PRINCIPAL_SET}" \
  --role="roles/iam.serviceAccountUser"

# Cloud Deploy: render and deploy the public Cloud Run service.
gcloud projects add-iam-policy-binding hamrah-ai \
  --member="serviceAccount:${EXECUTION_SERVICE_ACCOUNT}" \
  --role="roles/clouddeploy.jobRunner"

gcloud projects add-iam-policy-binding hamrah-ai \
  --member="serviceAccount:${EXECUTION_SERVICE_ACCOUNT}" \
  --role="roles/run.admin"

gcloud iam service-accounts add-iam-policy-binding "${EXECUTION_SERVICE_ACCOUNT}" \
  --project=hamrah-ai \
  --member="serviceAccount:${EXECUTION_SERVICE_ACCOUNT}" \
  --role="roles/iam.serviceAccountUser"

gcloud artifacts repositories add-iam-policy-binding hamrah \
  --project=hamrah-ai \
  --location=us-central1 \
  --member="serviceAccount:${EXECUTION_SERVICE_ACCOUNT}" \
  --role="roles/artifactregistry.reader"

# GitHub Actions still builds and pushes images.
gcloud artifacts repositories add-iam-policy-binding hamrah \
  --project=hamrah-ai \
  --location=us-central1 \
  --member="${PRINCIPAL_SET}" \
  --role="roles/artifactregistry.writer"
```

The public-access annotation needs `roles/run.admin` on the Cloud Deploy
execution account. GitHub Actions receives `roles/clouddeploy.admin` because it
applies the checked-in pipeline definition before every release; replace it with
a restricted custom role later if desired.

### Workload Identity Federation

If the workload-identity pool is not already configured:

```bash
gcloud iam workload-identity-pools create github-pool \
  --project=hamrah-ai \
  --location=global \
  --display-name="GitHub Actions Pool"

gcloud iam workload-identity-pools providers create-oidc github-provider \
  --project=hamrah-ai \
  --location=global \
  --workload-identity-pool=github-pool \
  --display-name="GitHub Provider" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" \
  --attribute-condition="assertion.repository_owner == 'mhamrah'" \
  --issuer-uri="https://token.actions.githubusercontent.com"
```

## Manual release

```bash
IMAGE="us-central1-docker.pkg.dev/hamrah-ai/hamrah/hamrah-api:$(git rev-parse --short HEAD)"
docker build -t "$IMAGE" .
docker push "$IMAGE"

IMAGE_DIGEST="$(gcloud artifacts docker images describe "$IMAGE" --format='value(image_summary.digest)')"
IMAGE_REFERENCE="${IMAGE%:*}@${IMAGE_DIGEST}"

gcloud deploy apply \
  --file=packages/api/deploy/clouddeploy.yaml \
  --project=hamrah-ai \
  --region=us-central1

gcloud deploy releases create "manual-$(date -u +%Y%m%d%H%M%S)" \
  --delivery-pipeline=hamrah-api \
  --project=hamrah-ai \
  --region=us-central1 \
  --source=packages/api/deploy \
  --images="hamrah-api=${IMAGE_REFERENCE}" \
  --deploy-parameters="apple_team_id=${APPLE_TEAM_ID},spotify_client_id=${SPOTIFY_CLIENT_ID},tidal_client_id=${TIDAL_CLIENT_ID}"
```

## CI/CD pipeline

The GitHub workflow (`.github/workflows/api-deploy.yml`) authenticates using
Workload Identity Federation, validates the API against a short-lived Neon
branch, builds and pushes an Artifact Registry image, applies the checked-in
Cloud Deploy configuration, and creates a digest-pinned release.

Deployments trigger on pushes to `main` that change `packages/api/**` or the
deployment workflow, and on manual dispatch.

## Database migrations

Migrations run automatically on startup. If migration fails, the process exits
before binding to port `8080`, so the new revision fails instead of serving
against a stale schema.

## Health checks and monitoring

- **Health endpoint**: `GET /healthz`
- **Ready endpoint**: `GET /readyz`

```bash
gcloud run services logs tail hamrah-api --region=us-central1
gcloud run services describe hamrah-api --region=us-central1
gcloud deploy releases list --delivery-pipeline=hamrah-api --region=us-central1
```
