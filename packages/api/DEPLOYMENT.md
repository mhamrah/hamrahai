# Deployment Guide

## Cloud Run Service

The application is deployed to Google Cloud Run:

- **Project**: hamrah-ai
- **Region**: us-central1
- **Service**: hamrah-api
- **URL**: https://hamrah-api-a7tefmgk7q-uc.a.run.app
- **Billing mode**: Request-based billing (`--cpu-throttling`) so idle instances are not billed

## Configuration

### Required Secrets

Create the following secrets in Google Cloud Secret Manager:

```bash
# Database connection string
gcloud secrets create DATABASE_URL \
  --project=hamrah-ai \
  --replication-policy=automatic

# JWT secret for authentication
gcloud secrets create JWT_SECRET \
  --project=hamrah-ai \
  --replication-policy=automatic
```

Set secret values:

```bash
# Set DATABASE_URL
echo -n "postgresql://user:pass@host/db" | gcloud secrets versions add DATABASE_URL --data-file=-

# Set JWT_SECRET
echo -n "your-secure-random-string" | gcloud secrets versions add JWT_SECRET --data-file=-
```

Grant Cloud Run access to secrets:

```bash
gcloud secrets add-iam-policy-binding DATABASE_URL \
  --member="serviceAccount:PROJECT_NUMBER-compute@developer.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"

gcloud secrets add-iam-policy-binding JWT_SECRET \
  --member="serviceAccount:PROJECT_NUMBER-compute@developer.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

### Artifact Registry

Create Artifact Registry repository for container images:

```bash
gcloud artifacts repositories create hamrah \
  --repository-format=docker \
  --location=us-central1 \
  --project=hamrah-ai
```

## Music Sync Configuration

The API is the only component that communicates with music providers. Do not
expose provider configuration to the web or native clients.

| Runtime setting | Purpose |
| --- | --- |
| `SPOTIFY_CLIENT_ID` | GitHub Actions repository variable; Spotify developer application client ID |
| `SPOTIFY_CLIENT_SECRET` | Secret Manager secret; Spotify developer application client secret |
| `SPOTIFY_REDIRECT_URI` | Deployment workflow value: `https://api.hamrah.app/v1/music/connections/spotify/callback` |
| `TIDAL_CLIENT_ID` | GitHub Actions repository variable; TIDAL developer application client ID |
| `TIDAL_REDIRECT_URI` | Deployment workflow value: `https://api.hamrah.app/v1/music/connections/tidal/callback` |
| `MUSIC_TOKEN_ENCRYPTION_KEY` | Secret Manager secret; a 32-byte base64url key used only to encrypt provider tokens at rest |
| `WEB_APP_URL` | Deployment workflow value: `https://hamrah.app` post-OAuth redirect destination |

Register both callback URLs exactly with their respective providers. Spotify
development mode users must be allowlisted. Run the first live TIDAL import
against a test account after enabling its playlist and collection write scopes.

## GitHub Actions Setup

### Required Repository Variables

Configure the following GitHub Actions repository variables:

| Variable | Value |
| --- | --- |
| `GCP_PROJECT_ID` | `hamrah-ai` |
| `GCP_PROJECT_NUMBER` | `66020219411` |
| `GCP_REGION` | `us-central1` |
| `CLOUD_RUN_SERVICE` | `hamrah-api` |
| `GAR_LOCATION` | `us-central1` |
| `GAR_REPOSITORY` | `hamrah` |
| `GAR_REGISTRY` | `us-central1-docker.pkg.dev` |
| `WIF_PROVIDER` | `projects/66020219411/locations/global/workloadIdentityPools/github-pool/providers/github-provider` |
| `APPLE_TEAM_ID` | Apple Developer Team ID used to verify App Attest app IDs |
| `SPOTIFY_CLIENT_ID` | Spotify developer application client ID |
| `TIDAL_CLIENT_ID` | TIDAL developer application client ID |
| `NEON_PROJECT_ID` | Neon project ID used for API CI/release validation branches |
| `NEON_PARENT_BRANCH` | Sanitized Neon branch used as the parent for short-lived CI branches |

### Required Repository Secrets

