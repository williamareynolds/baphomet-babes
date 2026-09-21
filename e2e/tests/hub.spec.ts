import { test, expect, type Page } from "@playwright/test";

// Must match SUPERADMIN_INVITE_CODE in playwright.config.ts
const BOOT_CODE = "e2e-boot-code";
const EMAIL = "root@e2e.test";
const USERNAME = "rootbabe";
const PASSWORD = "sup3r-secret-pw";

// Tests build on each other (register → login → profile → directory).
test.describe.configure({ mode: "serial" });

async function login(page: Page, email = EMAIL, password = PASSWORD) {
  await page.goto("/login");
  await page.fill("#login-email", email);
  await page.fill("#login-password", password);
  await page.click('form button[type="submit"]');
  // Wait for auth to land (saved to localStorage) before navigating further.
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();
  // Clear the notification onboarding bar so its sticky chrome can't overlay
  // the targets below it.
  const dismiss = page.getByRole("button", { name: "Dismiss" });
  if (await dismiss.isVisible().catch(() => false)) await dismiss.click();
}

// The notifications inbox has no page of its own (removed along with /about);
// these hit the backend feed directly with the logged-in member's token,
// preserving the same regression coverage the deleted page used to give.
async function inboxFeed(page: Page): Promise<{ channel: string; title: string }[]> {
  return page.evaluate(async () => {
    const auth = JSON.parse(localStorage.getItem("auth_user") ?? "{}");
    const res = await fetch("http://localhost:8080/notifications", {
      headers: { Authorization: `Bearer ${auth.token}` },
    });
    return res.json();
  });
}

async function clearInbox(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const auth = JSON.parse(localStorage.getItem("auth_user") ?? "{}");
    await fetch("http://localhost:8080/notifications/clear", {
      method: "POST",
      headers: { Authorization: `Bearer ${auth.token}` },
    });
  });
}

test("register the bootstrap superadmin", async ({ page }) => {
  await page.goto("/login");
  await page.getByRole("button", { name: "Register" }).click();

  await page.fill("#reg-email", EMAIL);
  await page.fill("#reg-username", USERNAME);
  await page.fill("#reg-password", PASSWORD);
  await page.fill("#reg-invite", BOOT_CODE);
  await page.click('form button[type="submit"]');

  // Successful registration redirects home, logged in.
  await expect(page).toHaveURL("/");
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();
  await expect(page.getByText(`Welcome back, ${USERNAME}`)).toBeVisible();
});

test("login with the registered account", async ({ page }) => {
  await login(page);
  await expect(page).toHaveURL("/");
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();
});

test("wrong password shows an error and stays logged out", async ({ page }) => {
  await page.goto("/login");
  await page.fill("#login-email", EMAIL);
  await page.fill("#login-password", "wrong-password");
  await page.click('form button[type="submit"]');
  await expect(page.locator(".error")).toHaveText("invalid credentials");
  await expect(page.getByRole("button", { name: "Logout" })).not.toBeVisible();
});

test("logout clears the session", async ({ page }) => {
  await login(page);
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();

  await page.getByRole("button", { name: "Logout" }).click();
  await expect(page.getByRole("link", { name: "Login" })).toBeVisible();

  // Session is really gone after reload, not just hidden.
  await page.reload();
  await expect(page.getByRole("link", { name: "Login" })).toBeVisible();
});

test("edit profile and publish it", async ({ page }) => {
  await login(page);
  await page.goto("/profile");

  await page.getByPlaceholder("Leave blank to use username").fill("Root Babe");
  await page.getByPlaceholder("they/them, she/her, …").fill("they/them");
  // Email and phone share a placeholder; email comes first in the form.
  const contact = page.getByPlaceholder("Optional — only shown if profile is public");
  await contact.nth(0).fill("root@babes.test");
  await contact.nth(1).fill("479-555-0666");
  await page
    .getByPlaceholder("A few words about yourself…")
    .fill("Founding member. Crafts, cosmos, and cinema.");
  // The first switch is "Public profile" (the notification-channel switches
  // follow it further down the page).
  await page.getByRole("switch").first().check();

  await page.getByRole("button", { name: "Save Profile" }).click();
  await expect(page.locator(".success")).toHaveText("Profile saved.");

  // Saved values survive a reload.
  await page.reload();
  await expect(
    page.getByPlaceholder("Leave blank to use username"),
  ).toHaveValue("Root Babe");
  await expect(page.getByRole("switch").first()).toBeChecked();
});

test("published profile appears in the member directory", async ({ page }) => {
  await login(page);
  await page.goto("/members");

  const card = page.locator(".thaw-card").filter({ hasText: "Root Babe" });
  await expect(card).toBeVisible();
  // Directory cards surface the published contact details directly.
  await expect(card.locator(".member-contact")).toContainText("root@babes.test");
  await expect(card.locator(".member-contact")).toContainText("479-555-0666");
  await card.click();

  // Full profile page renders the published details.
  await expect(
    page.getByRole("heading", { name: "Root Babe" }),
  ).toBeVisible();
  await expect(
    page.getByText("Founding member. Crafts, cosmos, and cinema."),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Edit Profile" }),
  ).toBeVisible();
});

test("a protected page redirects to login when logged out", async ({ page }) => {
  // The site-wide auth guard bounces any session-less visitor to /login,
  // regardless of which route they request.
  await page.goto("/members");
  await expect(page).toHaveURL("/login");
  await expect(page.locator("#login-email")).toBeVisible();
});

test("admin can create then edit an event in place", async ({ page }) => {
  await login(page); // root is superadmin → admin
  await page.goto("/admin/events");

  // Create an event. The date lives behind the "Already scheduled?"
  // disclosure — the normal flow leaves it for the poll to decide.
  await page.getByPlaceholder("Movie title").fill("The Crow");
  await page.getByRole("button", { name: /Already scheduled/ }).click();
  await page.locator('input[type="date"]').first().fill("2030-10-31");
  await page.getByRole("button", { name: "Create Event" }).click();
  await expect(page.locator(".success")).toHaveText("Event created!");

  const card = page.locator(".thaw-card").filter({ hasText: "The Crow" });
  await expect(card).toBeVisible();

  // Edit it: the Edit button must swap the row for the form in place
  // (regression guard — the card body is a reactive closure over editing_id).
  await card.getByRole("button", { name: "Edit" }).click();
  await expect(page.locator("#edit-title")).toBeVisible();
  await page.locator("#edit-title").fill("The Crow (1994)");
  await page.getByRole("button", { name: "Save" }).click();

  // Form swaps back to the display row, now showing the new title.
  await expect(
    page.locator(".thaw-card").filter({ hasText: "The Crow (1994)" }),
  ).toBeVisible();
  await expect(page.locator("#edit-title")).toHaveCount(0);
});

