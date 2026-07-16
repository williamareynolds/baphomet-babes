// Service worker: offline app shell + asset caching + FCM background push.
//
// This MUST be the only service worker on the site. A push subscription is
// bound to a scope's registration, and only that scope's active worker gets
// `push` events — a second worker registered at "/" would replace this one
// and silently swallow every notification (which is exactly the outage we
// had when the offline shell and the FCM worker were separate files fighting
// over the root scope).
//
// Strategy:
//   - navigations  -> network-first, fall back to the cached shell offline (so
//     an online reload always lands on the latest index.html / wasm).
//   - other GETs   -> cache-first (Trunk's hashed wasm/js/css/icons are
//     immutable, so this is safe and fast).
//   - API calls    -> not handled here (cross-origin Cloud Run goes to network).
//   - version.json -> never cached; it drives the in-app update check.
//   - push         -> Firebase compat SDK auto-displays "notification" payloads
//     when the app is backgrounded; we add click handling on top.
//
// Uses the compat SDK because service workers can't reliably import ES modules.
// importScripts sources are cached with the worker, so this works offline too.

importScripts(
  "https://www.gstatic.com/firebasejs/11.1.0/firebase-app-compat.js",
);
importScripts(
  "https://www.gstatic.com/firebasejs/11.1.0/firebase-messaging-compat.js",
);

firebase.initializeApp({
  apiKey: "AIzaSyCvALBMnTPVeeips_paWMxFW4N0-wEfBOo",
  projectId: "baphomet-babes",
  messagingSenderId: "780823612423",
  appId: "1:780823612423:web:ad385cf10a5b4d7f076ea2",
});

// Initializing messaging registers the background push handler internally.
firebase.messaging();

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target = (event.notification.data && event.notification.data.url) || "/";
  event.waitUntil(
    clients.matchAll({ type: "window", includeUncontrolled: true }).then((wins) => {
      // Focus an existing tab if one is open; otherwise open a new one.
      for (const win of wins) {
        if ("focus" in win) {
          win.navigate?.(target);
          return win.focus();
        }
      }
      if (clients.openWindow) return clients.openWindow(target);
    }),
  );
});

// Bump this whenever a deploy must forcibly evict every client's cached shell.
// Changing the string re-installs the worker (sw.js differs byte-for-byte), and
// `activate` below deletes all caches whose name != CACHE — so stale hashed
// assets (e.g. an old wasm bundle an installed iOS PWA was pinning) are purged
// and the next fetch goes to network. v2: drop the pre-SVG Leaflet marker build.
// v3: ride edit + free-text notes field.
// v4: post-a-ride moved to a sticky bar + bottom sheet.
const CACHE = "bb-shell-v4";
const SHELL = ["/"];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((c) => c.addAll(SHELL))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  const url = new URL(req.url);

  if (req.method !== "GET" || url.origin !== self.location.origin) return;
  if (url.pathname === "/version.json") return; // always fresh from network

  if (req.mode === "navigate") {
    event.respondWith(
      fetch(req)
        .then((res) => {
          const copy = res.clone();
          caches.open(CACHE).then((c) => c.put("/", copy));
          return res;
        })
        .catch(() => caches.match("/")),
    );
    return;
  }

  event.respondWith(
    caches.match(req).then(
      (hit) =>
        hit ||
        fetch(req).then((res) => {
          if (res.ok) {
            const copy = res.clone();
            caches.open(CACHE).then((c) => c.put(req, copy));
          }
          return res;
        }),
    ),
  );
});
