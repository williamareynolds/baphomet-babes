#!/usr/bin/env bash
# Idempotent setup for the uploaded-media bucket (gathering cover images).
#
# Wiring:
#   hub  ->  POST /gatherings/cover  ->  Cloud Run  ->  GCS bucket (public read)
#
# The backend proxies the bytes rather than handing out signed upload URLs: at a
# few images a month that is far less machinery, and it reuses the JWT auth we
# already have instead of bucket CORS plus IAM signBlob.
#
# Objects are PUBLIC-READ. Cover images are decoration, and serving them through
# the API would push image bytes through Cloud Run on every page load. Names are
# UUIDs, so URLs are unguessable — but this bucket is not private storage. Never
# put anything sensitive in it.
#
# Safe to re-run; each step is create-if-missing.
# Requires bash 3.2 (macOS default) — no ${VAR^} or other bash 4 syntax.
set -euo pipefail

PROJECT="baphomet-babes"
REGION="us-central1"
BUCKET="baphomet-babes-media"
# Cloud Run's runtime service account — the identity the backend uploads as.
SERVICE="movie-night-api"

echo "==> Enabling APIs"
gcloud services enable storage.googleapis.com --project "$PROJECT"

if gcloud storage buckets describe "gs://${BUCKET}" --project "$PROJECT" >/dev/null 2>&1; then
  echo "==> Bucket gs://${BUCKET} already exists"
else
  echo "==> Creating bucket gs://${BUCKET}"
  # Uniform access: per-object ACLs are legacy and interact badly with the
  # bucket-wide public read binding below.
  gcloud storage buckets create "gs://${BUCKET}" \
    --project "$PROJECT" \
    --location "$REGION" \
    --uniform-bucket-level-access
fi

echo "==> Granting public read"
gcloud storage buckets add-iam-policy-binding "gs://${BUCKET}" \
  --project "$PROJECT" \
  --member=allUsers \
  --role=roles/storage.objectViewer

RUNTIME_SA=$(gcloud run services describe "$SERVICE" \
  --region "$REGION" --project "$PROJECT" \
  --format="value(spec.template.spec.serviceAccountName)")
if [ -z "$RUNTIME_SA" ]; then
  # An unset service account means Cloud Run uses the default compute SA.
  PROJECT_NUMBER=$(gcloud projects describe "$PROJECT" --format="value(projectNumber)")
  RUNTIME_SA="${PROJECT_NUMBER}-compute@developer.gserviceaccount.com"
  echo "==> Service has no explicit SA; using the default compute SA"
fi

echo "==> Letting ${RUNTIME_SA} write objects"
gcloud storage buckets add-iam-policy-binding "gs://${BUCKET}" \
  --project "$PROJECT" \
  --member="serviceAccount:${RUNTIME_SA}" \
  --role=roles/storage.objectAdmin

echo
echo "Done. Set MEDIA_BUCKET=${BUCKET} as a GitHub Actions *variable* so the"
echo "next deploy picks it up, then redeploy. Until then the upload endpoint"
echo "returns \"image uploads aren't configured on this server\" and everything"
echo "else about gatherings works normally."
