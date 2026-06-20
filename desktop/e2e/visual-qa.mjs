import { chromium } from "playwright";

const browser = await chromium.launch({
  headless: true,
  executablePath: "C:/Program Files/Google/Chrome/Application/chrome.exe",
});

async function runViewport(name, width, height) {
  const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  const consoleMessages = [];
  const pageErrors = [];
  const failedRequests = [];
  page.on("console", (msg) => {
    if (["error", "warning"].includes(msg.type())) {
      consoleMessages.push({ type: msg.type(), text: msg.text() });
    }
  });
  page.on("pageerror", (err) => pageErrors.push(String(err.stack || err.message || err)));
  page.on("requestfailed", (req) => {
    failedRequests.push({ url: req.url(), failure: req.failure()?.errorText });
  });
  const response = await page.goto("http://127.0.0.1:1420", {
    waitUntil: "networkidle",
    timeout: 30000,
  });
  await page.screenshot({ path: `../qa-${name}.png`, fullPage: true });
  const metrics = await page.evaluate(() => {
    const body = document.body;
    const root = document.querySelector("#root");
    const overflow = [...document.querySelectorAll("body *")]
      .map((el) => {
        const r = el.getBoundingClientRect();
        return {
          tag: el.tagName,
          cls: el.className && String(el.className),
          text: (el.textContent || "").trim().slice(0, 80),
          x: r.x,
          y: r.y,
          w: r.width,
          h: r.height,
          sw: el.scrollWidth,
          cw: el.clientWidth,
        };
      })
      .filter((r) => r.w > 0 && r.h > 0 && (r.x < -2 || r.x + r.w > innerWidth + 2 || r.sw > r.cw + 2))
      .slice(0, 20);
    return {
      title: document.title,
      statusText: body.innerText.slice(0, 1000),
      rootChildren: root?.children.length || 0,
      bodyScrollWidth: body.scrollWidth,
      viewportWidth: innerWidth,
      overflow,
      buttons: [...document.querySelectorAll("button")]
        .map((button) => button.innerText.trim())
        .filter(Boolean)
        .slice(0, 60),
    };
  });
  await context.close();
  return { name, status: response?.status(), consoleMessages, pageErrors, failedRequests, metrics };
}

async function runInteractions() {
  const context = await browser.newContext({ viewport: { width: 1440, height: 950 } });
  const page = await context.newPage();
  const consoleMessages = [];
  const pageErrors = [];
  const failedRequests = [];
  page.on("console", (msg) => {
    if (["error", "warning"].includes(msg.type())) {
      consoleMessages.push({ type: msg.type(), text: msg.text() });
    }
  });
  page.on("pageerror", (err) => pageErrors.push(String(err.stack || err.message || err)));
  page.on("requestfailed", (req) => {
    failedRequests.push({ url: req.url(), failure: req.failure()?.errorText });
  });
  await page.goto("http://127.0.0.1:1420", { waitUntil: "networkidle", timeout: 30000 });
  const interactions = [];
  async function record(name, fn) {
    try {
      await fn();
      interactions.push({ name, ok: true });
    } catch (err) {
      interactions.push({ name, ok: false, error: String(err.message || err) });
    }
  }

  await record("open inspector", async () => {
    await page.getByRole("button", { name: /inspect/i }).click();
    await page.getByText("Project timeline").waitFor({ timeout: 5000 });
    await page.screenshot({ path: "../qa-inspector.png", fullPage: true });
    await page.getByRole("button", { name: /close/i }).click();
  });
  await record("open external collaboration", async () => {
    await page.getByText(/Invite External/i).first().click();
    await page.getByText("INVITE PERMISSIONS").waitFor({ timeout: 5000 });
    await page.screenshot({ path: "../qa-external-popover.png", fullPage: true });
  });
  await record("generate invite", async () => {
    await page.getByRole("button", { name: /generate invite/i }).click();
    await page.locator(".invite-result code").waitFor({ timeout: 6000 });
  });
  await record("add mcp server", async () => {
    await page.getByLabel("Name").fill("qa-test-server");
    await page.getByLabel("Command").fill("qa-command-does-not-exist");
    await page.getByLabel(/Args/).fill("--version");
    await page.getByRole("button", { name: /add server/i }).click();
    await page.waitForTimeout(1200);
  });
  await record("mcp manager visible", async () => {
    await page.getByText("MCP MANAGER").waitFor({ timeout: 8000 });
    await page.screenshot({ path: "../qa-mcp-manager.png", fullPage: true });
  });
  await record("mcp health action", async () => {
    await page.getByRole("button", { name: /health/i }).first().click();
    await page.waitForTimeout(1200);
  });

  const state = await page.evaluate(() => ({
    text: document.body.innerText.slice(0, 2500),
    popovers: [...document.querySelectorAll(".collab-popover")].map((el) => el.innerText.slice(0, 800)),
    overflow: [...document.querySelectorAll("body *")]
      .map((el) => {
        const r = el.getBoundingClientRect();
        return {
          cls: String(el.className),
          text: (el.textContent || "").trim().slice(0, 80),
          x: r.x,
          y: r.y,
          w: r.width,
          sw: el.scrollWidth,
          cw: el.clientWidth,
        };
      })
      .filter((r) => r.w > 0 && (r.x < -2 || r.x + r.w > innerWidth + 2 || r.sw > r.cw + 2))
      .slice(0, 20),
  }));
  await context.close();
  return { interactions, consoleMessages, pageErrors, failedRequests, state };
}

const report = {
  desktop: await runViewport("desktop", 1440, 950),
  mobile: await runViewport("mobile", 375, 812),
  interaction: await runInteractions(),
};

await browser.close();
console.log(JSON.stringify(report, null, 2));
