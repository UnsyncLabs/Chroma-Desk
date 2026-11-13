/** Advanced tab: users fix LED counts from the UI (number input + Apply, and the fan preset). */
import { assert, sleep } from "../lib.mjs";

/** Real keyboard entry into the nth element matching `selector`. */
async function typeInto(t, selector, index, value) {
  const els = await t.page.$$(selector);
  const el = els[index];
  assert.ok(el, `${selector}[${index}] exists`);
  await el.evaluate((e) => e.scrollIntoView({ block: "center" }));
  await el.click({ clickCount: 3 });
  await t.page.keyboard.type(String(value));
  await sleep(120);
}

const rows = (t) =>
  t.page.$$eval("section table tbody tr:not(.preset)", (trs) =>
    trs.map((tr) => ({ name: tr.children[0].textContent.trim(), range: tr.children[3].textContent.trim(), leds: tr.querySelector("input[type=number]")?.value ?? tr.children[2].textContent.trim() })),
  );

/** Clicks the nth "Apply" button (only resizable zones have one). */
async function apply(t, n) {
  const btns = await t.page.$$("tbody button");
  const labelled = [];
  for (const b of btns) if ((await b.evaluate((e) => e.textContent.trim())) === "Apply") labelled.push(b);
  await labelled[n].evaluate((e) => e.scrollIntoView({ block: "center" }));
  await labelled[n].click();
  await sleep(150);
}

async function usePreset(t, n) {
  const btns = await t.page.$$("tr.preset button");
  await btns[n].evaluate((e) => e.scrollIntoView({ block: "center" }));
  await btns[n].click();
  await sleep(150);
}

export default async function (t, url) {
  await t.load(url);

  // ---- Corsair Lighting Node Core: headers 0-204 ----
  await t.openDevice("Corsair");
  await t.tab("Advanced");
  await t.waitFor(() => t.page.$("section table"), "zone table");
  let r = await rows(t);
  assert.equal(r[0].range, "0–204", "Corsair header range matches the real server");
  assert.equal(r[1].range, "0–204");
  assert.equal(r[0].leds, "24");
  await t.shot("advanced-corsair", 1200, 800);
  await t.shot("advanced-corsair", 400, 400);

  // Number input + Apply on header 2 (empty by default).
  await typeInto(t, 'tbody input[aria-label="LEDs"]', 1, 16);
  await apply(t, 1);
  await t.waitFor(async () => (await rows(t))[1].leds === "16", "header 2 resized to 16");
  assert.equal((await rows(t))[0].leds, "24", "other header untouched");

  // Fan preset on header 1: 5 x AF120 (8 LEDs) = 40, then back to 3 x = 24.
  await typeInto(t, 'input[aria-label="Number of fans"]', 0, 5);
  assert.equal(await t.page.$eval("tr.preset b", (b) => b.textContent), "40");
  await usePreset(t, 0);
  await t.waitFor(async () => (await rows(t))[0].leds === "40", "header 1 resized by the preset");
  await typeInto(t, 'input[aria-label="Number of fans"]', 0, 3);
  await usePreset(t, 0);
  await t.waitFor(async () => (await rows(t))[0].leds === "24", "3 x AF120 = 24");
  assert.equal(await t.page.$$eval(".leds .led", (e) => e.length), 40, "LED table follows (24 + 16)");
  const sc = await t.scene();
  assert.equal(sc.devices["Corsair Lighting Node Core@1"].fan_leds["Corsair RGB Header 1"], 8, "ring size remembered");

  // ---- X870 board: ARGB headers 0-256 ----
  await t.openDevice("X870");
  await t.waitFor(async () => /X870/.test(await t.page.$eval(".title h2", (e) => e.textContent)), "board page");
  await sleep(400);
  await t.tab("Advanced");
  await t.waitFor(() => t.page.$("section table"), "board zone table").catch((e) => { throw new Error(e.message + " " + t.errors.join(" | ")); });
  r = await rows(t);
  const argb = r.filter((x) => x.name.startsWith("ARGB_V2_"));
  assert.equal(argb.length, 3);
  for (const a of argb) assert.equal(a.range, "0–256");
  assert.equal(argb[0].leds, "0");
  await t.shot("advanced-x870", 1200, 800);
  await t.shot("advanced-x870", 400, 400);
  await typeInto(t, 'tbody input[aria-label="LEDs"]', 1, 60);
  await apply(t, 1);
  await t.waitFor(async () => (await rows(t)).find((x) => x.name === "ARGB_V2_2")?.leds === "60", "ARGB_V2_2 resized");
  await typeInto(t, 'tbody input[aria-label="LEDs"]', 0, 256);
  await apply(t, 0);
  await t.waitFor(async () => (await rows(t)).find((x) => x.name === "ARGB_V2_1")?.leds === "256", "ARGB_V2_1 at the maximum");
  // Preset on the third header: 12 fans x 8 = 96.
  await typeInto(t, 'input[aria-label="Number of fans"]', 2, 12);
  await usePreset(t, 2);
  await t.waitFor(async () => (await rows(t)).find((x) => x.name === "ARGB_V2_3")?.leds === "96", "ARGB_V2_3 by preset");
}