test("movie nights features the next screening and dates it nicely", async ({
  page,
}) => {
  await login(page);
  await page.goto("/movie-nights");

  // The soonest upcoming event ("The Crow (1994)", 2030-10-31) is the marquee
  // hero, with the kicker, title, and a humanized date.
  const hero = page.locator(".next-feature");
  await expect(hero).toBeVisible();
  await expect(hero.locator(".kicker")).toHaveText("Next Feature");
  await expect(hero.locator(".feature-title")).toHaveText("The Crow (1994)");
  await expect(hero.locator(".feature-date")).toHaveText("October 31, 2030");

  // The hero is NOT repeated in the archive below it — it already headlines.
  await expect(
    page.locator(".mn-row").filter({ hasText: "The Crow (1994)" }),
  ).toHaveCount(0);
});

test("a poll's voting deadline is public, and undated films lead the list", async ({
  page,
}) => {
  await login(page);
  await page.goto("/admin/events");

  // An undated film with an open poll and a voting cutoff. The schedule
  // disclosure stays shut, so the only date input on the form is "Voting
  // closes" — the cutoff is deliberately not tied to having a screening date.
  await page.getByPlaceholder("Movie title").fill("Nosferatu");
  await page
    .getByPlaceholder("https://rcv123.org/poll/...")
    .fill("https://rcv123.org/poll/e2e");
  await page.locator('input[type="date"]').first().fill("2030-09-15");
  await page.getByRole("button", { name: "Create Event" }).click();
  await expect(page.locator(".success")).toHaveText("Event created!");

  await page.goto("/movie-nights");

  // The Crow is dated and upcoming, so it still headlines; Nosferatu is
  // undated and therefore leads the list rather than sinking below it.
  await expect(page.locator(".next-feature .feature-title")).toHaveText(
    "The Crow (1994)",
  );
  const rows = page.locator(".mn-row");
  await expect(rows.first()).toContainText("Nosferatu");

  // The cutoff is visible to members, who otherwise can't see it without
  // opening the rcv123 poll itself.
  await expect(rows.first().locator(".mn-deadline")).toHaveText(
    "Voting closes September 15, 2030",
  );

  // Clean up so later serial tests see the original single-event schedule.
  await page.goto("/admin/events");
  const card = page.locator(".thaw-card").filter({ hasText: "Nosferatu" });
  await card.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Nosferatu" }),
  ).toHaveCount(0);
});

test("a member can RSVP and admins see who's going", async ({ page }) => {
  await login(page);
  await page.goto("/movie-nights");

  // RSVP to the featured screening; the button flips and the count ticks up.
  const hero = page.locator(".next-feature");
  await expect(hero.locator(".feature-title")).toHaveText("The Crow (1994)");
  const rsvpBtn = hero.locator(".rsvp-btn");
  await expect(rsvpBtn).toContainText("RSVP");
  await rsvpBtn.click();
  await expect(rsvpBtn).toContainText("Going");
  await expect(hero.locator(".rsvp-count")).toHaveText("1 person going");

  // Admins see the count and can expand the attendee names; members can't
  // (enforced server-side — the names endpoint is admin-only).
  await page.goto("/admin/events");
  const card = page.locator(".thaw-card").filter({ hasText: "The Crow (1994)" });
  await expect(card).toContainText("1 going");
  await card.getByRole("button", { name: "View RSVPs" }).click();
  await expect(card.locator(".rsvp-names li")).toHaveCount(1);

  // Cancelling brings the count back to zero.
  await page.goto("/movie-nights");
  const hero2 = page.locator(".next-feature");
  await hero2.locator(".rsvp-btn").click();
  await expect(hero2.locator(".rsvp-btn")).toContainText("RSVP");
  await expect(hero2.locator(".rsvp-count")).toHaveText("0 going");
});

test("a member posts a ride, is auto-attending, and can bail and rejoin", async ({
  page,
}) => {
  await login(page);
  await page.goto("/rides");

  // Posting lives in a bottom sheet opened from the sticky action bar.
  await page.getByRole("button", { name: "Post a Ride" }).click();
  await expect(page.locator(".ride-sheet")).toHaveClass(/open/);

  // Post a ride: pick a trail and a time window.
  await page.locator("select").selectOption("Coler");
  await page.locator('input[type="datetime-local"]').nth(0).fill("2031-06-01T09:00");
  await page.locator('input[type="datetime-local"]').nth(1).fill("2031-06-01T11:00");
  await page.getByRole("button", { name: "Post Ride" }).click();
  // On success the sheet closes and the fresh ride pops onto the list.
  await expect(page.locator(".ride-sheet")).not.toHaveClass(/open/);

  // The ride card shows the trail, the poster, and — unlike movie nights —
  // the attendee names are visible to members. The creator is auto-attending.
  const card = page.locator(".thaw-card").filter({ hasText: "Posted by Root Babe" });
  await expect(card).toBeVisible();
  await expect(card.locator(".mn-title")).toHaveText("Coler");
  await expect(card.locator(".rsvp-count")).toHaveText("1 rider");
  await expect(card.locator(".rsvp-deadline")).toHaveText("Root Babe");
  await expect(card.locator(".rsvp-btn")).toContainText("Going");

  // Bailing drops them off the list; rejoining puts them back.
  await card.locator(".rsvp-btn").click();
  await expect(card.locator(".rsvp-count")).toHaveText("0 riders");
  await expect(card.locator(".rsvp-btn")).toContainText("Join this ride");
  await card.locator(".rsvp-btn").click();
  await expect(card.locator(".rsvp-count")).toHaveText("1 rider");

  // The creator can delete their ride.
  await card.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Posted by Root Babe" }),
  ).toHaveCount(0);
});

