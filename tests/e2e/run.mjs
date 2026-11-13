// Drives the real application binary. See run.sh for how it is launched.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readdir, readFile } from "node:fs/promises";
import { Driver } from "./webdriver.mjs";

const application = process.env.DAYTRACE_BINARY;
const database = process.env.DAYTRACE_DATABASE;
const shots = process.env.DAYTRACE_SHOTS;
await mkdir(shots, { recursive: true });

const driver = new Driver();
const heading = (title) => `//h1[normalize-space()="${title}"]`;
let passed = 0;

async function check(name, action) {
  await action();
  passed += 1;
  console.log(`ok - ${name}`);
}

await driver.start(application);

await check("first launch shows onboarding, not tracking", async () => {
  await driver.seesText("DayTrace records how you use your computer");
  await driver.screenshot(`${shots}/onboarding-1.png`);
});

await check("onboarding explains what is and is not collected", async () => {
  await driver.clickText("Continue");
  await driver.clickText("Continue");
  await driver.seesText("The name and program path of the focused application");
  await driver.clickText("Continue");
  await driver.seesText("Keystrokes, passwords or clipboard contents");
  await driver.clickText("Continue");
  await driver.seesText("no telemetry");
  await driver.clickText("Continue");
  await driver.seesText("What works on this computer");
  await driver.seesText("Idle detection");
  await driver.screenshot(`${shots}/onboarding-capabilities.png`);
});

await check("declining consent leaves tracking off", async () => {
  await driver.clickText("Not now");
  await driver.find(heading("Dashboard"));
  await driver.seesText("Automatic tracking is off");
  await driver.seesText("Tracking is off");
});

for (const [page, title] of [
  ["timeline", "Timeline"],
  ["applications", "Applications"],
  ["categories", "Categories"],
  ["analytics", "Analytics"],
  ["settings", "Settings"],
  ["dashboard", "Dashboard"],
]) {
  await check(`${title} page opens`, async () => {
    await driver.visit(page);
    await driver.find(heading(title));
  });
}

await check("the nine default categories are present", async () => {
  await driver.visit("categories");
  await driver.seesText("Administration");
  await driver.seesText("Entertainment");
});

await check("projects are gone from the navigation", async () => {
  assert.equal((await driver.findAll('//nav//a[contains(., "Project")]')).length, 0);
});

await check("a category can be created", async () => {
  await driver.visit("categories");
  await driver.clickText("New category");
  await driver.type('//dialog[@open]//label[contains(., "Name")]//input', "Thesis");
  await driver.clickText("Create category");
  await driver.find('//*[normalize-space()="Thesis"]');
});

await check("a rule can be created from the Categories page", async () => {
  await driver.clickText("New rule");
  await driver.type('//dialog[@open]//label[contains(., "Name")]//input', "Editors are thesis work");
  await driver.type('//dialog[@open]//input[@aria-label="Value"]', "code");
  await driver.script(`
    const select = [...document.querySelectorAll("dialog[open] select")].at(-1);
    const option = [...select.options].find((item) => item.text === "Thesis");
    const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set;
    set.call(select, option.value);
    select.dispatchEvent(new Event("change", { bubbles: true }));
  `);
  await driver.clickText("Create rule");
  await driver.find('//li//*[normalize-space()="Editors are thesis work"]');
});

await check("a past entry can be added to the timeline", async () => {
  await driver.visit("timeline");
  await driver.clickText("Add entry");
  await driver.type('//dialog[@open]//label[contains(., "Label")]//input', "Library reading");
  await driver.click('//dialog[@open]//button[normalize-space()="Add entry"]');
  await driver.find('//li//*[normalize-space()="Library reading"]');
});

await check("overlapping entries are refused with a clear message", async () => {
  await driver.clickText("Add entry");
  await driver.click('//dialog[@open]//button[normalize-space()="Add entry"]');
  await driver.seesText("overlaps an existing session");
  await driver.click('//dialog[@open]//button[normalize-space()="Cancel"]');
});

await check("a session can be corrected", async () => {
  await driver.click('//li//button[contains(., "Library reading")]');
  await driver.type('//dialog[@open]//label[contains(., "Label")]//input', "Reading room");
  await driver.clickText("Save changes");
  await driver.find('//li//*[normalize-space()="Reading room"]');
});

await check("a manual timer starts and stops", async () => {
  await driver.click('//aside//button[normalize-space()="Timer"]');
  await driver.type('//dialog[@open]//input', "Notes");
  await driver.clickText("Start timer");
  await driver.seesText("Timer running");
  await new Promise((resolve) => setTimeout(resolve, 1200));
  await driver.clickText("Stop timer");
  await driver.seesText("Tracking is off");
});

