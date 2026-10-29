import { expect, test, type Page } from "@playwright/test";

test.use({ viewport: { width: 390, height: 844 } });

async function settle(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForFunction(() => [...document.images].every((image) => image.complete));
}

test("pairing screen", async ({ page }) => {
  await page.goto("/visual.html?screen=pairing");
  await expect(page.getByRole("heading", { name: /From desktop/ })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("pairing.png");
});

test("library grid", async ({ page }) => {
  await page.goto("/visual.html");
  await expect(page.getByText("589 projects")).toBeVisible();
  await expect(page.getByText("Modular Drill Dock")).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("library.png");
});

test("connection error offers a clear retry", async ({ page }) => {
  await page.goto("/visual.html?connection=offline");
  await expect(page.getByText("Could not connect to Volum Desktop")).toBeVisible();
  await expect(page.getByText("Check that Tailscale is connected on both devices, Volum Desktop is open, and Companion Access is on.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("connection-error.png");
});

test("library filter sheet", async ({ page }) => {
  await page.goto("/visual.html");
  await expect(page.getByText("589 projects")).toBeVisible();
  await page.getByRole("button", { name: "Filter and sort" }).click();
  await expect(page.getByRole("heading", { name: "Filter & sort" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("library-filters.png");
});

test("project QR explanation", async ({ page }) => {
  await page.goto("/visual.html");
  await expect(page.getByText("589 projects")).toBeVisible();
  await page.getByRole("button", { name: "Open project scanner" }).click();
  await expect(page.getByRole("heading", { name: "Open a shared project" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("project-qr-explainer.png");
});

test("project scanner can always be cancelled", async ({ page }) => {
  await page.goto("/visual.html");
  await expect(page.getByText("589 projects")).toBeVisible();
  await page.getByRole("button", { name: "Open project scanner" }).click();
  await page.getByRole("button", { name: "Scan project QR" }).click();
  await expect(page.getByRole("heading", { name: "Scan a project QR" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel scan" }).first().click();
  await expect(page.getByRole("heading", { name: "Library" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Scan a project QR" })).not.toBeVisible();
});

test("pairing scanner can always be cancelled", async ({ page }) => {
  await page.goto("/visual.html?screen=pairing");
  await page.getByRole("button", { name: "Scan desktop code" }).click();
  await expect(page.getByRole("heading", { name: "Scan the pairing code" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel scan" }).first().click();
  await expect(page.getByRole("heading", { name: /From desktop/ })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Scan the pairing code" })).not.toBeVisible();
});

test("project detail", async ({ page }) => {
  await page.goto("/visual.html");
  await page.getByRole("button", { name: /Modular Drill Dock/ }).click();
  await expect(page.getByRole("heading", { name: "Modular Drill Dock" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("project-detail.png");
});

test("Catalan settings", async ({ page }) => {
  await page.goto("/visual.html?language=ca");
  await expect(page.getByText("589 projectes")).toBeVisible();
  await page.getByRole("button", { name: "Configuració" }).click();
  await expect(page.getByRole("heading", { name: "Configuració" })).toBeVisible();
  await expect(page.locator(".mobile-settings-card").filter({ hasText: "Idioma" }).locator(".lucide-languages")).toBeVisible();
  await expect(page.getByText("Permisos de l'app", { exact: true })).toBeVisible();
  const cardTitles = await page.locator(".mobile-settings-card strong").allTextContents();
  expect(cardTitles.indexOf("Privat per disseny")).toBeGreaterThan(cardTitles.indexOf("Memòria cau multimèdia recent"));
  expect(cardTitles.indexOf("Pujades de fitxers segures")).toBeGreaterThan(cardTitles.indexOf("Privat per disseny"));
  await settle(page);
  await expect(page).toHaveScreenshot("settings-ca.png");
});