test("a ride can carry a meeting-spot pin and a contact link", async ({
  page,
}) => {
  await login(page);
  await page.goto("/rides");

  await page.getByRole("button", { name: "Post a Ride" }).click();
  await expect(page.locator(".ride-sheet")).toHaveClass(/open/);

  await page.locator("select").selectOption("Bike Park");
  await page.locator('input[type="datetime-local"]').nth(0).fill("2032-06-01T09:00");
  await page.locator('input[type="datetime-local"]').nth(1).fill("2032-06-01T11:00");

  // The Leaflet picker is self-hosted, so it initialises without any network;
  // tapping it drops a pin even if OSM tiles never load. A click at a fixed
  // pixel yields whatever coordinate lands there — we only care that a pin
  // registered, not where.
  // L.map(id) turns the div itself into the Leaflet container, so the class
  // lands on #bb-ride-map (not a descendant) once init runs.
  const map = page.locator("#bb-ride-map");
  await expect(map).toBeVisible();
  await expect(map).toHaveClass(/leaflet-container/);
  await map.click({ position: { x: 150, y: 110 } });
  await expect(page.locator(".ride-map-status")).toContainText("Pin dropped");

  // Free-text contact: a Signal group invite gets its own button on the card.
  const signalUrl = "https://signal.group/#CjQKIexampleinvite";
  await page.getByPlaceholder("phone, email, or a group-chat link").fill(signalUrl);

  await page.getByRole("button", { name: "Post Ride" }).click();
  await expect(page.locator(".ride-sheet")).not.toHaveClass(/open/);

  const card = page.locator(".thaw-card").filter({ hasText: "Posted by Root Babe" });
  await expect(card.locator(".mn-title")).toHaveText("Bike Park");

  // Meeting spot renders as three "open in maps" links — no embedded map — so
  // viewing the card never hits a tile server.
  const meetLinks = card.locator(".ride-meet a");
  await expect(meetLinks).toHaveCount(3);
  await expect(card.locator(".ride-meet")).toContainText("Apple");
  await expect(meetLinks.first()).toHaveAttribute("href", /maps\.apple\.com/);
  // A pin is never an embedded iframe/tile on the card.
  await expect(card.locator("iframe")).toHaveCount(0);
  await expect(card.locator(".leaflet-container")).toHaveCount(0);

  // The Signal invite becomes a labelled button pointing at the raw link.
  const signalBtn = card.locator(".ride-contact-btn");
  await expect(signalBtn).toHaveText("Join Signal group");
  await expect(signalBtn).toHaveAttribute("href", signalUrl);

  // Clean up so later serial tests see a fresh page.
  await card.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Posted by Root Babe" }),
  ).toHaveCount(0);
});

test("the rides page notification toggle syncs with profile settings", async ({
  page,
}) => {
  await login(page);
  await page.goto("/rides");

  // The opt-in toggle appears once prefs load; the channel is off by default.
  const toggle = page.locator(".ride-notify").getByRole("switch");
  await expect(toggle).toBeVisible();
  await expect(toggle).not.toBeChecked();

  // Flipping it saves immediately (no save button) — wait for the PUT to land
  // before navigating away, then the profile page's channel switch agrees.
  const saved = page.waitForResponse(
    (r) => r.url().includes("/notifications/prefs") && r.request().method() === "PUT",
  );
  await toggle.check();
  await saved;
  await page.goto("/profile");
  // Scoped to the push group: the same channel names appear again under the
  // email group, so an unscoped label filter matches two switches.
  const mtb = page
    .locator(".notif-channels .thaw-switch")
    .filter({ hasText: "Mountain Bike Rides" })
    .getByRole("switch");
  await expect(mtb).toBeChecked();

  // Turn it back off from the rides page so the channel ends where it started.
  await page.goto("/rides");
  await expect(toggle).toBeChecked();
  const savedOff = page.waitForResponse(
    (r) => r.url().includes("/notifications/prefs") && r.request().method() === "PUT",
  );
  await toggle.uncheck();
  await savedOff;
  await page.goto("/profile");
  await expect(
    page
      .locator(".notif-channels .thaw-switch")
      .filter({ hasText: "Mountain Bike Rides" })
      .getByRole("switch"),
  ).not.toBeChecked();
});

test("movie nights offers a calendar subscription link", async ({ page }) => {
  await login(page);
  await page.goto("/movie-nights");

  await expect(
    page.getByRole("heading", { name: "Subscribe to the calendar" }),
  ).toBeVisible();

  // The Apple/iCloud button is a webcal:// link to this member's .ics feed.
  const apple = page.getByRole("link", { name: "Apple / iCloud" });
  await expect(apple).toBeVisible();
  const href = await apple.getAttribute("href");
  expect(href).toMatch(/^webcal:\/\/.*\/calendar\/.+\/baphomet-babes\.ics$/);

  // The subscribe card sits above the full "All Screenings" archive list.
  const subBox = await page
    .getByRole("heading", { name: "Subscribe to the calendar" })
    .boundingBox();
  const listBox = await page
    .getByRole("heading", { name: "All Screenings" })
    .boundingBox();
  expect(subBox!.y).toBeLessThan(listBox!.y);
});

test("the install guide is reachable and lists steps", async ({ page }) => {
  await login(page);
  await page.getByRole("link", { name: "Install App" }).click();
  await expect(page).toHaveURL("/install");
  await expect(
    page.getByRole("heading", { name: "Install the App" }),
  ).toBeVisible();

  // The recommended card plus collapsible guides for other devices.
  await expect(page.getByText("Recommended for your device")).toBeVisible();
  await expect(page.locator(".install-details").first()).toBeVisible();
});

test("admin generates a named invite and can copy it", async ({ page }) => {
  await login(page); // root is superadmin → admin
  await page.goto("/admin/invites");

  // Generate a code with contact details.
  await page.getByPlaceholder("First name").fill("Morticia");
  await page.getByPlaceholder("Last name").fill("Addams");
  await page.getByPlaceholder("555-123-4567").fill("555-0666");
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");

  // The new code appears in the listing, tagged with the invitee's name, and
  // exposes a Copy button.
  const card = page.locator(".thaw-card").filter({ hasText: "Morticia Addams" });
  await expect(card).toBeVisible();
  await expect(card.getByText("555-0666")).toBeVisible();
  await expect(card.getByRole("button", { name: "Copy link" })).toBeVisible();
  await expect(card.getByRole("button", { name: "Copy code" })).toBeVisible();

  // "Revoke all unused" clears the spare codes (confirm dialog auto-accepted).
  page.on("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "Revoke all unused" }).click();
  await expect(page.locator(".success")).toContainText("Revoked");
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Morticia Addams" }),
  ).toHaveCount(0);
});

