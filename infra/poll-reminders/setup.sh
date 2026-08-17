#!/usr/bin/env bash
# Idempotent setup for the daily poll-reminder job.
#
# Wiring:
#   Cloud Scheduler (daily)  ->  POST /events/poll-reminders on Cloud Run
#
# The endpoint authorizes on a shared secret in the X-Reminder-Secret header,
# not on a member session — there is no user behind a cron tick. The same secret
# must be set as REMINDER_SECRET on the Cloud Run service; CI reads it from the
# REMINDER_SECRET GitHub secret, so set it in both places or the job 403s.
#
# The endpoint is idempotent (each event carries a poll_reminder_sent_at stamp),
# so a retried or double-fired tick sends nothing extra.
#
# Safe to re-run; each step is create-if-missing.
set -euo pipefail

PROJECT="baphomet-babes"
REGION="us-central1"
JOB="poll-reminders"
SERVICE_URL="https://movie-night-api-r6vuubbgla-uc.a.run.app"
# 15:00 UTC ≈ 9am Central, so the nudge lands in the morning rather than
# overnight. Cron runs in the schedule's own timezone, set below.
SCHEDULE="0 9 * * *"
TIMEZONE="America/Chicago"

if [ -z "${REMINDER_SECRET:-}" ]; then
  echo "REMINDER_SECRET must be set in the environment." >&2
  echo "Use the same value that is stored as the REMINDER_SECRET GitHub secret." >&2
  exit 1
fi

echo "==> Enabling APIs"
gcloud services enable cloudscheduler.googleapis.com --project "$PROJECT"

# create-or-update: `create` fails if the job exists, so fall back to `update`.
# Plain strings rather than ${ACTION^} — macOS ships bash 3.2, which has no
# case-modifying parameter expansion and aborts the script with "bad
# substitution" under `set -e`.
if gcloud scheduler jobs describe "$JOB" --location "$REGION" --project "$PROJECT" >/dev/null 2>&1; then
  ACTION="update"
  # `update http` has no --headers; it takes --update-headers (merge) instead.
  # Passing the create-only flag here fails the command *and* echoes the
  # rejected argument — secret included — into the terminal, so getting this
  # right is what keeps a rotation from leaking the new value.
  HEADER_FLAG="--update-headers"
  echo "==> Job '$JOB' exists; updating it"
else
  ACTION="create"
  HEADER_FLAG="--headers"
  echo "==> Creating Cloud Scheduler job '$JOB'"
fi
# Belt and braces: gcloud prints offending arguments on a usage error, so scrub
# the secret out of anything this command emits before it reaches a terminal or
# a CI log.
gcloud scheduler jobs "$ACTION" http "$JOB" \
  --location "$REGION" \
  --project "$PROJECT" \
  --schedule "$SCHEDULE" \
  --time-zone "$TIMEZONE" \
  --uri "${SERVICE_URL}/events/poll-reminders" \
  --http-method POST \
  "$HEADER_FLAG" "X-Reminder-Secret=${REMINDER_SECRET}" \
  --attempt-deadline 120s \
  --max-retry-attempts 3 \
  2>&1 | sed "s/${REMINDER_SECRET}/[redacted]/g"

echo
echo "Done. Fire it once by hand with:"
echo "  gcloud scheduler jobs run $JOB --location $REGION --project $PROJECT"
