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
    // An element legitimately past the edge if it lives inside a horizontally
    // scrollable container (e.g. the admin tab strip is overflow-x:auto by
    // design) — the user can scroll to it, so it's not the bug we're hunting.
    const inScrollable = (el: Element): boolean => {
      let p = el.parentElement;
      while (p && p !== document.body) {
        const ox = getComputedStyle(p).overflowX;
        if (ox === "auto" || ox === "scroll") return true;
        p = p.parentElement;
      }
      return false;
    };
    const out: string[] = [];
    for (const el of Array.from(document.querySelectorAll("body *"))) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue; // hidden
      if (inScrollable(el)) continue;
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

test("admin users page fits the iPhone viewport", async ({ page }) => {
  await login(page);
  await page.goto("/admin/users");
  await expect(page.getByRole("heading", { name: "Admin" })).toBeVisible();
  await expectNoOverflow(page);
});
