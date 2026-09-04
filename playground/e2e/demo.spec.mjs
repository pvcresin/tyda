import { test, expect } from "@playwright/test";

// End-to-end check that the Tyda Playground delivers a TypeProf.wasm-style,
// LSP-like experience against the freshly built wasm:
//   - inferred RBS for the sample User class
//   - CodeLens (inferred method signatures) on the Ruby pane
//   - diagnostics (squiggles) for an unresolved call
//   - hover spans carrying inferred types
//   - URL state restoration (lz-string in location.hash)
// Asserts behavior, not binary identity, so local (macOS) and CI (Ubuntu)
// builds can differ bit-for-bit.

test("infers RBS, emits CodeLens + diagnostics + hover", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (e) => pageErrors.push(String(e)));

  await page.goto("/");

  // Analysis is done once the front-end has exposed its result on window.__tyda.
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  // The front-end exposes the latest analysis on window.__tyda.
  const result = await page.evaluate(() => window.__tyda);
  expect(result, "window.__tyda should be populated").toBeTruthy();

  // 1) Inferred RBS for the sample User class.
  expect(result.rbs).toContain("class User");
  expect(result.rbs).toContain("def greeting: -> String");
  expect(result.rbs).toContain("def ids: -> Array[Integer]");

  // 2) CodeLens: an inferred signature per method definition line.
  expect(Array.isArray(result.code_lens)).toBe(true);
  expect(result.code_lens.length).toBeGreaterThan(0);
  const lensSignatures = result.code_lens.map((l) => l.signature).join(" ");
  expect(lensSignatures).toContain("-> String");
  expect(lensSignatures).toContain("-> Array[Integer]");
  // The lenses are actually rendered into the editor.
  await expect(page.locator(".codelens-decoration").first()).toBeVisible({
    timeout: 15_000,
  });

  // 3) Diagnostics: the unresolved `totally_undefined_helper` call is flagged.
  expect(Array.isArray(result.diagnostics)).toBe(true);
  expect(result.diagnostics.length).toBeGreaterThan(0);
  const markers = await page.evaluate(() =>
    window.monaco.editor
      .getModelMarkers({ owner: "tyda" })
      .map((m) => ({ message: m.message, line: m.startLineNumber })),
  );
  expect(markers.length).toBeGreaterThan(0);
  expect(markers.some((m) => m.message.includes("not found"))).toBe(true);

  // 4) Hover spans exist and carry rendered types.
  expect(Array.isArray(result.hovers)).toBe(true);
  expect(result.hovers.length).toBeGreaterThan(0);
  const hoverTypes = result.hovers.map((h) => h.display).join(" ");
  expect(hoverTypes.length).toBeGreaterThan(0);

  expect(pageErrors, "no uncaught page errors").toEqual([]);
});

test("prevents browser save shortcuts", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  const shortcuts = await page.evaluate(() =>
    [
      { key: "s", ctrlKey: true },
      { key: "s", metaKey: true },
    ].map((modifiers) => {
      const event = new KeyboardEvent("keydown", {
        ...modifiers,
        bubbles: true,
        cancelable: true,
      });
      const target = document.querySelector("#ruby textarea") || document.body;
      target.dispatchEvent(event);
      return event.defaultPrevented;
    }),
  );
  expect(shortcuts).toEqual([true, true]);
});

test("shows annotated parameter hovers and literal interpolation", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  const source = `class User
  #: ("test") -> void
  def initialize(name)
    @name = name
  end

  def greeting = "hello, #{@name}"
end
`;
  await page.evaluate((value) => window.__editors.ruby.setValue(value), source);
  await page.waitForFunction(
    () => window.__tyda?.rbs?.includes('def greeting: -> "hello, test"'),
    null,
    { timeout: 15_000 },
  );

  const result = await page.evaluate(() => window.__tyda);
  expect(result.rbs).toContain('def greeting: -> "hello, test"');
  expect(result.hovers).toContainEqual(
    expect.objectContaining({
      line: 3,
      column: 17,
      name: "name",
      display: '[Tyda] "test"',
    }),
  );
});

test("clicking a CodeLens inserts a #: comment and removes that lens", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  const before = await page.evaluate(() => window.__tyda.code_lens.length);
  expect(before).toBeGreaterThan(0);

  // Click the greeting lens (`-> String`).
  const link = page.locator(".codelens-decoration a", { hasText: "-> String" }).first();
  await expect(link).toBeVisible({ timeout: 15_000 });
  await link.click();

  // Re-analysis is debounced; the annotated method's lens should drop out.
  await page.waitForFunction((b) => window.__tyda && window.__tyda.code_lens.length < b, before, {
    timeout: 15_000,
  });

  const rubyText = await page.evaluate(() =>
    window.monaco.editor
      .getModels()
      .map((m) => m.getValue())
      .find((v) => v.includes("class User")),
  );
  expect(rubyText).toContain("#: -> String"); // inline RBS annotation inserted
});