| Secret | Purpose |
| --- | --- |
| `NEON_API_KEY` | Creates and deletes short-lived Neon branches for API CI/release validation |

### Workload Identity Federation Setup

```bash
# Create workload identity pool
gcloud iam workload-identity-pools create github-pool \
  --project=hamrah-ai \
  --location=global \
  --display-name="GitHub Actions Pool"

# Create provider. Keep the attribute condition scoped to the GitHub owner.
gcloud iam workload-identity-pools providers create-oidc github-provider \
  --project=hamrah-ai \
  --location=global \
  --workload-identity-pool=github-pool \
  --display-name="GitHub Provider" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" \
  --attribute-condition="assertion.repository_owner == 'mhamrah'" \
  --issuer-uri="https://token.actions.githubusercontent.com"

# Grant Direct WIF permissions to this repository principal.
WORKLOAD_IDENTITY_POOL_ID="projects/66020219411/locations/global/workloadIdentityPools/github-pool"
GITHUB_REPO="mhamrah/hamrahai"
PRINCIPAL_SET="principalSet://iam.googleapis.com/${WORKLOAD_IDENTITY_POOL_ID}/attribute.repository/${GITHUB_REPO}"
RUNTIME_SERVICE_ACCOUNT="66020219411-compute@developer.gserviceaccount.com"

gcloud projects add-iam-policy-binding hamrah-ai \
  --member="${PRINCIPAL_SET}" \
  --role="roles/run.admin"

gcloud iam service-accounts add-iam-policy-binding "${RUNTIME_SERVICE_ACCOUNT}" \
  --project=hamrah-ai \
  --member="${PRINCIPAL_SET}" \
  --role="roles/iam.serviceAccountUser"

gcloud artifacts repositories add-iam-policy-binding hamrah \
  --project=hamrah-ai \
  --location=us-central1 \
  --member="${PRINCIPAL_SET}" \
  --role="roles/artifactregistry.writer"
```

## Manual Deployment

To deploy manually:

```bash
# Build and push image
IMAGE="us-central1-docker.pkg.dev/hamrah-ai/hamrah/hamrah-api:$(git rev-parse --short HEAD)"
docker build -t "$IMAGE" .
docker push "$IMAGE"

# Deploy to Cloud Run
gcloud run deploy hamrah-api \
  --project hamrah-ai \
  --region us-central1 \
  --image "$IMAGE" \
  --platform managed \
  --allow-unauthenticated \
  --min-instances 0 \
  --max-instances 1 \
  --memory 512Mi \
  --cpu 1 \
  --port 8080 \
  --cpu-throttling \
            --startup-probe httpGet.path=/readyz,httpGet.port=8080,initialDelaySeconds=10,periodSeconds=10,timeoutSeconds=3,failureThreshold=18 \
            --liveness-probe httpGet.path=/healthz,httpGet.port=8080,initialDelaySeconds=30,periodSeconds=30,timeoutSeconds=3,failureThreshold=3 \
            --set-env-vars "RUST_LOG=info,APPLE_TEAM_ID=${APPLE_TEAM_ID}" \
            --set-secrets "DATABASE_URL=DATABASE_URL:latest,JWT_SECRET=JWT_SECRET:latest"
```

## CI/CD Pipeline

The GitHub Actions workflow (`.github/workflows/api-deploy.yml`) automatically:

1. Authenticates to Google Cloud using Workload Identity Federation
2. Builds the Docker image
3. Pushes to Artifact Registry
4. Deploys to Cloud Run

Deployments trigger on:
- Push to `main` branch
- Manual workflow dispatch

## Database Migrations

Migrations run automatically on application startup. The service will:
1. Connect to the database
2. Run pending migrations from `./migrations`
3. Start accepting requests

If migration fails, the process exits before binding to port `8080`, so the
Cloud Run revision will fail startup instead of serving against a stale schema.

## Health Checks

- **Health endpoint**: `GET /healthz`
- **Ready endpoint**: `GET /readyz`

## Monitoring

View logs and metrics:

```bash
# View logs
gcloud run services logs tail hamrah-api --region=us-central1

# View service details
gcloud run services describe hamrah-api --region=us-central1
```
