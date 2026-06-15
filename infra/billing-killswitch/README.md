# Billing kill-switch

Hard spend cap for the `baphomet-babes` GCP project. GCP budgets only *alert* by
default — they never stop spending. This wires a budget to a function that
**disables billing** when the cap is hit, so a bot/DDoS attack takes the apps
offline instead of running up an unbounded bill.

```
Cloud Billing budget ($30/mo)
        │  publishes status several times/day
        ▼
   Pub/Sub topic  (billing-killswitch)
        │  triggers
        ▼
   Cloud Function (stop_billing)
        │  cost >= budget?
        ▼
   unlink billing account  ──►  all billable usage stops
```

## Deploy

```sh
just setup-killswitch        # or: infra/billing-killswitch/setup.sh
```

Idempotent — safe to re-run after editing `main.py` (redeploys the function).

## Caveats

- **Billing data lags hours.** Real spend can overshoot $30 (think $40–60) before
  the unlink lands. This stops the bleeding; it is not a precise ceiling.
- **Disabling billing takes the site down.** Cloud Run, Firestore, Functions all
  stop. That is the point — better dark than a four-figure bill.
- **Recovery is manual, on purpose.** Re-link in Console → Billing → the project →
  *Link a billing account*. A human should decide it is safe to turn back on.
- The cap covers the whole project, not per-service.

## Test the wiring (without disabling anything)

Publish an *under-budget* message — the function logs "No action" and stops:

```sh
gcloud pubsub topics publish billing-killswitch --project=baphomet-babes \
  --message='{"costAmount":1,"budgetAmount":30}'

gcloud functions logs read billing-killswitch --gen2 --region=us-central1 \
  --project=baphomet-babes --limit=10
```

Do **not** test with `costAmount >= budgetAmount` — that really unlinks billing.