test("shows a heading 'syntax error' badge for malformed rbs / ruby", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  const setPane = (which, text) =>
    page.evaluate(([which, text]) => window.__editors[which].setValue(text), [which, text]);

  // RBS written in Ruby syntax (the common mistake) → rbs badge, no squiggle.
  await setPane("rbs", "class User\n  def helper(name)\n  end\nend\n");
  await page.waitForFunction(() => window.__tyda?.rbs_syntax_error === true, null, {
    timeout: 15_000,
  });
  await expect(page.locator("#rbs-title")).toContainText("syntax error");
  await expect(page.locator("#rb-title")).not.toContainText("syntax error");

  // Broken Ruby → rb badge.
  await setPane("ruby", "class User\n  def x =\nend");
  await page.waitForFunction(() => window.__tyda?.ruby_syntax_error === true, null, {
    timeout: 15_000,
  });
  await expect(page.locator("#rb-title")).toContainText("syntax error");
});

test("keeps a clean URL by default and persists edits into the hash", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  // The initial example needs no sharable URL — the hash stays empty until
  // the user edits something.
  expect(await page.evaluate(() => location.hash)).toBe("");

  // Editing persists state into location.hash.
  await page.evaluate(() => window.__editors.ruby.setValue("class Widget\n  def size = 42\nend\n"));
  await page.waitForFunction(() => location.hash.length > 1, null, {
    timeout: 45_000,
  });
  const restored = await page.evaluate(() => {
    const json = window.LZString.decompressFromEncodedURIComponent(location.hash.replace(/^#/, ""));
    return JSON.parse(json);
  });
  expect(restored.ruby).toContain("class Widget");
});

test("restores ruby + rbs state from the URL hash", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  // Navigate to a hash carrying custom code and confirm it restores.
  const custom = { ruby: "class Widget\n  def size = 42\nend\n", rbs: "" };
  const customHash = await page.evaluate(
    (s) => window.LZString.compressToEncodedURIComponent(JSON.stringify(s)),
    custom,
  );
  // Set the hash, then reload so the app boots fresh and restores from it
  // (a hash-only change wouldn't re-run the page — but pasting a URL does).
  await page.goto(`/#${customHash}`);
  await page.reload();
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });
  const result = await page.evaluate(() => window.__tyda);
  expect(result.rbs).toContain("class Widget");
  expect(result.rbs).toContain("def size: -> 42"); // literal-typed inference ran
});

test("clicking the title resets to the initial example with a clean URL", async ({ page }) => {
  // Boot with custom code carried in the URL hash.
  const custom = { ruby: "class Widget\n  def size = 42\nend\n", rbs: "" };
  await page.goto("/");
  const customHash = await page.evaluate(
    (s) => window.LZString.compressToEncodedURIComponent(JSON.stringify(s)),
    custom,
  );
  await page.goto(`/#${customHash}`);
  await page.reload();
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });
  expect(await page.evaluate(() => location.hash)).not.toBe("");

  // Clicking the title drops the hash and resets to the sample in place. Wait
  // for the *sample* analysis (the pre-reset page only ever has the Widget
  // result), then assert the URL is clean.
  await page.click("#reset");
  await page.waitForFunction(() => window.__tyda?.rbs?.includes("class User"), null, {
    timeout: 45_000,
  });
  expect(await page.evaluate(() => location.hash)).toBe("");
});

test("browser Back restores the pre-reset editor state", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__tyda !== undefined, null, {
    timeout: 45_000,
  });

  // Edit to custom content (persisted into the hash).
  await page.evaluate(() => window.__editors.ruby.setValue("class Widget\n  def size = 42\nend\n"));
  await page.waitForFunction(() => location.hash.length > 1, null, {
    timeout: 45_000,
  });

  // Reset via the title, then Back should restore the edited code.
  await page.click("#reset");
  await page.waitForFunction(
    () => window.__editors.ruby.getValue().includes("class User") && location.hash === "",
    null,
    { timeout: 45_000 },
  );
  await page.goBack();
  await page.waitForFunction(
    () => window.__editors.ruby.getValue().includes("class Widget"),
    null,
    { timeout: 45_000 },
  );
});
