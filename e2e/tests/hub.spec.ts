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
  await page
    .getByPlaceholder("A few words about yourself…")
    .fill("Founding member. Crafts, cosmos, and cinema.");
  await page.getByRole("switch").check();

  await page.getByRole("button", { name: "Save Profile" }).click();
  await expect(page.locator(".success")).toHaveText("Profile saved.");

  // Saved values survive a reload.
  await page.reload();
  await expect(
    page.getByPlaceholder("Leave blank to use username"),
  ).toHaveValue("Root Babe");
  await expect(page.getByRole("switch")).toBeChecked();
});

test("published profile appears in the member directory", async ({ page }) => {
  await login(page);
  await page.goto("/members");

  const card = page.getByText("Root Babe");
  await expect(card).toBeVisible();
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
  await page.locator('input[type="date"]').fill("2030-10-31");
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
