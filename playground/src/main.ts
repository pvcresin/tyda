// Browser front-end for the Tyda Playground (TypeProf.wasm-style, LSP-like).
//
// Runs the same wasm32-wasip1 module the CLI builds, under a browser WASI shim,
// with vendor/rbs mounted into a virtual fs at /rbs. The two editor panes are:
//   left  (#rbs)  — hand-written RBS used as extra type context
//   right (#ruby) — Ruby source, annotated with inferred-type CodeLens and
//                   missing-method diagnostics (squiggles); hover shows types.
//
// One analysis = one fresh WASI instance. We pipe {ruby, rbs} JSON in via stdin
// and read one JSON line from stdout:
//   { rbs, diagnostics:[…], hovers:[…], code_lens:[…] }
// (diagnostic/hover line 1-based, column 0-based; code_lens line 1-based.)
//
// State (ruby + rbs) is compressed into location.hash with lz-string on every
// analysis, so the address-bar URL always restores the playground — copy it to
// share. No separate Share button is needed.
//
// Monaco, lz-string and the WASI shim are bundled from npm by Vite (no CDN).
import {
  ConsoleStdout,
  Directory,
  File,
  type Inode,
  OpenFile,
  PreopenDirectory,
  WASI,
} from "@bjorn3/browser_wasi_shim";
import LZString from "lz-string";
// Minimal Monaco: the editor API + only the two editor features the playground
// uses (CodeLens and hover) + the Ruby Monarch grammar. Importing the full
// "monaco-editor" (or edcore.main) pulls in ~50 editor contributions and every
// language/language-service, which dominates the bundle.
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import "monaco-editor/esm/vs/editor/contrib/codelens/browser/codelensController.js";
import "monaco-editor/esm/vs/editor/contrib/hover/browser/hoverContribution.js";
import "monaco-editor/esm/vs/basic-languages/ruby/ruby.contribution";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

// The playground only edits the "ruby" language (no JSON/TS/CSS language
// services), so the base editor worker is all Monaco needs.
(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = {
  getWorker: () => new EditorWorker(),
};

// ── Types ────────────────────────────────────────────────────────────────────
interface Diagnostic {
  line?: number;
  column?: number;
  end_line?: number;
  end_column?: number;
  message?: string;
}
interface Hover {
  line: number;
  column: number;
  end_line: number;
  end_column: number;
  name: string;
  display: string;
}
interface CodeLensItem {
  line: number;
  signature: string;
}
interface AnalysisResult {
  rbs: string;
  diagnostics: Diagnostic[];
  hovers: Hover[];
  code_lens: CodeLensItem[];
  ruby_syntax_error: boolean;
  rbs_syntax_error: boolean;
}
interface State {
  ruby: string;
  rbs: string;
}

declare global {
  interface Window {
    monaco: typeof monaco;
    LZString: typeof LZString;
    __tyda: AnalysisResult;
    __editors: {
      ruby: monaco.editor.IStandaloneCodeEditor;
      rbs: monaco.editor.IStandaloneCodeEditor;
    };
  }
}

// E2E tests (and ad-hoc debugging) reach for these globals.
window.monaco = monaco;
window.LZString = LZString;

window.addEventListener(
  "keydown",
  (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
    }
  },
  { capture: true },
);

const SAMPLE_RUBY = `class User
  def initialize(name)
    @name = name
  end

  def name = @name

  def greeting = "hello, #{@name}"

  def ids = [1, 2, 3].map { |n| n * 2 }

  def first_id = ids[0]

  def created = Time.now.year

  # Unresolved method calls show up as diagnostics (squiggles)
  def oops = totally_undefined_helper(@name)
end
`;

const SAMPLE_RBS = `# Hand-written RBS here is passed as type context for the Ruby pane on the right.
# Example:
# class User
#   def name: () -> String
# end
`;

