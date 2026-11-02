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

test("pairing address review explains private HTTP", async ({ page }) => {
  await page.goto("/visual.html?screen=pairing");
  await page.getByRole("button", { name: "Enter pairing data manually" }).click();
  await page.getByLabel("Pairing data").fill(JSON.stringify({
    version: 1,
    serverId: "visual-server",
    serverName: "Workshop Mac",
    baseUrl: "http://100.64.0.20:43821",
    secret: "visual-one-use-secret",
    expiresAt: "2999-01-01T00:00:00Z",
    transport: "tailscale-http",
  }));
  await page.getByRole("button", { name: "Review connection" }).click();
  await expect(page.getByText("Verify the address before connecting")).toBeVisible();
  await page.getByLabel("Server address").fill("http://192.168.1.20:43821");
  await expect(page.getByText("Trust this private HTTP connection")).toBeVisible();
  await expect(page.getByRole("button", { name: "Verify and connect" })).toBeDisabled();
  await settle(page);
  await expect(page).toHaveScreenshot("pairing-private-http.png");
});

test("demo library opens without pairing", async ({ page }) => {
  await page.goto("/visual.html?screen=pairing");
  await page.getByRole("button", { name: "Explore the demo library" }).click();
  await expect(page.getByText("589 projects")).toBeVisible();
  await expect(page.getByText("DEMO LIBRARY")).toHaveCount(0);
});

test("library grid", async ({ page }) => {
  await page.goto("/visual.html");
  await expect(page.getByText("589 projects")).toBeVisible();
  await expect(page.getByText("Modular Drill Dock")).toBeVisible();
  await expect(page.locator(".mobile-header .brand")).toHaveCount(0);
  await settle(page);
  await expect(page).toHaveScreenshot("library.png");
});

test("library uses a fallback for missing previews", async ({ page }) => {
  await page.goto("/visual.html?media=missing");
  await expect(page.getByText("589 projects")).toBeVisible();
  await expect(page.getByText("No preview").first()).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("library-missing-previews.png");
});

