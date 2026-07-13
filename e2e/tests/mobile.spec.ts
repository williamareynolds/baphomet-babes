import { test, expect, type Page } from "@playwright/test";

// Runs under the `mobile-safari` project: Playwright's WebKit engine (the same
// engine as iOS Safari) at an iPhone 14 Pro Max viewport. This is where our
// layout bugs actually surface — Chromium consistently failed to reproduce the
// horizontal-overflow issues real iOS users hit. The functional behaviour is
// covered by hub.spec.ts on Chromium; here we only assert the layout fits.
//
// The bootstrap superadmin is registered by the `chromium` project, which this
// project depends on, so the credentials below already exist by the time we log
// in. Keep these in sync with hub.spec.ts / playwright.config.ts.
const EMAIL = "root@e2e.test";
const PASSWORD = "sup3r-secret-pw";

async function login(page: Page) {
  await page.goto("/login");
  await page.fill("#login-email", EMAIL);
  await page.fill("#login-password", PASSWORD);
  await page.click('form button[type="submit"]');
  await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();
  const dismiss = page.getByRole("button", { name: "Dismiss" });
  if (await dismiss.isVisible().catch(() => false)) await dismiss.click();
}

/// Return a description of every element whose box spills past the viewport's
/// right edge — i.e. content the user can't reach (the body clips overflow, so
/// there's no scroll). A correctly-bounded page returns an empty list.
async function horizontalOverflow(page: Page): Promise<string[]> {
  // Let fonts settle — a fallback face can be wider and skew widths.
  await page.evaluate(() => (document as any).fonts?.ready);
  return page.evaluate(() => {
    const docW = document.documentElement.clientWidth;
    // An element past the edge isn't the bug we're hunting if some ancestor
    // bounds it horizontally: overflow-x:auto/scroll means the user can scroll
    // to it (e.g. the admin tab strip), and overflow-x:hidden means it's clipped
    // and unreachable (e.g. Leaflet's oversized internal tile container inside
    // .leaflet-container) — either way it can't push the page into a horizontal
    // scroll, which is what we actually guard against.
    const bounded = (el: Element): boolean => {
      let p = el.parentElement;
      while (p && p !== document.body) {
        const ox = getComputedStyle(p).overflowX;
        if (ox === "auto" || ox === "scroll" || ox === "hidden") return true;
        p = p.parentElement;
      }
      return false;
    };
    const out: string[] = [];
    for (const el of Array.from(document.querySelectorAll("body *"))) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue; // hidden
      if (bounded(el)) continue;
      // 1px slack for sub-pixel rounding.
      if (r.right > docW + 1) {
        const cls = (el.className || "").toString().trim().slice(0, 40);
        out.push(
          `<${el.tagName.toLowerCase()}${cls ? "." + cls : ""}> ` +
            `right=${Math.round(r.right)} width=${Math.round(r.width)} (viewport=${docW})`,
        );
      }
    }
    return out;
  });
}

async function expectNoOverflow(page: Page) {
  const offenders = await horizontalOverflow(page);
  expect(offenders, `elements overflow the viewport:\n${offenders.join("\n")}`).toEqual([]);
}

test("login page fits the iPhone viewport", async ({ page }) => {
  await page.goto("/login");
  await expect(page.locator("#login-email")).toBeVisible();
  await expectNoOverflow(page);
});

test("form fields are at least 16px so iOS never auto-zooms", async ({
  page,
}) => {
  // iOS zooms the page when a focused field's font-size is under 16px, and in
  // the installed PWA the zoom sticks after blur — the page opens "zoomed in"
  // with the menu off-screen. The iPhone device profile has a coarse pointer,
  // so the touch-only 16px override applies here.
  await page.goto("/login");
  await page.getByRole("button", { name: "Register" }).click();
  await expect(page.locator("#reg-invite")).toBeVisible();
  const tooSmall = await page.evaluate(() =>
    Array.from(document.querySelectorAll("input, select, textarea"))
      .map((el) => ({
        id: el.id || el.tagName.toLowerCase(),
        size: parseFloat(getComputedStyle(el).fontSize),
      }))
      .filter((f) => f.size < 16),
  );
  expect(
    tooSmall,
    `fields under 16px trigger the iOS focus zoom: ${JSON.stringify(tooSmall)}`,
  ).toEqual([]);
});

test("home page fits the iPhone viewport", async ({ page }) => {
  await login(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Baphomet Babes" })).toBeVisible();
  await expectNoOverflow(page);
});

test("about page fits the iPhone viewport", async ({ page }) => {
  await login(page);
  await page.goto("/about");
  await expectNoOverflow(page);
});

test("movie nights page fits the iPhone viewport", async ({ page }) => {
  await login(page);
  await page.goto("/movie-nights");
  await expectNoOverflow(page);
});

test("rides page fits the iPhone viewport", async ({ page }) => {
  await login(page);
  await page.goto("/rides");
  await expect(page.getByRole("heading", { name: "Mountain Bike Rides" })).toBeVisible();
  await expectNoOverflow(page);
});

test("admin users page fits the iPhone viewport", async ({ page }) => {
  await login(page);
  await page.goto("/admin/users");
  await expect(page.getByRole("heading", { name: "Admin" })).toBeVisible();
  await expectNoOverflow(page);
});
