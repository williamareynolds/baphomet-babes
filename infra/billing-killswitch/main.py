"""Cloud Function (gen2) that disables billing when the budget is exceeded.

Triggered by a Pub/Sub message from a Cloud Billing budget. When reported cost
meets or exceeds the budget amount, it unlinks the billing account from the
project, which stops all further billable usage (and takes the services
offline). This is a hard spend cap — GCP budgets only *alert* by default; they
never stop spending on their own.

Billing data lags by hours, so real spend can overshoot the cap somewhat before
the unlink lands. Treat the budget as "stop the bleeding," not a precise ceiling.

Re-enabling billing is a manual step (Console → Billing → link account), by
design — recovery should be a human decision, not automatic.
"""

import base64
import json
import os

import functions_framework
from googleapiclient import discovery

# Project to disable billing on. Set at deploy time; falls back to the runtime's
# own project if unset.
TARGET_PROJECT = os.environ.get("TARGET_PROJECT") or os.environ.get(
    "GOOGLE_CLOUD_PROJECT"
)


@functions_framework.cloud_event
def stop_billing(cloud_event):
    payload = base64.b64decode(cloud_event.data["message"]["data"]).decode("utf-8")
    budget = json.loads(payload)

    cost = budget.get("costAmount", 0)
    limit = budget.get("budgetAmount", 0)

    if cost < limit:
        print(f"Under budget — cost {cost} < limit {limit}. No action.")
        return

    project_name = f"projects/{TARGET_PROJECT}"
    billing = discovery.build("cloudbilling", "v1", cache_discovery=False)
    projects = billing.projects()

    if not _billing_enabled(project_name, projects):
        print("Billing already disabled. No action.")
        return

    print(f"Cost {cost} >= limit {limit}. DISABLING BILLING on {project_name}.")
    _disable_billing(project_name, projects)


def _billing_enabled(project_name, projects):
    res = projects.getBillingInfo(name=project_name).execute()
    return res.get("billingEnabled", False)


def _disable_billing(project_name, projects):
    # Empty billingAccountName unlinks the billing account.
    body = {"billingAccountName": ""}
    res = projects.updateBillingInfo(name=project_name, body=body).execute()
    print(f"Billing disabled: {json.dumps(res)}")