test("connection error offers a clear retry", async ({ page }) => {
  await page.goto("/visual.html?connection=offline");
  await expect(page.getByText("Could not connect to Volum Desktop")).toBeVisible();
  await expect(page.getByText("Check that the selected private network or HTTPS address is reachable, Volum Desktop is open, and Companion Access is on.")).toBeVisible();
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
  await expect(page.getByRole("button", { name: "Photo Room" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("project-detail.png");
});

test("project detail opens Photo Room directly", async ({ page }) => {
  await page.goto("/visual.html");
  await page.getByRole("button", { name: /Modular Drill Dock/ }).click();
  await page.getByRole("button", { name: "Photo Room" }).click();
  await expect(page.getByText("PHOTO ROOM", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Set the scene" })).toBeVisible();
  await expect(page.getByText("Preparing 3D model…")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Format", exact: true })).toBeVisible();
});

test("Desktop-rendered plate objects can be inspected", async ({ page }) => {
  await page.goto("/visual.html");
  await page.getByRole("button", { name: /Modular Drill Dock/ }).click();
  await page.getByRole("button", { name: "View in 3D" }).click();
  await expect(page.getByText("Preparing 3D model…")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Open object list" })).toContainText("2 objects");
  await page.locator(".fullscreen-viewer canvas").click({ position: { x: 145, y: 365 } });
  await expect(page.getByText(/OBJECT [12] OF 2/)).toBeVisible();
  await page.getByRole("button", { name: "Clear object selection" }).click();
  await page.getByRole("button", { name: "Open object list" }).click();
  await expect(page.getByText("PLATE OBJECTS")).toBeVisible();
  await page.getByRole("button", { name: /Object 2/ }).click();
  await expect(page.getByText("OBJECT 2 OF 2")).toBeVisible();
  await page.getByRole("button", { name: "Inspect alone" }).click();
  await expect(page.getByRole("button", { name: "Show all" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("project-viewer-object-isolated.png");
});

test("cached previews survive detail navigation and failed revalidation", async ({ page }) => {
  await page.goto("/visual.html?media=fail-after-first");
  const libraryImages = page.locator(".project-grid .project-image > img");
  await expect(libraryImages).toHaveCount(4);
  await expect(libraryImages.first()).toBeVisible();

  await page.getByRole("button", { name: /Modular Drill Dock/ }).click();
  await expect(page.getByRole("heading", { name: "Modular Drill Dock" })).toBeVisible();
  await expect(page.locator(".detail-stage .project-image > img")).toBeVisible();
  await expect(page.getByText("No preview available")).toHaveCount(0);

  await page.getByRole("button", { name: "Back to library" }).click();
  await expect(page.getByRole("heading", { name: "Library" })).toBeVisible();
  await expect(libraryImages).toHaveCount(4);
  await expect(libraryImages.first()).toBeVisible();
  await expect(page.getByText("No preview")).toHaveCount(0);
});

test("project detail explains a missing preview", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await page.goto("/visual.html?media=missing");
  await page.getByRole("button", { name: /Modular Drill Dock/ }).click();
  await expect(page.getByText("No preview available")).toBeVisible();
  await expect(page.getByText("Open the 3D viewer to explore this file")).toBeVisible();
  await expect(page.getByRole("button", { name: "View in 3D" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("project-detail-missing-preview.png");
});

test("capture mode has an unmistakable selected state", async ({ page }) => {
  await page.goto("/visual.html");
  await page.getByRole("button", { name: "Capture" }).click();
  await expect(page.getByText("WORKSHOP CAPTURE")).toHaveCount(0);
  await expect(page.locator(".mobile-header .brand")).toHaveCount(0);
  const photo = page.getByRole("tab", { name: "Photo" });
  const file = page.getByRole("tab", { name: "STL file" });
  await expect(photo).toHaveAttribute("aria-selected", "true");
  await expect(photo).toHaveClass(/is-active/);
  await file.click();
  await expect(file).toHaveAttribute("aria-selected", "true");
  await expect(file).toHaveClass(/is-active/);
  await expect(photo).toHaveAttribute("aria-selected", "false");
  await settle(page);
  await expect(page).toHaveScreenshot("capture-file-selected.png");
});

test("local STL preview stays on device until save", async ({ page }) => {
  await page.goto("/visual.html?screen=local-stl");
  await expect(page.getByRole("heading", { name: "workbench-bracket" })).toBeVisible();
  await expect(page.getByText("40 mm × 30 mm × 20 mm")).toBeVisible();
  await expect(page.getByText("Nothing is uploaded until you tap Save to Volum.")).toBeVisible();
  await expect(page.getByText("Opening 3D viewer…")).toHaveCount(0);
  await expect(page.getByText("Preparing 3D model…")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Open object list" })).toContainText("2 objects");
  await settle(page);
  await expect(page).toHaveScreenshot("local-stl-preview.png");

  await page.locator(".local-stl-stage canvas").click({ position: { x: 165, y: 210 } });
  await expect(page.getByText("OBJECT 1 OF 2")).toBeVisible();
  await page.getByRole("button", { name: "Clear object selection" }).click();

  await page.getByRole("button", { name: "Open object list" }).click();
  await expect(page.getByRole("heading", { name: "Inspect individual objects" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("local-stl-objects.png");
  await page.getByRole("button", { name: /Object 2/ }).click();
  await expect(page.getByText("OBJECT 2 OF 2")).toBeVisible();
  await expect(page.getByText("12 mm", { exact: true })).toHaveCount(3);
  await page.getByRole("button", { name: "Inspect alone" }).click();
  await expect(page.getByRole("button", { name: "Show all" })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("local-stl-object-isolated.png");
  await page.getByRole("button", { name: "Show all" }).click();

  await page.getByRole("button", { name: "Save to Volum" }).click();
  await expect(page.getByRole("heading", { name: "Choose a destination" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "New project" })).toHaveAttribute("aria-selected", "true");
  await settle(page);
  await expect(page).toHaveScreenshot("local-stl-save.png");
});

test("local STL opens in Photo Room", async ({ page }) => {
  await page.goto("/visual.html?screen=local-stl");
  await page.getByRole("button", { name: "Open Photo Room" }).click();
  await expect(page.getByText("PHOTO ROOM", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Set the scene" })).toBeVisible();
  await expect(page.getByText("Preparing 3D model…")).toHaveCount(0);
  await page.getByRole("button", { name: "Format", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Choose a format" })).toBeVisible();
  await page.getByRole("button", { name: /Reel \/ Story/ }).click();
  const reelCanvas = page.locator(".photo-room__model canvas");
  const renderedBounds = await reelCanvas.evaluate((node) => {
    const canvas = node as HTMLCanvasElement;
    const gl = canvas.getContext("webgl2") ?? canvas.getContext("webgl");
    if (!gl) return null;
    const pixels = new Uint8Array(canvas.width * canvas.height * 4);
    gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    let minX = canvas.width;
    let maxX = -1;
    for (let pixel = 0; pixel < canvas.width * canvas.height; pixel += 1) {
      if (pixels[pixel * 4 + 3] < 8) continue;
      const x = pixel % canvas.width;
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
    }
    return maxX < 0 ? null : { minX, maxX, width: canvas.width };
  });
  expect(renderedBounds).not.toBeNull();
  expect(renderedBounds!.minX).toBeGreaterThan(renderedBounds!.width * 0.02);
  expect(renderedBounds!.maxX).toBeLessThan(renderedBounds!.width * 0.98);
  await page.getByRole("button", { name: "Model", exact: true }).click();
  await expect(page.getByRole("button", { name: "Turntable" })).toHaveClass(/is-active/);
  const constrainedRotation = page.getByRole("slider", { name: "Rotate model horizontally" });
  await constrainedRotation.press("ArrowRight");
  await expect(page.getByRole("slider", { name: "Model turn angle" })).toHaveValue("5");
  await page.getByRole("button", { name: "Free", exact: true }).click();
  const freeRotation = page.getByRole("slider", { name: "Rotate model freely" });
  await freeRotation.press("ArrowDown");
  await expect(page.getByRole("slider", { name: "Model tilt angle" })).toHaveValue("5");
  await page.getByRole("button", { name: "Text", exact: true }).click();
  await page.getByRole("button", { name: "Add text" }).click();
  const text = page.locator(".photo-room__text");
  await expect(text).toContainText("Your text");
  const rotateHandle = page.getByRole("button", { name: "Rotate text" });
  const resizeHandle = page.getByRole("button", { name: "Resize text" });
  await expect(rotateHandle).toBeVisible();
  await expect(resizeHandle).toBeVisible();
  const stageBounds = await page.locator(".photo-room__stage").boundingBox();
  const rotateBounds = await rotateHandle.boundingBox();
  expect(stageBounds).not.toBeNull();
  expect(rotateBounds).not.toBeNull();
  const textCenter = { x: stageBounds!.x + stageBounds!.width * 0.5, y: stageBounds!.y + stageBounds!.height * 0.16 };
  await page.mouse.move(rotateBounds!.x + rotateBounds!.width / 2, rotateBounds!.y + rotateBounds!.height / 2);
  await page.mouse.down();
  await page.mouse.move(textCenter.x + 60, textCenter.y, { steps: 4 });
  await page.mouse.up();
  expect(Math.abs(Number(await page.getByRole("slider", { name: "Text angle" }).inputValue()))).toBeGreaterThan(70);
  const resizeBounds = await resizeHandle.boundingBox();
  expect(resizeBounds).not.toBeNull();
  const resizeCenter = { x: resizeBounds!.x + resizeBounds!.width / 2, y: resizeBounds!.y + resizeBounds!.height / 2 };
  const resizeVector = { x: resizeCenter.x - textCenter.x, y: resizeCenter.y - textCenter.y };
  const resizeLength = Math.hypot(resizeVector.x, resizeVector.y);
  await page.mouse.move(resizeCenter.x, resizeCenter.y);
  await page.mouse.down();
  await page.mouse.move(resizeCenter.x + resizeVector.x / resizeLength * 50, resizeCenter.y + resizeVector.y / resizeLength * 50, { steps: 4 });
  await page.mouse.up();
  expect(Number(await page.getByRole("slider", { name: "Text size" }).inputValue())).toBeGreaterThan(100);
  await page.getByRole("button", { name: "Glow", exact: true }).click();
  await expect(text).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await page.getByRole("slider", { name: "Text angle" }).fill("27");
  await expect(page.getByRole("slider", { name: "Text angle" })).toHaveValue("27");
  await text.dblclick();
  await expect(page.getByRole("textbox", { name: "Text content" })).toBeFocused();
  await page.getByRole("button", { name: "Motion", exact: true }).click();
  await expect(page.getByRole("button", { name: /Spin/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Flip/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Roll/ })).toBeVisible();
  await page.getByRole("button", { name: /Tumble/ }).click();
  await expect(page.getByRole("option", { name: "Forward" })).toBeAttached();
  await expect(page.getByRole("option", { name: "Reverse" })).toBeAttached();
  await page.getByRole("button", { name: "Preview tumble" }).click();
  await expect(page.locator(".photo-room__animation-badge")).toContainText("Tumble");
  await page.getByRole("button", { name: "Stop preview" }).click();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(page.getByText("Tumble video", { exact: true })).toBeVisible();
  await expect(page.getByText("Silent H.264 MP4", { exact: true })).toBeVisible();
  await expect(page.getByText("1080p", { exact: true })).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("photo-room-export.png");
  await page.evaluate(() => {
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async (data: ShareData) => {
        const file = data.files?.[0];
        if (!file) throw new Error("The share payload did not contain a file.");
        const bytes = new Uint8Array(await file.arrayBuffer());
        document.documentElement.dataset.sharedName = file.name;
        document.documentElement.dataset.sharedType = file.type;
        document.documentElement.dataset.sharedSize = String(file.size);
        document.documentElement.dataset.sharedSignature = String.fromCharCode(...bytes.slice(4, 8));
        if (file.type === "image/jpeg") document.documentElement.dataset.photoShared = "true";
      },
    });
    Object.defineProperty(navigator, "canShare", { configurable: true, value: () => true });
  });
  await page.getByRole("button", { name: "Share photo" }).click();
  await expect(page.getByText(/workbench-bracket-story\.jpg/)).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-photo-shared", "true");
  await expect(page.getByRole("button", { name: "Share again" })).toBeVisible();
  await page.getByRole("button", { name: "Render MP4" }).click();
  await expect(page.getByText("workbench-bracket-story.mp4", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "Share again" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-shared-name", "workbench-bracket-story.mp4");
  await expect(page.locator("html")).toHaveAttribute("data-shared-type", "video/mp4");
  await expect(page.locator("html")).toHaveAttribute("data-shared-signature", "ftyp");
  await expect.poll(() => page.locator("html").getAttribute("data-shared-size").then(Number)).toBeGreaterThan(0);
});

test("collections header has no redundant branding", async ({ page }) => {
  await page.goto("/visual.html");
  await page.getByRole("button", { name: "Collections" }).click();
  await expect(page.getByRole("heading", { name: "Collections" })).toBeVisible();
  await expect(page.getByText("ORGANIZE", { exact: true })).toHaveCount(0);
  await expect(page.locator(".mobile-header .brand")).toHaveCount(0);
  await settle(page);
  await expect(page).toHaveScreenshot("collections.png");
});

test("settings tests the connection and shows the app version", async ({ page }) => {
  await page.goto("/visual.html");
  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.locator(".mobile-header .brand")).toHaveCount(0);
  await page.getByRole("button", { name: "Test connection" }).click();
  await expect(page.getByText("Connected to Workshop Mac")).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("settings-connection.png");

  const version = page.getByText("v1.0.0 · Protocol 1");
  await version.scrollIntoViewIfNeeded();
  await expect(version).toBeVisible();
  await settle(page);
  await expect(page).toHaveScreenshot("settings-about.png");
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
