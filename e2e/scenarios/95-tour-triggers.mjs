/** First-run tour (3 steps) and the Triggers section of Settings. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url, { tour: true });
  await t.waitFor(() => t.page.$("dialog[open] #tour-title"), "tour opens on first run");
  const title = () => t.page.$eval("#tour-title", (e) => e.textContent);
  assert.equal(await title(), "Start with Quick");
  assert.match(await t.page.$eval("dialog[open] .count", (e) => e.textContent), /Step 1 of 3/);
  await t.shot("tour", 1200, 800);
  await t.shot("tour", 400, 400);
  await t.clickText("dialog[open] button", "^Next$");
  assert.equal(await title(), "Draw your own");
  await t.clickText("dialog[open] button", "^Back$");
  assert.equal(await title(), "Start with Quick");
  await t.clickText("dialog[open] button", "^Next$");
  await t.clickText("dialog[open] button", "^Next$");
  assert.equal(await title(), "Keep them in the library");
  await t.clickText("dialog[open] button", "^Open the library$");
  await sleep(300);
  assert.equal(await t.page.$("dialog[open]"), null, "tour closed");
  assert.ok(await t.page.$(".grid [data-anim]"), "landed on the library");
  assert.equal(await t.page.evaluate(() => localStorage.getItem("chroma-desk.tour")), "done");

  // It does not come back after a reload, but Settings can replay it.
  await t.page.reload({ waitUntil: "networkidle0" });
  await sleep(800);
  assert.equal(await t.page.$("dialog[open]"), null, "no tour on the second run");
  await t.nav("^Settings");
  await t.clickText("button", "^Show the tour again$");
  await t.waitFor(() => t.page.$("dialog[open] #tour-title"), "replayed");
  await t.page.keyboard.press("Escape");
  await sleep(300);
  assert.equal(await t.page.$("dialog[open]"), null, "Escape skips the tour");

  // --- Triggers section.
  const text = await t.page.evaluate(() => document.body.innerText);
  assert.match(text, /Triggers \(notifications\)/);
  assert.match(text, /chroma-desk trigger "Alert" --keys W,A,S,D --duration 2s/);
  assert.match(text, /socat - UNIX-CONNECT:\/run\/user\/1000\/chroma-desk\.sock/, "shows the real socket path");
  await t.clickText("section button", "^Copy$");
  await sleep(200);
  await t.clickText("section button", "^Try it: flash WASD$");
  await t.waitFor(async () => (await t.mockState("triggers")).length === 1, "trigger sent");
  const trig = (await t.mockState("triggers"))[0];
  assert.deepEqual(trig.keys, ["W", "A", "S", "D"]);
  assert.equal(trig.animation, "Alert");
  await t.clickText("section button", "^Stop all$");
  await t.waitFor(async () => /Stopped 1 trigger/.test(await t.page.evaluate(() => document.body.innerText)), "stopped");
  await t.shot("settings-triggers", 1200, 800);
  await t.shot("settings-triggers", 400, 400);
}
