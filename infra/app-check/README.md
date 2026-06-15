# Firebase App Check

App Check proves a request came from a genuine load of our web apps (attested
via reCAPTCHA Enterprise) rather than a script or bot. The backend verifies the
token and rejects everything else — so scripted abuse can't reach Firestore or
burn Cloud Run cycles even with a stolen JWT.

```
browser (hub / movienight)
  └─ reCAPTCHA Enterprise → App Check token (X-Firebase-AppCheck header)
        └─ backend middleware verifies RS256 vs Google JWKS, checks aud+iss
              └─ valid → handler   |   missing/invalid → 401
```

## What's already in the code

- **Backend** (`backend/src/app_check.rs`): verifier + axum middleware. Enforced
  only when `APP_CHECK_ENFORCE=true` (+ `APP_CHECK_PROJECT_NUMBER`). Off by
  default. `/health` and CORS preflight always pass.
- **Frontends** (`hub/index.html`, `frontend/index.html`): App Check bootstrap
  that exposes `window.__appCheckToken()`. Inert on localhost; active only on
  `*.baphometbabes.com`. **Placeholders** (`FIREBASE_WEB_*`, `SITE_KEY`) must be
  filled before deploy.
- **Bridge** (`auth-client`): `app_check_token()` → both apps attach the header
  to every backend call.
- **Test** (`backend/tests/integration.rs::app_check_blocks_direct_api_access`):
  proves missing/garbage tokens, and even a valid JWT, are blocked when enforced.

## Rollout (sequenced — do NOT flip enforcement first, or you lock everyone out)

We use **reCAPTCHA v3 (classic)** — a site key (public) + secret key (private).
One key pair covers both apps as long as both domains are on the key.

### 1. reCAPTCHA v3 key

Created at https://www.google.com/recaptcha/admin (reCAPTCHA v3). Ensure the
key's domain list includes `baphometbabes.com` and `movienight.baphometbabes.com`.
You get a **site key** (public) and a **secret key** (private).

### 2. Register web apps + get config

In Firebase Console → Project settings → *Your apps* → add two Web apps (hub,
movienight) if not present. Then:

```sh
firebase apps:list WEB --project baphomet-babes
firebase apps:sdkconfig WEB <APP_ID> --project baphomet-babes   # per app
```

Fill `apiKey` / `appId` into the matching `index.html`, and the **site key**
into both `SITE_KEY` constants.

### 3. Link the key in App Check

Console → App Check → for each web app, register the **reCAPTCHA v3** provider:
paste the **site key** and the **secret key**. Firebase uses the secret server-
side to validate tokens — it lives only here, never in the repo. Leave
**enforcement OFF** for now (monitoring).

### 4. Deploy frontends (still unenforced)

```sh
just build-hub && just deploy-hub
just build-movienight && just deploy-movienight
```

Load each site, exercise it, then watch Console → App Check → Metrics. Confirm
**verified** requests climbing and **outdated/invalid** near zero. This proves
real users mint valid tokens before anything is enforced.

### 5. Flip backend enforcement

Once metrics look clean, redeploy the backend with:

```
APP_CHECK_ENFORCE=true
APP_CHECK_PROJECT_NUMBER=780823612423
```

(Add both to the Cloud Run deploy env in
`.github/workflows/deploy-backend.yml`, then push or run the workflow.)

### 6. Verify the gate

```sh
# No token → 401 (bot's-eye view)
curl -i -X POST https://movie-night-api-<hash>-uc.a.run.app/auth/login \
  -H 'content-type: application/json' -d '{"email":"x","password":"y"}'
# expect: HTTP/1.1 401 ... {"error":"missing app check token"}
```

Real apps keep working (they send the header); raw curl/bots get 401.

## Rollback

Set `APP_CHECK_ENFORCE=false` (or remove it) and redeploy the backend. The
verifier goes dormant immediately — no frontend change needed.