// ── URL state (lz-string into location.hash) ────────────────────────────────
function encodeState(state: State): string {
  return LZString.compressToEncodedURIComponent(JSON.stringify(state));
}
function decodeState(hash: string): State | null {
  if (!hash) return null;
  try {
    const json = LZString.decompressFromEncodedURIComponent(hash.replace(/^#/, ""));
    if (!json) return null;
    const parsed = JSON.parse(json);
    if (typeof parsed.ruby !== "string" || typeof parsed.rbs !== "string") {
      return null;
    }
    return parsed;
  } catch (_e) {
    return null;
  }
}

// Build the [[name, File|Directory]] entry tree browser_wasi_shim wants from
// the flat { "core/array.rbs": contents, … } bundle.
type RbsTree = Map<string, RbsTree | File>;
function buildRbsTree(bundle: Record<string, string>): Map<string, Inode> {
  const enc = new TextEncoder();
  const root: RbsTree = new Map();
  for (const [path, content] of Object.entries(bundle)) {
    const parts = path.split("/");
    let node = root;
    for (let i = 0; i < parts.length - 1; i++) {
      if (!node.has(parts[i])) node.set(parts[i], new Map());
      node = node.get(parts[i]) as RbsTree;
    }
    node.set(parts.at(-1)!, new File(enc.encode(content)));
  }
  const toDir = (map: RbsTree): Map<string, Inode> => {
    const out = new Map<string, Inode>();
    for (const [name, val] of map) {
      out.set(name, val instanceof Map ? new Directory(toDir(val)) : val);
    }
    return out;
  };
  return toDir(root);
}

let wasmModule: WebAssembly.Module | null = null;
let rbsTree: Map<string, Inode> = new Map();

// Latest analysis overlays, consumed by the Monaco providers.
let currentHovers: Hover[] = [];
let currentCodeLens: CodeLensItem[] = [];
// Fired after each analysis to make Monaco re-pull the CodeLenses.
const lensEmitter = new monaco.Emitter<monaco.languages.CodeLensProvider>();
let codeLensProvider: monaco.languages.CodeLensProvider | null = null;

async function loadAssets(): Promise<void> {
  // arrayBuffer + compile (not compileStreaming) so it works regardless of
  // whether the static server sends Content-Type: application/wasm.
  const [mod, bundle] = await Promise.all([
    fetch("./tyda.wasm")
      .then((r) => r.arrayBuffer())
      .then((b) => WebAssembly.compile(b)),
    fetch("./rbs-bundle.json").then((r) => r.json()),
  ]);
  wasmModule = mod;
  rbsTree = buildRbsTree(bundle);
}

// One analyze = one fresh WASI instance (the command module runs `_start`
// once). {ruby, rbs} JSON in via stdin, JSON out via stdout.
async function analyze(ruby: string, rbs: string): Promise<AnalysisResult> {
  const enc = new TextEncoder();
  const stdin = JSON.stringify({ ruby, rbs });
  // Capture stdout into a File buffer rather than ConsoleStdout.lineBuffered:
  // the wasm prints a single JSON line, and line buffering would withhold
  // output that isn't newline-terminated. Reading File.data is newline-safe.
  const stdout = new File(new Uint8Array());
  const fds = [
    new OpenFile(new File(enc.encode(stdin))),
    new OpenFile(stdout),
    ConsoleStdout.lineBuffered((m) => console.warn("[tyda stderr]", m)),
    new PreopenDirectory("/rbs", rbsTree),
  ];
  const wasi = new WASI(["tyda-wasm"], [], fds);
  const instance = await WebAssembly.instantiate(wasmModule!, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });
  wasi.start(instance as { exports: { memory: WebAssembly.Memory; _start: () => unknown } });
  const text = new TextDecoder().decode(stdout.data).trim();
  const empty: AnalysisResult = {
    rbs: "",
    diagnostics: [],
    hovers: [],
    code_lens: [],
    ruby_syntax_error: false,
    rbs_syntax_error: false,
  };
  if (!text) return empty;
  try {
    return { ...empty, ...JSON.parse(text) };
  } catch (_e) {
    // Non-JSON stdout: treat as raw RBS text (older wasm builds).
    return { ...empty, rbs: text };
  }
}

// Tyda line/column are 1-based line / 0-based column. Monaco is 1-based for
// both, so column needs +1; line is used as-is.
function toMarkers(diagnostics: Diagnostic[]): monaco.editor.IMarkerData[] {
  return (diagnostics || []).map((d) => ({
    severity: monaco.MarkerSeverity.Warning,
    message: d.message || "unresolved",
    source: "tyda",
    startLineNumber: d.line ?? 1,
    startColumn: (d.column ?? 0) + 1,
    endLineNumber: d.end_line ?? d.line ?? 1,
    endColumn: (d.end_column ?? d.column ?? 0) + 1,
  }));
}

// CodeLens: inferred signature above each method definition line. Clicking a
// lens inserts the signature as a `#: …` RBS inline comment (TypeProf-style);
// the method is then annotated, so on re-analysis its lens disappears.
// `insertCmdId` is the editor command registered to perform the insertion.
function registerCodeLens(insertCmdId: string): void {
  codeLensProvider = {
    onDidChange: lensEmitter.event,
    provideCodeLenses() {
      return {
        lenses: (currentCodeLens || []).map((lens, i) => ({
          range: {
            startLineNumber: lens.line,
            startColumn: 1,
            endLineNumber: lens.line,
            endColumn: 1,
          },
          id: `tyda-lens-${i}`,
          command: {
            id: insertCmdId,
            // Match the VSCode/LSP CodeLens: the title is the exact `#: …`
            // inline RBS comment that clicking inserts.
            title: `#: ${lens.signature}`,
            tooltip: "Click to insert",
            arguments: [lens.line, lens.signature],
          },
        })),
        dispose() {},
      };
    },
    resolveCodeLens(_model, lens) {
      return lens;
    },
  };
  monaco.languages.registerCodeLensProvider("ruby", codeLensProvider);
}

// Insert `<indent>#: <signature>` on the line above the method definition, so
// the inferred type becomes an explicit RBS inline annotation.
function insertSignatureComment(
  editor: monaco.editor.IStandaloneCodeEditor,
  line: number,
  signature: string,
): void {
  const model = editor.getModel();
  if (!model) return;
  const indent = (model.getLineContent(line).match(/^\s*/) || [""])[0];
  editor.executeEdits("tyda-codelens", [
    {
      range: new monaco.Range(line, 1, line, 1),
      text: `${indent}#: ${signature}\n`,
    },
  ]);
}

// VSCode-style hover: show the inferred type for the narrowest span under the
// cursor. Spans use 1-based line / 0-based column.
function registerHover(): void {
  monaco.languages.registerHoverProvider("ruby", {
    provideHover(_model, position) {
      const ln = position.lineNumber;
      const col = position.column - 1; // Monaco col is 1-based → 0-based
      const inSpan = (h: Hover) => {
        const afterStart = h.line < ln || (h.line === ln && h.column <= col);
        const beforeEnd = h.end_line > ln || (h.end_line === ln && h.end_column >= col);
        return afterStart && beforeEnd;
      };
      const hits = (currentHovers || []).filter(inSpan);
      if (hits.length === 0) return null;
      // Prefer the narrowest span covering the cursor.
      const span = (h: Hover) => (h.end_line - h.line) * 1e6 + (h.end_column - h.column);
      hits.sort((a, b) => span(a) - span(b));
      const h = hits[0];
      // `display` is already the exact hover body the LSP returns (it goes
      // through the shared `format_hover_body`), so show it verbatim to mirror
      // the editor — do not re-prepend the name.
      const label = h.display;
      return {
        range: new monaco.Range(h.line, h.column + 1, h.end_line, h.end_column + 1),
        contents: [{ value: "```rbs\n" + label + "\n```" }],
      };
    },
  });
}

// Show / clear a "syntax error" badge next to a pane heading.
function setSyntaxBadge(titleId: string, label: string, hasError: boolean): void {
  const el = document.getElementById(titleId);
  if (!el) return;
  el.textContent = label;
  if (hasError) {
    const badge = document.createElement("span");
    badge.className = "syntax-error";
    badge.textContent = "syntax error";
    el.appendChild(badge);
  }
}

async function main(): Promise<void> {
  const restored = decodeState(location.hash);
  const ruby = monaco.editor.create(document.getElementById("ruby")!, {
    value: restored?.ruby ?? SAMPLE_RUBY,
    language: "ruby",
    theme: "vs-dark",
    minimap: { enabled: false },
    fontSize: 13,
    automaticLayout: true,
    codeLens: true,
  });

  // Command invoked when a CodeLens is clicked → insert its `#:` signature.
  const insertCmdId = ruby.addCommand(0, (_ctx, line: number, signature: string) =>
    insertSignatureComment(ruby, line, signature),
  )!;
  registerCodeLens(insertCmdId);
  registerHover();
  const rbs = monaco.editor.create(document.getElementById("rbs")!, {
    value: restored?.rbs ?? SAMPLE_RBS,
    language: "ruby", // RBS isn't a built-in Monaco language; ruby highlights well enough
    theme: "vs-dark",
    minimap: { enabled: false },
    fontSize: 13,
    automaticLayout: true,
    // The CodeLens / hover providers are registered for the "ruby" language and
    // this pane shares it for highlighting — but those overlays describe the
    // right-hand Ruby analysis, so disable them here to keep the rbs pane clean.
    codeLens: false,
    hover: { enabled: false },
  });

  // Expose the editors for E2E tests / debugging.
  window.__editors = { ruby, rbs };

  let timer: ReturnType<typeof setTimeout> | null = null;
  const run = async () => {
    if (!wasmModule) return; // assets still loading — the first run fires after loadAssets()
    try {
      const result = await analyze(ruby.getValue(), rbs.getValue());
      currentHovers = result.hovers || [];
      currentCodeLens = result.code_lens || [];
      // Expose the latest analysis for E2E tests / debugging.
      window.__tyda = result;
      monaco.editor.setModelMarkers(ruby.getModel()!, "tyda", toMarkers(result.diagnostics));
      if (codeLensProvider) lensEmitter.fire(codeLensProvider); // refresh lenses
      // Parse status shown as a heading badge — deliberately NOT a squiggle, so
      // it isn't mistaken for one of Tyda's type-inference diagnostics.
      setSyntaxBadge("rb-title", "rb", result.ruby_syntax_error);
      setSyntaxBadge("rbs-title", "rbs", result.rbs_syntax_error);
      // Persist state into the URL hash (without scrolling / history spam).
      // The initial example needs no sharable URL, so keep the address bar
      // clean until the user actually edits something — this is also the state
      // the title-click reset returns to.
      const rubyValue = ruby.getValue();
      const rbsValue = rbs.getValue();
      const isInitial = rubyValue === SAMPLE_RUBY && rbsValue === SAMPLE_RBS;
      history.replaceState(
        null,
        "",
        isInitial
          ? location.pathname + location.search
          : "#" + encodeState({ ruby: rubyValue, rbs: rbsValue }),
      );
    } catch (e) {
      console.error(e);
    }
  };

  const schedule = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(run, 350);
  };
  ruby.onDidChangeModelContent(schedule);
  rbs.onDidChangeModelContent(schedule);

  // Load a (possibly null) decoded state into the editors and re-analyze now,
  // cancelling the debounced run that `setValue` would otherwise trigger.
  const applyState = (state: State | null) => {
    ruby.setValue(state?.ruby ?? SAMPLE_RUBY);
    rbs.setValue(state?.rbs ?? SAMPLE_RBS);
    if (timer) clearTimeout(timer);
    run();
  };

  // Browser back / forward navigates between history entries (e.g. the clean
  // entry pushed by a title-click reset and the prior edited state). Restore
  // the editors from whatever hash that entry carries.
  window.addEventListener("popstate", () => applyState(decodeState(location.hash)));

  // Clicking the title returns to the initial example with a clean URL. Push a
  // new history entry (rather than replacing) so Back restores the pre-reset
  // editor state from its hash.
  document.getElementById("reset")?.addEventListener("click", () => {
    if (location.hash) {
      history.pushState(null, "", location.pathname + location.search);
    }
    applyState(null);
  });

  // The editors are interactive immediately; load the wasm + RBS bundle in the
  // background and run the first analysis once they're ready.
  await loadAssets();
  await run();
}

main().catch((e) => console.error(e));