test("superadmin issues and revokes a calendar link for a non-member", async ({
  page,
}) => {
  await login(page); // root is superadmin
  await page.goto("/admin/invites");

  // The tab is superadmin-only, so it renders alongside Users.
  await expect(page.getByRole("link", { name: "Calendar links" })).toBeVisible();
  await page.getByRole("link", { name: "Calendar links" }).click();
  await expect(page).toHaveURL(/\/admin\/calendar-links$/);

  const stamp = Date.now();
  const guest = `Gomez${stamp}`;

  // Both fields are required, and the phone has to look like one — checked in
  // the browser before any request goes out.
  await page.getByPlaceholder("Who is this for?").fill(guest);
  await page.getByRole("button", { name: "Create link" }).click();
  await expect(page.locator(".error")).toContainText("phone number is required");

  await page.getByPlaceholder("555-123-4567").fill("nope");
  await page.getByRole("button", { name: "Create link" }).click();
  await expect(page.locator(".error")).toContainText("doesn't look like");

  // A valid one lands in the list with its phone number and a copy button.
  await page.getByPlaceholder("555-123-4567").fill("555-0123");
  await page.getByRole("button", { name: "Create link" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");

  // Scoped to the listing: the form's success message also quotes the name.
  const list = page.locator(".calendar-link-list");
  const card = list.locator(".thaw-card").filter({ hasText: guest });
  await expect(card).toBeVisible();
  await expect(card.getByText("555-0123")).toBeVisible();
  await expect(card.getByRole("button", { name: "Copy link" })).toBeVisible();

  // Revoking removes the record outright (confirm dialog auto-accepted).
  page.on("dialog", (d) => d.accept());
  await card.getByRole("button", { name: "Revoke" }).click();
  await expect(page.locator(".success")).toContainText("revoked");
  await expect(list.locator(".thaw-card").filter({ hasText: guest })).toHaveCount(
    0,
  );
});

test("a single-use invite link prefills register and onboards a member", async ({
  page,
}) => {
  // Admin mints a fresh member invite.
  await login(page); // root is superadmin → admin
  await page.goto("/admin/invites");
  await page.getByPlaceholder("First name").fill("Wednesday");
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");

  // Grab the generated code from its listing card.
  const card = page.locator(".thaw-card").filter({ hasText: "Wednesday" });
  await expect(card).toBeVisible();
  const code = ((await card.locator("code").first().textContent()) ?? "").trim();
  expect(code).not.toEqual("");

  // Simulate the recipient: a session-less visitor opening the share link.
  await page.evaluate(() => localStorage.clear());
  await page.goto(`/login?code=${encodeURIComponent(code)}`);

  // The link auto-switches to the register tab with the code prefilled — the
  // invitee never types it by hand.
  await expect(page.locator("#reg-invite")).toHaveValue(code);

  // They fill in the rest and land logged-in.
  const stamp = Date.now();
  await page.fill("#reg-email", `wednesday-${stamp}@e2e.test`);
  await page.fill("#reg-username", `wednesday${stamp}`);
  await page.fill("#reg-password", "member-pw-123");
  await page.click('form button[type="submit"]');

  await expect(page).toHaveURL("/");
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();

  // The link is single-use: re-registering with the same code is rejected.
  await page.getByRole("button", { name: "Logout" }).click();
  await page.goto(`/login?code=${encodeURIComponent(code)}`);
  await page.fill("#reg-email", `wednesday2-${stamp}@e2e.test`);
  await page.fill("#reg-username", `wednesday2${stamp}`);
  await page.fill("#reg-password", "member-pw-123");
  await page.click('form button[type="submit"]');
  await expect(page.locator(".error")).toContainText("already used");
});

test("admin panel lists usernames, chat display names, emails and phone numbers, and reset-email surfaces the not-configured error", async ({
  page,
}) => {
  // Mint a fresh member so this test doesn't depend on state from earlier
  // tests (e.g. whether the "wednesday" invite test ran first).
  await login(page); // root is superadmin → admin
  await page.goto("/admin/invites");
  await page.getByPlaceholder("First name").fill("Fester");
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");
  const inviteCard = page.locator(".thaw-card").filter({ hasText: "Fester" });
  const code = ((await inviteCard.locator("code").first().textContent()) ?? "").trim();

  const stamp = Date.now();
  const email = `fester-${stamp}@e2e.test`;
  const username = `fester${stamp}`;
  await page.evaluate(() => localStorage.clear());
  await page.goto(`/login?code=${encodeURIComponent(code)}`);
  await page.fill("#reg-email", email);
  await page.fill("#reg-username", username);
  await page.fill("#reg-password", "member-pw-123");
  await page.click('form button[type="submit"]');
  await expect(page).toHaveURL("/");

  // The member sets a phone number and a chat display name distinct from
  // their account username, but leaves their profile private — the admin
  // panel needs both to tell accounts apart (including matching who's who
  // in chat) regardless of that setting.
  const phone = "479-555-0199";
  const chatName = `Spooky ${username}`;
  await page.goto("/profile");
  await page
    .getByPlaceholder("Leave blank to use username")
    .fill(chatName);
  await page
    .getByPlaceholder("Optional — only shown if profile is public")
    .nth(1)
    .fill(phone);
  await page.getByRole("button", { name: "Save Profile" }).click();
  await expect(page.locator(".success")).toHaveText("Profile saved.");

  await page.getByRole("button", { name: "Logout" }).click();

  // Back in as the superadmin, the new member's row shows their username,
  // chat display name, email and phone.
  await login(page);
  await page.goto("/admin/users");
  const memberCard = page.locator(".thaw-card").filter({ hasText: username });
  await expect(memberCard).toBeVisible();
  await expect(memberCard.getByText(chatName)).toBeVisible();
  await expect(memberCard.getByText(email)).toBeVisible();
  await expect(memberCard.getByText(phone)).toBeVisible();

  // This harness runs the backend with no RESEND_API_KEY, so the real path to
  // check here is the error surfaced when email isn't configured. The full
  // emailed-link → set-password → login flow is covered against a mocked
  // Resend server in backend/tests/integration.rs (password_reset_* tests).
  await memberCard.getByRole("button", { name: "Email password reset" }).click();
  await expect(
    page.locator(".error").filter({ hasText: "email is not configured" }),
  ).toBeVisible();
});

test("the reset-password page validates a new password before calling the backend", async ({
  page,
}) => {
  // No token in the URL: the form still renders (so validation can be
  // checked), but the missing-token notice shows up top.
  await page.goto("/reset-password");
  await expect(
    page.locator(".error").filter({ hasText: "missing its token" }),
  ).toBeVisible();

  await page.fill("#reset-password", "short");
  await page.fill("#reset-confirm", "short");
  await page.click('form button[type="submit"]');
  await expect(
    page.locator(".error").filter({ hasText: "at least 8 characters" }),
  ).toBeVisible();

  await page.fill("#reset-password", "a-long-enough-password");
  await page.fill("#reset-confirm", "does-not-match");
  await page.click('form button[type="submit"]');
  await expect(
    page.locator(".error").filter({ hasText: "don't match" }),
  ).toBeVisible();
});

test("the reset-password page rejects a token that doesn't exist", async ({
  page,
}) => {
  await page.goto("/reset-password?token=not-a-real-token");
  await expect(
    page.locator(".error").filter({ hasText: "invalid or expired reset link" }),
  ).toBeVisible();
  // The account field never appears (the GET failed before it could learn the
  // email), and submission is blocked rather than silently failing later.
  await expect(page.locator("#reset-email")).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Save password" })).toBeDisabled();
});

test("profile exposes notification settings", async ({ page }) => {
  await login(page);
  await page.goto("/profile");
  await expect(
    page.getByRole("heading", { name: "Notifications" }),
  ).toBeVisible();
  // Push channels: seven (including Book Club), plus the admin-only test
  // channel (root is a superadmin). Counted per group rather than in total,
  // so adding a switch elsewhere on the page doesn't quietly pass by shifting
  // a single number.
  await expect(
    page.locator(".notif-channels").getByRole("switch"),
  ).toHaveCount(8);
  // Email channels: the six that can be emailed. Chat is absent by design —
  // it delivers push-only and never reaches the email fan-out.
  await expect(
    page.locator(".email-channels").getByRole("switch"),
  ).toHaveCount(6);
  await expect(
    page.locator(".email-channels").filter({ hasText: "Group Chat" }),
  ).toHaveCount(0);
  // Plus the standalone public-profile switch above the notification section.
  await expect(page.getByRole("switch")).toHaveCount(15);
  await expect(
    page.locator(".thaw-switch").filter({ hasText: "Test Messages (admins only)" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Save Notification Settings" }),
  ).toBeVisible();
  // Headless Chromium hard-denies the Notification API, which doubles as
  // coverage for the blocked-state recovery guidance (shown instead of a
  // dead Enable Push button).
  await expect(page.locator(".push-blocked")).toContainText(
    "remove the app from your Home Screen",
  );
});

test("an admin announcement lands in the notifications inbox", async ({
  page,
}) => {
  await login(page);
  await page.goto("/admin/announcements");
  await page.getByPlaceholder("What's happening").fill("Spooky Season Kickoff");
  await page.getByPlaceholder("Tell the members…").fill("Costumes encouraged.");
  await page.getByRole("button", { name: "Post Announcement" }).click();
  await expect(page.locator(".success")).toHaveText("Announcement posted!");

  const feed = await inboxFeed(page);
  const entry = feed.find((n) => n.title === "Spooky Season Kickoff");
  expect(entry?.channel).toBe("announcements");
});

test("an admin broadcast reaches the inbox on the General channel", async ({
  page,
}) => {
  await login(page);
  await page.goto("/admin/broadcast");
  await page.getByPlaceholder("Short headline").fill("Bingo Night");
  await page
    .getByPlaceholder("What do you want people to know?")
    .fill("This Friday at 7.");
  await page.getByRole("button", { name: "Send Broadcast" }).click();
  await expect(page.locator(".success")).toContainText("Broadcast sent");

  const feed = await inboxFeed(page);
  const entry = feed.find((n) => n.title === "Bingo Night");
  expect(entry?.channel).toBe("general");
});

test("group chat sends and shows a message", async ({ page }) => {
  await login(page);
  await page.goto("/chat");

  await expect(page.getByRole("heading", { name: "Group Chat" })).toBeVisible();

  const msg = `Hello from e2e ${Date.now()}`;
  await page.getByPlaceholder("Message the group…").fill(msg);
  await page.getByRole("button", { name: "Send" }).click();

  // The sent message renders in the feed, attributed to the author.
  const bubble = page.locator(".chat-bubble").filter({ hasText: msg });
  await expect(bubble).toBeVisible();
  await expect(page.locator(".chat-msg.mine .chat-author").last()).toHaveText(
    "Root Babe",
  );

  // Chat is push-only — it must not appear in the notifications inbox.
  const feed = await inboxFeed(page);
  expect(feed.some((n) => n.channel === "chat")).toBe(false);
});

test("voting is open while undated and closes once a date is set", async ({
  page,
}) => {
  await login(page);
  await page.goto("/admin/events");

  // A main event with a poll but no date — date is still being voted on.
  await page.getByPlaceholder("Movie title").fill("Undated Pick");
  await page
    .getByPlaceholder("https://rcv123.org/poll/...")
    .first()
    .fill("https://rcv123.org/poll/test");
  await page.getByRole("button", { name: "Create Event" }).click();
  await expect(page.locator(".success")).toHaveText("Event created!");

  // The lifecycle stepper shows where it sits: poll set, date pending.
  const card = page.locator(".thaw-card").filter({ hasText: "Undated Pick" });
  await expect(card.locator(".stage-steps")).toHaveText(
    /posted ✓ · poll ✓ · date — · rsvp —/i,
  );

  // The vote page surfaces the open poll.
  await page.goto("/vote");
  await expect(page.getByText("Voting for:")).toBeVisible();
  await expect(page.getByText("Undated Pick")).toBeVisible();

  // Undated means nothing to RSVP to yet — the members' row hides the button.
  await page.goto("/movie-nights");
  const undatedRow = page.locator(".mn-row").filter({ hasText: "Undated Pick" });
  await expect(undatedRow).toBeVisible();
  await expect(undatedRow.locator(".rsvp-btn")).toHaveCount(0);

  // Close the poll from the card's primary action; picking the date suggests
  // an RSVP deadline of the day before.
  await page.goto("/admin/events");
  await card.getByRole("button", { name: "Close Poll & Set Date" }).click();
  const stageForm = page.locator(".stage-form");
  await stageForm.locator('input[type="date"]').first().fill("2031-12-25");
  await expect(stageForm.locator('input[type="date"]').nth(1)).toHaveValue(
    "2031-12-24",
  );
  await stageForm.getByRole("button", { name: "Save" }).click();

  // The card shows the saved date and a fully-stamped stepper.
  await expect(card).toContainText("2031-12-25");
  await expect(card).toContainText("RSVP by 2031-12-24");
  await expect(card.locator(".stage-steps")).toHaveText(
    /posted ✓ · poll ✓ · date ✓ · rsvp ✓/i,
  );

  await page.goto("/vote");
  await expect(
    page.getByText("No active poll right now. Check back soon!"),
  ).toBeVisible();

  // Now dated, members can RSVP to it.
  await page.goto("/movie-nights");
  await expect(
    page
      .locator(".mn-row")
      .filter({ hasText: "Undated Pick" })
      .locator(".rsvp-btn"),
  ).toBeVisible();
});

test("clearing the inbox empties it", async ({ page }) => {
  await login(page);
  // Prior tests posted announcements/broadcasts, so the feed is non-empty.
  expect((await inboxFeed(page)).length).toBeGreaterThan(0);

  await clearInbox(page);
  expect(await inboxFeed(page)).toHaveLength(0);
});

test("exactly one service worker owns the root scope", async ({ page }) => {
  // Regression guard for the push outage: a second worker registered at "/"
  // (the old firebase-messaging-sw.js) replaces the shell worker and swallows
  // every push event. There must be exactly one registration, and it must be
  // the combined sw.js.
  await login(page);
  const regs = await page.evaluate(async () => {
    await navigator.serviceWorker.ready; // registration happens on window load
    const rs = await navigator.serviceWorker.getRegistrations();
    return rs.map((r) => ({
      scope: r.scope,
      script: (r.active ?? r.installing ?? r.waiting)?.scriptURL ?? "",
    }));
  });
  expect(regs).toHaveLength(1);
  expect(regs[0].scope).toBe("http://localhost:3001/");
  expect(regs[0].script).toBe("http://localhost:3001/sw.js");
});

test("members can send themselves a test notification", async ({ page }) => {
  // With permission granted the profile offers a self-serve test push. In dev
  // push isn't configured, so no device is enrolled — the button must say so
  // instead of pretending it worked. Headless Chromium hard-denies the
  // Notification API (grantPermissions has no effect), so stub the permission
  // the app reads.
  await page.addInitScript(() => {
    Object.defineProperty(Notification, "permission", { get: () => "granted" });
  });
  await login(page);
  await page.goto("/profile");
  await page.getByRole("button", { name: "Send Test Notification" }).click();
  await expect(page.locator(".success")).toContainText("No devices enrolled");
});

test("test broadcasts stay out of the inbox", async ({ page }) => {
  await login(page);
  await page.goto("/admin/broadcast");
  await page.getByPlaceholder("Short headline").fill("Pipeline probe");
  await page
    .getByPlaceholder("What do you want people to know?")
    .fill("Admins only.");
  await page
    .locator(".thaw-switch")
    .filter({ hasText: "Send to Test channel" })
    .getByRole("switch")
    .check();
  await page.getByRole("button", { name: "Send Broadcast" }).click();
  await expect(page.locator(".success")).toContainText(
    "only admins receive it",
  );

  const feed = await inboxFeed(page);
  expect(feed.some((n) => n.title === "Pipeline probe")).toBe(false);
});

test("members see no admin-only test channel controls", async ({ page }) => {
  // Mint a member invite as root…
  await login(page);
  await page.goto("/admin/invites");
  await page.getByPlaceholder("First name").fill("Pugsley");
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");
  const card = page.locator(".thaw-card").filter({ hasText: "Pugsley" });
  const code = ((await card.locator("code").first().textContent()) ?? "").trim();
  expect(code).not.toEqual("");

  // …and onboard the member.
  await page.evaluate(() => localStorage.clear());
  await page.goto(`/login?code=${encodeURIComponent(code)}`);
  const stamp = Date.now();
  await page.fill("#reg-email", `pugsley-${stamp}@e2e.test`);
  await page.fill("#reg-username", `pugsley${stamp}`);
  await page.fill("#reg-password", "member-pw-123");
  await page.click('form button[type="submit"]');
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();

  // Their profile has the seven regular push channels (including Book Club),
  // no test channel, plus the six email channels and the public-profile switch.
  await page.goto("/profile");
  await expect(
    page.getByRole("heading", { name: "Notifications" }),
  ).toBeVisible();
  await expect(
    page.locator(".notif-channels").getByRole("switch"),
  ).toHaveCount(7);
  await expect(
    page.locator(".email-channels").getByRole("switch"),
  ).toHaveCount(6);
  await expect(page.getByRole("switch")).toHaveCount(14);
  await expect(
    page.locator(".thaw-switch").filter({ hasText: "Test Messages (admins only)" }),
  ).toHaveCount(0);

  // And the broadcast tool is off-limits entirely.
  await page.goto("/admin/broadcast");
  await expect(page.locator(".error")).toContainText("Access denied");

  // Leave the session as root for any tests that follow.
  await page.evaluate(() => localStorage.clear());
  await login(page);
});

test("an admin posts a gathering, members RSVP, and names stay admin-only", async ({
  page,
}) => {
  await login(page);
  await page.goto("/gatherings");

  await page.getByRole("button", { name: "Post a Gathering" }).click();
  await page.getByPlaceholder("Bonfire at the quarry").fill("Quarry Bonfire");
  // datetime-local inputs: the form requires a time, not just a date.
  await page.locator('input[type="datetime-local"]').first().fill("2030-09-01T18:30");
  await page
    .getByPlaceholder("905 NW 10th St, Bentonville")
    .fill("The old quarry");
  await page.getByRole("button", { name: "Post Gathering" }).click();

  const card = page.locator(".thaw-card").filter({ hasText: "Quarry Bonfire" });
  await expect(card).toBeVisible();
  await expect(card).toContainText("September 1, 2030");
  await expect(card).toContainText("6:30 PM");
  await expect(card).toContainText("Nobody going yet");

  // RSVP flips the button and the public count.
  await card.locator(".rsvp-btn").click();
  await expect(card.locator(".rsvp-btn")).toContainText("Going");
  await expect(card).toContainText("1 person going");

  // An admin can see who's going.
  await card.getByRole("button", { name: "View RSVPs" }).click();
  await expect(card.locator(".rsvp-names li")).toHaveCount(1);
});

test("a member sees the gathering count but no attendee controls", async ({
  page,
}) => {
  // Mint a member invite as root…
  await login(page);
  await page.goto("/admin/invites");
  // Unique per run: a fixed name matches leftover invites from an earlier run
  // against the same emulator, and the card locator then resolves to two.
  const stamp = Date.now();
  const invitee = `Wednesday${stamp}`;
  await page.getByPlaceholder("First name").fill(invitee);
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");
  // Wait for the card itself before reading out of it: the list re-renders
  // after Generate, and reading too early yields an empty code, which then
  // fails several steps later at "why am I not logged in".
  const inviteCard = page.locator(".thaw-card").filter({ hasText: invitee });
  await expect(inviteCard).toBeVisible();
  const code = ((await inviteCard.locator("code").first().textContent()) ?? "").trim();
  expect(code).not.toEqual("");

  // …and onboard them.
  await page.evaluate(() => localStorage.clear());
  await page.goto(`/login?code=${encodeURIComponent(code)}`);
  await page.fill("#reg-email", `wednesday-${stamp}@e2e.test`);
  await page.fill("#reg-username", `wednesday${stamp}`);
  await page.fill("#reg-password", "member-pw-123");
  await page.click('form button[type="submit"]');
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();

  await page.goto("/gatherings");
  const card = page.locator(".thaw-card").filter({ hasText: "Quarry Bonfire" });
  // The count is public — that's the point of splitting count from names.
  await expect(card).toContainText("1 person going");
  // The admin affordances are not theirs.
  await expect(card.getByRole("button", { name: "View RSVPs" })).toHaveCount(0);
  await expect(card.getByRole("button", { name: "Delete" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Post a Gathering" })).toHaveCount(0);

  // Clean up: back to root, remove the gathering for later serial tests.
  await page.evaluate(() => localStorage.clear());
  await login(page);
  await page.goto("/gatherings");
  const adminCard = page.locator(".thaw-card").filter({ hasText: "Quarry Bonfire" });
  await adminCard.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Quarry Bonfire" }),
  ).toHaveCount(0);
});

test("map buttons don't post the form, and a gathering can be edited", async ({
  page,
}) => {
  await login(page);
  await page.goto("/gatherings");
  await page.getByRole("button", { name: "Post a Gathering" }).click();

  await page.getByPlaceholder("Bonfire at the quarry").fill("Edit Me");
  await page
    .locator('input[type="datetime-local"]')
    .first()
    .fill("2030-10-05T19:00");
  await page.getByPlaceholder("905 NW 10th St, Bentonville").fill("Clubhouse");

  // Regression guard: inside a <form> a bare <button> defaults to submit, so
  // the map helpers used to post the gathering before the admin was finished
  // with it. "Clear pin" is the network-free one of the pair.
  await page.getByRole("button", { name: "Clear pin" }).click();
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Edit Me" }),
  ).toHaveCount(0);

  // Submitting deliberately does post it.
  await page.getByRole("button", { name: "Post Gathering" }).click();
  const card = page.locator(".thaw-card").filter({ hasText: "Edit Me" });
  await expect(card).toBeVisible();
  await expect(card).toContainText("October 5, 2030");

  // Edit prefills the form from the stored gathering.
  await card.getByRole("button", { name: "Edit" }).click();
  await expect(page.getByPlaceholder("Bonfire at the quarry")).toHaveValue(
    "Edit Me",
  );
  await expect(page.getByPlaceholder("905 NW 10th St, Bentonville")).toHaveValue(
    "Clubhouse",
  );

  await page.getByPlaceholder("Bonfire at the quarry").fill("Edited Already");
  await page
    .locator('input[type="datetime-local"]')
    .first()
    .fill("2030-10-06T20:00");
  await page.getByRole("button", { name: "Save Changes" }).click();

  const edited = page.locator(".thaw-card").filter({ hasText: "Edited Already" });
  await expect(edited).toBeVisible();
  await expect(edited).toContainText("October 6, 2030");
  await expect(edited).toContainText("8:00 PM");
  // Edited in place, not posted as a second gathering.
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Edit Me" }),
  ).toHaveCount(0);

  // Clean up for later serial tests.
  await edited.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Edited Already" }),
  ).toHaveCount(0);
});

test("admin builds the book club queue, promotes a book, and a member joins", async ({
  page,
}) => {
  await login(page);
  await page.goto("/admin/book-club");
  await expect(page.getByText("Between books — nothing current.")).toBeVisible();

  await page.getByRole("button", { name: "+ Add to queue" }).click();
  await page.getByPlaceholder("Wide Sargasso Sea").fill("The Bloody Chamber");
  await page.getByPlaceholder("Jean Rhys").fill("Angela Carter");
  await page.getByPlaceholder("1966").fill("1979");
  await page.getByRole("button", { name: "Add to queue" }).click();
  await expect(page.locator(".success")).toContainText("Added to the queue");

  await page.getByRole("button", { name: "Finish & start next" }).click();
  await expect(page.locator(".success")).toContainText("Moved on to the next book");
  await expect(
    page.locator(".mn-title").filter({ hasText: "The Bloody Chamber" }),
  ).toBeVisible();

  // The public page shows it as current, with nobody reading yet.
  await page.goto("/book-club");
  await expect(page.getByText("Now Reading")).toBeVisible();
  await expect(page.locator(".feature-title")).toHaveText("The Bloody Chamber");
  await expect(page.getByText("0 members reading")).toBeVisible();

  // Joining flips the button, the count, and the profile notification channel.
  await page.getByRole("button", { name: "Join the Book Club" }).click();
  await expect(page.getByRole("button", { name: /In the club/ })).toBeVisible();
  await expect(page.getByText("1 member reading")).toBeVisible();

  await page.goto("/profile");
  await expect(
    page
      .locator(".notif-channels .thaw-switch")
      .filter({ hasText: "Book Club" })
      .getByRole("switch"),
  ).toBeChecked();

  // Leaving flips it back off, and doesn't disturb later channel-count tests.
  await page.goto("/book-club");
  await page.getByRole("button", { name: /In the club/ }).click();
  await expect(page.getByRole("button", { name: "Join the Book Club" })).toBeVisible();
  await page.goto("/profile");
  await expect(
    page
      .locator(".notif-channels .thaw-switch")
      .filter({ hasText: "Book Club" })
      .getByRole("switch"),
  ).not.toBeChecked();
});

test("admin sets a meetup and a member RSVPs with their name visible", async ({
  page,
}) => {
  await login(page);
  await page.goto("/admin/book-club");
  await page.locator('input[type="datetime-local"]').fill("2030-10-08T19:00");
  await page.getByPlaceholder("Marisol V.").fill("Marisol V.");
  await page
    .getByPlaceholder("1412 SE C St, Bentonville")
    .fill("1412 SE C St, Bentonville");
  await page.getByRole("button", { name: "Save meetup" }).click();
  await expect(page.locator(".success")).toContainText("Meetup saved");

  await page.goto("/book-club");
  await expect(page.getByText("Hosted by Marisol V.")).toBeVisible();
  await expect(page.getByText("Nobody going yet")).toBeVisible();

  await page.getByRole("button", { name: "RSVP — I'm going" }).click();
  await expect(page.getByRole("button", { name: "Going ✓" })).toBeVisible();
  await expect(page.getByText("1 person going")).toBeVisible();
  await expect(page.getByText("Root Babe")).toBeVisible();
});

test("admin reorders and removes book club queue entries", async ({ page }) => {
  await login(page);
  await page.goto("/admin/book-club");

  await page.getByRole("button", { name: "+ Add to queue" }).click();
  await page.getByPlaceholder("Wide Sargasso Sea").fill("Beloved");
  await page.getByPlaceholder("Jean Rhys").fill("Toni Morrison");
  await page.getByRole("button", { name: "Add to queue" }).click();
  await expect(page.locator(".success")).toContainText("Added to the queue");

  await page.getByRole("button", { name: "+ Add to queue" }).click();
  await page.getByPlaceholder("Wide Sargasso Sea").fill("Piranesi");
  await page.getByPlaceholder("Jean Rhys").fill("Susanna Clarke");
  await page.getByRole("button", { name: "Add to queue" }).click();
  await expect(page.locator(".success")).toContainText("Added to the queue");

  // Beloved was queued first; move Piranesi up one slot to swap them.
  await page
    .locator(".mn-row")
    .filter({ hasText: "Piranesi" })
    .getByRole("button", { name: "↑" })
    .click();
  const rows = page.locator(".mn-row").filter({ hasText: /Beloved|Piranesi/ });
  await expect(rows.first()).toContainText("Piranesi");

  // Remove Beloved from the queue entirely.
  await page
    .locator(".mn-row")
    .filter({ hasText: "Beloved" })
    .getByRole("button", { name: "×" })
    .click();
  await expect(page.locator(".mn-row").filter({ hasText: "Beloved" })).toHaveCount(0);
});

test("a scoped Book Club Manager reaches only the Book Club admin panel", async ({
  page,
  browser,
}) => {
  // Root stays logged into `page` for the whole test — a second browser
  // context carries the manager's separate session, so this doesn't need to
  // repeatedly log root out and back in (each login is a real POST /auth/*,
  // and the suite runs serially from one IP; churning through many rapid
  // logins here was tripping the auth rate limiter).
  await login(page);
  await page.goto("/admin/invites");
  const stamp = Date.now();
  const invitee = `Manager${stamp}`;
  await page.getByPlaceholder("First name").fill(invitee);
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.locator(".success")).toContainText("created and copied");
  const inviteCard = page.locator(".thaw-card").filter({ hasText: invitee });
  await expect(inviteCard).toBeVisible();
  const code = ((await inviteCard.locator("code").first().textContent()) ?? "").trim();
  expect(code).not.toEqual("");

  const managerCtx = await browser.newContext();
  const mgr = await managerCtx.newPage();
  const username = `manager${stamp}`;
  const email = `manager-${stamp}@e2e.test`;

  async function loginAsManager() {
    await mgr.goto("/login");
    await mgr.fill("#login-email", email);
    await mgr.fill("#login-password", "member-pw-123");
    await mgr.click('form button[type="submit"]');
    await expect(mgr.getByRole("button", { name: "Logout" })).toBeVisible();
  }

  // Onboard the manager via the invite link (auto-logs them in).
  await mgr.goto(`/login?code=${encodeURIComponent(code)}`);
  await mgr.fill("#reg-email", email);
  await mgr.fill("#reg-username", username);
  await mgr.fill("#reg-password", "member-pw-123");
  await mgr.click('form button[type="submit"]');
  await expect(mgr.getByRole("button", { name: "Logout" })).toBeVisible();

  // Before any grant: no Admin link, and the admin routes refuse them.
  await expect(mgr.getByRole("link", { name: "Admin" })).toHaveCount(0);
  await mgr.goto("/admin/book-club");
  await expect(mgr.getByText("Access denied.")).toBeVisible();

  // Root grants the scoped permission from the Roles page.
  await page.goto("/admin/roles");
  const row = page.locator(".thaw-card").filter({ hasText: username });
  await expect(row).toBeVisible();
  await row.getByRole("switch").click();

  // The permission only lands in the manager's own session on their next
  // login (their cached AuthUser doesn't self-refresh, same as a role change
  // wouldn't) — the member now reaches the Book Club panel, and only that one.
  await loginAsManager();
  const adminLink = mgr.getByRole("link", { name: "Admin" });
  await expect(adminLink).toBeVisible();
  await expect(adminLink).toHaveAttribute("href", "/admin/book-club");
  await adminLink.click();
  await expect(mgr.getByRole("heading", { name: "Book Club" })).toBeVisible();
  // Only the Book Club tab shows — no Announcements/Events/Invites/Roles.
  await expect(mgr.locator(".admin-tabs").getByText("Announcements")).toHaveCount(0);
  await expect(mgr.locator(".admin-tabs").getByText("Roles")).toHaveCount(0);

  // Still no access to full-admin-only areas.
  await mgr.goto("/admin/events");
  await expect(mgr.getByText("Access denied.")).toBeVisible();
  await mgr.goto("/admin/roles");
  await expect(mgr.getByText("Access denied.")).toBeVisible();

  // The permission is functional, not just a nav affordance.
  await mgr.goto("/admin/book-club");
  await mgr.getByRole("button", { name: "+ Add to queue" }).click();
  await mgr.getByPlaceholder("Wide Sargasso Sea").fill("Kindred");
  await mgr.getByPlaceholder("Jean Rhys").fill("Octavia Butler");
  await mgr.getByRole("button", { name: "Add to queue" }).click();
  await expect(mgr.locator(".success")).toContainText("Added to the queue");

  // Root revokes it (still the same root session — no relogin needed); the
  // member loses access again on their next login.
  await row.getByRole("switch").click();
  await expect(row.getByRole("switch")).not.toBeChecked();

  await loginAsManager();
  await expect(mgr.getByRole("link", { name: "Admin" })).toHaveCount(0);
  await mgr.goto("/admin/book-club");
  await expect(mgr.getByText("Access denied.")).toBeVisible();

  await managerCtx.close();
});
