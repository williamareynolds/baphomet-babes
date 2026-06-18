// Firebase Cloud Messaging background worker.
//
// Separate from sw.js (offline shell). Firebase auto-displays "notification"
// messages when the page is backgrounded; this worker only adds click handling
// so tapping a notification focuses the app (and honors an optional data.url).
//
// Uses the compat SDK because service workers can't reliably import ES modules.

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

// Initializing messaging registers the background message handler internally.
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
