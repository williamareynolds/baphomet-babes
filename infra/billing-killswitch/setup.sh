#!/usr/bin/env bash
# Idempotent setup for the billing kill-switch.
#
# Wiring:
#   Cloud Billing budget ($30/mo)  ->  Pub/Sub topic  ->  Cloud Function
#   The function unlinks the billing account when cost >= budget, which stops
#   all billable usage (apps go offline). Re-enabling billing is manual, by
#   design.
#
# Safe to re-run; each step is create-if-missing.
set -euo pipefail

PROJECT="baphomet-babes"
PROJECT_NUMBER="780823612423"
BILLING_ACCOUNT="0150A3-C0AEFB-AFD2D9"
REGION="us-central1"
BUDGET_AMOUNT="30USD"
TOPIC="billing-killswitch"
FUNCTION="billing-killswitch"
SA="cf-billing-killswitch-sa"
SA_EMAIL="${SA}@${PROJECT}.iam.gserviceaccount.com"
SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Enabling APIs"
gcloud services enable \
  cloudbilling.googleapis.com \
  billingbudgets.googleapis.com \
  cloudfunctions.googleapis.com \
  run.googleapis.com \
  cloudbuild.googleapis.com \
  eventarc.googleapis.com \
  pubsub.googleapis.com \
  artifactregistry.googleapis.com \
  --project="$PROJECT"

echo "==> Pub/Sub topic: $TOPIC"
gcloud pubsub topics describe "$TOPIC" --project="$PROJECT" >/dev/null 2>&1 \
  || gcloud pubsub topics create "$TOPIC" --project="$PROJECT"

echo "==> Service account: $SA_EMAIL"
gcloud iam service-accounts describe "$SA_EMAIL" --project="$PROJECT" >/dev/null 2>&1 \
  || gcloud iam service-accounts create "$SA" \
       --display-name="Billing kill-switch function" --project="$PROJECT"

echo "==> Grant Project Billing Manager (lets the function unlink billing)"
# roles/billing.projectManager carries resourcemanager.projects.deleteBillingAssignment.
# A freshly created SA can take a few seconds to be visible to IAM; retry.
for attempt in $(seq 1 6); do
  if gcloud projects add-iam-policy-binding "$PROJECT" \
       --member="serviceAccount:${SA_EMAIL}" \
       --role="roles/billing.projectManager" \
       --condition=None >/dev/null 2>&1; then
    break
  fi
  echo "    SA not visible yet (attempt ${attempt}/6) — waiting 10s..."
  sleep 10
  [ "$attempt" -eq 6 ] && { echo "    grant still failing; re-run 'just setup-killswitch'"; exit 1; }
done

echo "==> Grant Pub/Sub the token-creator role (lets it invoke the function)"
# The Pub/Sub service agent mints identity tokens to deliver to the gen2
# function via Eventarc. Without this, messages never reach the function.
PUBSUB_AGENT="service-${PROJECT_NUMBER}@gcp-sa-pubsub.iam.gserviceaccount.com"
gcloud projects add-iam-policy-binding "$PROJECT" \
  --member="serviceAccount:${PUBSUB_AGENT}" \
  --role="roles/iam.serviceAccountTokenCreator" \
  --condition=None >/dev/null

echo "==> Deploy Cloud Function (gen2)"
gcloud functions deploy "$FUNCTION" \
  --gen2 \
  --runtime=python312 \
  --region="$REGION" \
  --source="$SOURCE_DIR" \
  --entry-point=stop_billing \
  --trigger-topic="$TOPIC" \
  --service-account="$SA_EMAIL" \
  --set-env-vars="TARGET_PROJECT=${PROJECT}" \
  --max-instances=1 \
  --project="$PROJECT"

echo "==> Allow the trigger SA to invoke the function's Run service"
# Gen2 functions are backed by Cloud Run; the Eventarc trigger SA needs
# run.invoker or delivery is rejected with 401 and the function never runs.
gcloud run services add-iam-policy-binding "$FUNCTION" \
  --member="serviceAccount:${SA_EMAIL}" \
  --role="roles/run.invoker" \
  --region="$REGION" --project="$PROJECT" >/dev/null

echo "==> Budget: $BUDGET_AMOUNT/mo -> $TOPIC"
# Attaching the topic here makes Cloud Billing auto-grant itself publish rights.
if gcloud billing budgets list --billing-account="$BILLING_ACCOUNT" \
     --format="value(displayName)" 2>/dev/null | grep -qx "baphomet-babes hard cap"; then
  echo "    Budget already exists — leaving as-is."
else
  gcloud billing budgets create \
    --billing-account="$BILLING_ACCOUNT" \
    --display-name="baphomet-babes hard cap" \
    --budget-amount="$BUDGET_AMOUNT" \
    --filter-projects="projects/${PROJECT_NUMBER}" \
    --threshold-rule=percent=0.5 \
    --threshold-rule=percent=0.9 \
    --threshold-rule=percent=1.0 \
    --notifications-rule-pubsub-topic="projects/${PROJECT}/topics/${TOPIC}"
fi

echo "==> Done. Verify the trigger:"
echo "    gcloud functions describe ${FUNCTION} --gen2 --region=${REGION} --project=${PROJECT}"
