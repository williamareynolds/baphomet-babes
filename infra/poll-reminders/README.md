# Email notifications & poll reminders

Members get club email through Resend, on the same per-channel preference model
as push. The point is turnout: voting happens on an external rcv123 poll that
the backend can't see into, so we can't tell who has voted or chase only the
people who haven't — the lever we do have is reaching everyone, including the
members who never installed the PWA or never granted push permission.

```
notification created (new movie night, announcement, …)
  └─ notifications::dispatch
       ├─ persist to the inbox feed
       ├─ FCM fan-out   → devices whose push pref allows the channel
       └─ email fan-out → members whose email pref allows the channel
                            └─ each message carries its own unsubscribe link

Cloud Scheduler (daily, 9am Central)
  └─ POST /events/poll-reminders  (X-Reminder-Secret)
       └─ events in Voting stage whose poll_deadline is within 2 days
            └─ dispatch → "Last call to vote"  (push + email + inbox)
```

## What's in the code

- **`backend/src/email.rs`** — Resend sender. Constructed only when
  `RESEND_API_KEY` is set, so dev and tests run with `email: None` and every
  send is a no-op, exactly like FCM.
- **`backend/src/routes/notifications.rs`** — `email_fanout` and the message
  template, hung off the existing `dispatch`.
- **`backend/src/routes/email.rs`** — unsubscribe capability URLs. GET offers,
  POST performs (mail scanners prefetch links; a mutating GET would unsubscribe
  people who never clicked). Exempt from App Check, like the calendar feed.
- **`backend/src/routes/events.rs`** — `poll_deadline` on events, plus the
  `/events/poll-reminders` job endpoint and its `needs_poll_reminder` window
  rules.
- **`hub/src/pages/profile.rs`** — per-channel email toggles.
- **`hub/src/pages/admin_events.rs`** — the "Voting closes" date field.

## Defaults

Only **movie night** email is on by default. Announcements, general, and
mountain bike are opt-in, so club mail stays rare enough to get opened. Chat has
no email channel at all — it delivers via `push_only` and never reaches the
fan-out.

## One-time setup

1. **Resend account + domain.** Add `baphometbabes.com` in Resend and publish
   the DKIM/SPF records it gives you. Until the domain verifies, sends fail.
2. **GitHub Actions config**, read by the Cloud Run deploy step in `ci.yml`.
   Mind which tab each one lives in — a secret read as a variable (or the
   reverse) arrives as an empty string, not an error:
   - `RESEND_API_KEY` — **secret**. From the Resend dashboard.
   - `REMINDER_SECRET` — **secret**. Any long random string
     (`openssl rand -hex 32`). Must match the value passed to `setup.sh` below.
   - `EMAIL_FROM` — **variable** (it isn't sensitive). Either
     `noreply@baphometbabes.com` or `Baphomet Babes <noreply@baphometbabes.com>`;
     the address must be on the verified domain. Empty falls back to the latter.
3. **Deploy** so the service picks up the new env vars.
4. **Scheduler**, using the same secret as step 2:

   ```sh
   REMINDER_SECRET=<same value> infra/poll-reminders/setup.sh
   ```

Skipping steps 1–3 is safe: with no API key the backend simply doesn't send, and
the reminder endpoint refuses every call while `REMINDER_SECRET` is unset.

## Operating it

Fire the job by hand:

```sh
gcloud scheduler jobs run poll-reminders --location us-central1 --project baphomet-babes
```

It responds `{"reminded": n}`. Reminders are stamped per event
(`poll_reminder_sent_at`), so a poll is nudged once — re-running the job the
same day sends nothing. Moving an event's `poll_deadline` clears the stamp and
re-arms it, which is what you want when voting gets extended.

An event only qualifies while it's in the **Voting** stage: a poll URL is set,
no date has been chosen yet, and the deadline falls between today and two days
out. Deadlines already in the past are skipped rather than nudged late.