await check("starting with the computer writes a real autostart entry", async () => {
  await driver.visit("settings");
  await driver.clickText("Startup");
  await driver.click('//button[@role="switch"][@aria-label="Start automatically when you turn on your computer"]');
  await new Promise((resolve) => setTimeout(resolve, 600));
  const directory = `${process.env.HOME}/.config/autostart`;
  const entries = await readdir(directory);
  assert.equal(entries.length, 1, "one autostart entry");
  const entry = await readFile(`${directory}/${entries[0]}`, "utf8");
  assert.match(entry, /Exec=.*daytrace.*--background/);
  await driver.click('//button[@role="switch"][@aria-label="Start automatically when you turn on your computer"]');
  await new Promise((resolve) => setTimeout(resolve, 600));
  assert.deepEqual(await readdir(directory), [], "turning it off removes the entry");
});

await check("starting hidden depends on both startup options", async () => {
  const hidden = '//button[@role="switch"][@aria-label="Start without opening a window"]';
  assert.equal(await driver.command("GET", `/element/${await driver.find(hidden)}/enabled`), false);
  await driver.screenshot(`${shots}/settings-startup.png`);
});

await check("the interface starts dark and the theme setting applies immediately", async () => {
  assert.equal(await driver.script('return document.documentElement.dataset.theme ?? ""'), "dark");
  await driver.visit("settings");
  await driver.clickText("Appearance");
  await driver.find('//label[contains(., "Theme")]//select');
  await driver.script(`
    const select = [...document.querySelectorAll("select")].find((item) => item.value === "dark");
    const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set;
    set.call(select, "light");
    select.dispatchEvent(new Event("change", { bubbles: true }));
  `);
  await new Promise((resolve) => setTimeout(resolve, 400));
  assert.equal(await driver.script('return document.documentElement.dataset.theme ?? ""'), "light");
  await driver.script(`
    const select = [...document.querySelectorAll("select")].find((item) => item.value === "light");
    const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set;
    set.call(select, "dark");
    select.dispatchEvent(new Event("change", { bubbles: true }));
  `);
});

await check("the title bar has window controls and a drag region", async () => {
  for (const label of ["Minimize", "Maximize", "Close"]) {
    await driver.find(`//header[@data-tauri-drag-region="deep"]//button[@aria-label="${label}"]`);
  }
  await driver.seesText("DayTrace");
});

await driver.stop();
await new Promise((resolve) => setTimeout(resolve, 800));

const seeded = spawnSync("python3", ["-I", new URL("./seed.py", import.meta.url).pathname, database], { encoding: "utf8" });
assert.equal(seeded.status, 0, seeded.stderr);

await driver.start(application);

await check("settings and history survive a restart", async () => {
  await driver.find(heading("Dashboard"));
  await driver.seesText("Computer use");
});

const capture = async (page, name, prepare) => {
  await driver.visit(page);
  await new Promise((resolve) => setTimeout(resolve, 700));
  if (prepare) await prepare();
  await new Promise((resolve) => setTimeout(resolve, 900));
  await driver.screenshot(`${shots}/${name}.png`);
  // Only the content area scrolls. If the page itself grew taller than the
  // window, a hard scroll would lift the title bar and sidebar off screen.
  const overflow = await driver.script("return document.documentElement.scrollHeight - window.innerHeight");
  assert.equal(overflow, 0, `${name} makes the window content taller than the window`);
};

await check("populated pages render from stored data", async () => {
  await capture("dashboard", "dashboard-today");
  await capture("dashboard", "dashboard-week", () => driver.clickText("Last 7 days"));
  await capture("dashboard", "dashboard-month", () => driver.clickText("Last 30 days"));
  await capture("timeline", "timeline");
  await capture("applications", "applications");
  await capture("categories", "categories");
  await capture("analytics", "analytics");
  await capture("settings", "settings-tracking");
});

await check("rule preview reports changes without applying them", async () => {
  await driver.visit("categories");
  await driver.clickText("Preview last 30 days");
  await driver.seesText("sessions would change");
  await driver.screenshot(`${shots}/rule-preview.png`);
  await driver.click('//dialog[@open]//button[normalize-space()="Cancel"]');
});

await check("light theme renders", async () => {
  await driver.script('document.documentElement.dataset.theme = "light"');
  await capture("dashboard", "dashboard-light");
  await capture("analytics", "analytics-light");
  await driver.script('document.documentElement.dataset.theme = "dark"');
});

await check("deleting all history clears sessions and applications", async () => {
  await driver.visit("settings");
  await driver.clickText("Privacy");
  await driver.clickText("Delete all history");
  await driver.click('//dialog[@open]//button[normalize-space()="Delete all history"]');
  await driver.seesText("sessions deleted", 15000);
  await driver.visit("applications");
  await driver.seesText("Applications appear here once tracking has recorded them");
  await driver.visit("categories");
  await driver.seesText("Terminal is development");
});

await driver.stop();
console.log(`\n${passed} end-to-end checks passed`);
