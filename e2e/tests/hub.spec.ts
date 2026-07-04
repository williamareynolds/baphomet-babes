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

test("register the bootstrap superadmin", async ({ page }) => {
  await page.goto("/login");
  await page.getByRole("button", { name: "Request Entry" }).click();

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

  // Create an event.
  await page.getByPlaceholder("Movie title").fill("The Crow");
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

  // The featured screening also appears in the full archive list below.
  await expect(
    page.locator(".mn-row").filter({ hasText: "The Crow (1994)" }),
  ).toBeVisible();
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

  // Post a ride: pick a trail and a time window.
  await page.locator("select").selectOption("Coler");
  await page.locator('input[type="datetime-local"]').nth(0).fill("2031-06-01T09:00");
  await page.locator('input[type="datetime-local"]').nth(1).fill("2031-06-01T11:00");
  await page.getByRole("button", { name: "Post Ride" }).click();
  await expect(page.locator(".success")).toContainText("Ride posted");

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
  const mtb = page
    .locator(".thaw-switch")
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
      .locator(".thaw-switch")
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

test("profile exposes notification settings", async ({ page }) => {
  await login(page);
  await page.goto("/profile");
  await expect(
    page.getByRole("heading", { name: "Notifications" }),
  ).toBeVisible();
  // is_public switch + five channel switches + the admin-only test channel
  // (root is a superadmin).
  await expect(page.getByRole("switch")).toHaveCount(7);
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

  await page.goto("/notifications");
  const card = page
    .locator(".thaw-card")
    .filter({ hasText: "Spooky Season Kickoff" });
  await expect(card).toBeVisible();
  await expect(card.locator(".badge-announcements")).toHaveText("Announcement");
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

  await page.goto("/notifications");
  const card = page.locator(".thaw-card").filter({ hasText: "Bingo Night" });
  await expect(card).toBeVisible();
  await expect(card.locator(".badge-general")).toHaveText("General");
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
  await page.goto("/notifications");
  await expect(page.locator(".badge-chat")).toHaveCount(0);
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

  // The vote page surfaces the open poll.
  await page.goto("/vote");
  await expect(page.getByText("Voting for:")).toBeVisible();
  await expect(page.getByText("Undated Pick")).toBeVisible();

  // Set a date on it → the poll closes. (Clicking Edit swaps the card body for
  // the edit form, so target the form by its #edit-title field.)
  await page.goto("/admin/events");
  const card = page.locator(".thaw-card").filter({ hasText: "Undated Pick" });
  await card.getByRole("button", { name: "Edit" }).click();
  const editForm = page.locator('form:has(#edit-title)');
  await editForm.locator('input[type="date"]').first().fill("2031-12-25");
  await editForm.getByRole("button", { name: "Save" }).click();

  // The card swaps back to a display row showing the saved date.
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Undated Pick" }),
  ).toContainText("2031-12-25");

  await page.goto("/vote");
  await expect(
    page.getByText("No active poll right now. Check back soon!"),
  ).toBeVisible();
});

test("clearing the inbox empties it", async ({ page }) => {
  await login(page);
  await page.goto("/notifications");
  // Prior tests posted announcements/broadcasts, so the feed is non-empty.
  await expect(page.locator(".thaw-card").first()).toBeVisible();

  await page.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByText("No notifications yet.")).toBeVisible();
  await expect(page.locator(".thaw-card")).toHaveCount(0);
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

  await page.goto("/notifications");
  await expect(
    page.locator(".thaw-card").filter({ hasText: "Pipeline probe" }),
  ).toHaveCount(0);
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

  // Their profile has the six regular switches, no test channel.
  await page.goto("/profile");
  await expect(
    page.getByRole("heading", { name: "Notifications" }),
  ).toBeVisible();
  await expect(page.getByRole("switch")).toHaveCount(6);
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
