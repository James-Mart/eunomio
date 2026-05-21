import type { FileLineRanges } from "@/lib/api";
import { cssEscape } from "@/lib/utils";

const SYNTH_WRAP_CLASS = "eunomia-synthesized";
const SYNTH_LINE_ATTR = "data-eunomia-synthesized";
const SYNTH_GUTTER_ATTR = "data-eunomia-synthesized-gutter";

const STICKY_HEADER_CSS =
  "[data-diffs-header] { position: sticky; top: 0; z-index: 5; cursor: pointer; background-color: var(--diffs-bg-separator); border-bottom: 1px solid var(--diffs-bg-buffer); }";

// Per-word shimmer + per-line gutter glyph injected into each FileDiff's shadow
// DOM via the library's `unsafeCSS` option. Decoration spans we create at
// post-render are wrapped in `.eunomia-synthesized`; matching gutter cells get
// `data-eunomia-synthesized-gutter`.
const SYNTHESIZED_CSS =
  ".eunomia-synthesized{display:inline;background-image:linear-gradient(100deg,rgba(167,139,250,0) 0%,rgba(167,139,250,0.16) 25%,rgba(167,139,250,0.45) 50%,rgba(167,139,250,0.16) 75%,rgba(167,139,250,0) 100%);background-size:300% 100%;background-repeat:no-repeat;background-position:200% 0;animation:eunomia-shimmer 3.5s linear infinite;border-radius:1px;}" +
  "@keyframes eunomia-shimmer{0%{background-position:200% 0;}100%{background-position:-100% 0;}}" +
  "[data-eunomia-synthesized-gutter]{position:relative;}" +
  "[data-eunomia-synthesized-gutter]::after{content:'\\2731';position:absolute;right:2px;top:50%;transform:translateY(-50%);font-size:0.7em;line-height:1;color:rgb(196,181,253);pointer-events:none;}";

export const FILEDIFF_CSS = STICKY_HEADER_CSS + SYNTHESIZED_CSS;

const SYNTH_TOOLTIP = {
  child: "Synthesized — a later Edge overwrites this content.",
  parent: "Synthesized removal — a later Edge restores this content.",
} as const;

type LineSpans = ReadonlyArray<readonly [number, number]>;

export type SpanLookup = Map<string, Map<number, LineSpans>>;

export function buildLookup(files: FileLineRanges[]): SpanLookup {
  const m: SpanLookup = new Map();
  for (const f of files) {
    const lines = new Map<number, LineSpans>();
    for (const l of f.lines) lines.set(l.line, l.spans);
    m.set(f.path, lines);
  }
  return m;
}

// `@pierre/diffs` renders each `<FileDiff />` into a custom element with its
// own shadow root. Decorations have to be applied inside that root since
// styles and DOM from outside don't reach in. The library exposes
// `onPostRender(node, instance)` after each render pass; we walk
// `node.shadowRoot` and wrap synthesized character ranges in spans tagged with
// `.eunomia-synthesized`, then mark matching gutter cells.
export function decorateFileContainer(
  node: HTMLElement,
  childForFile: Map<number, LineSpans> | undefined,
  parentForFile: Map<number, LineSpans> | undefined,
) {
  const root = (node as HTMLElement & { shadowRoot: ShadowRoot | null }).shadowRoot;
  if (!root) return;

  const lines = root.querySelectorAll<HTMLElement>("[data-line][data-line-type]");
  for (const lineEl of lines) {
    if (lineEl.hasAttribute(SYNTH_LINE_ATTR)) continue;
    const type = lineEl.getAttribute("data-line-type") ?? "";
    const lineNum = Number(lineEl.getAttribute("data-line"));
    if (!Number.isFinite(lineNum)) continue;

    let spans: LineSpans | undefined;
    let side: "child" | "parent" | undefined;
    if (type === "change-deletion") {
      if (parentForFile) {
        spans = parentForFile.get(lineNum);
        side = "parent";
      }
    } else if (type === "change-addition") {
      if (childForFile) {
        spans = childForFile.get(lineNum);
        side = "child";
      }
    } else if (type === "context" || type === "context-expanded") {
      // Skip the deletions side of split mode: its `data-line` is the old
      // line number, which doesn't index into `childForFile` (keyed by
      // child_tree line numbers).
      const code = lineEl.closest("code");
      const isLeftSide = code?.hasAttribute("data-deletions") ?? false;
      if (!isLeftSide && childForFile) {
        spans = childForFile.get(lineNum);
        side = "child";
      }
    }

    if (!spans || spans.length === 0 || !side) continue;
    decorateLine(lineEl, spans, side);
    decorateGutter(root, lineEl);
  }
}

function decorateLine(lineEl: HTMLElement, spans: LineSpans, side: "child" | "parent") {
  lineEl.setAttribute(SYNTH_LINE_ATTR, side);
  // Process spans right-to-left so earlier offsets stay valid as the DOM is
  // mutated.
  const sorted = [...spans].sort((a, b) => b[0] - a[0]);
  for (const [start, end] of sorted) {
    if (end <= start) continue;
    wrapRange(lineEl, start, end, side);
  }
}

function wrapRange(
  lineEl: HTMLElement,
  startCol: number,
  endCol: number,
  side: "child" | "parent",
) {
  const doc = lineEl.ownerDocument ?? document;
  const walker = doc.createTreeWalker(lineEl, NodeFilter.SHOW_TEXT);
  let pos = 0;
  let startNode: Text | null = null;
  let startOff = 0;
  let endNode: Text | null = null;
  let endOff = 0;
  let n: Node | null;
  while ((n = walker.nextNode()) !== null) {
    const tn = n as Text;
    const len = tn.data.length;
    if (startNode === null && pos + len > startCol) {
      startNode = tn;
      startOff = startCol - pos;
    }
    if (pos + len >= endCol) {
      endNode = tn;
      endOff = endCol - pos;
      break;
    }
    pos += len;
  }
  if (!startNode || !endNode) return;
  const range = doc.createRange();
  try {
    range.setStart(startNode, startOff);
    range.setEnd(endNode, endOff);
  } catch {
    return;
  }
  let fragment: DocumentFragment;
  try {
    fragment = range.extractContents();
  } catch {
    return;
  }
  const wrapper = doc.createElement("span");
  wrapper.className = SYNTH_WRAP_CLASS;
  wrapper.setAttribute("data-side", side);
  wrapper.setAttribute("title", SYNTH_TOOLTIP[side]);
  wrapper.appendChild(fragment);
  range.insertNode(wrapper);
}

function decorateGutter(root: ShadowRoot, lineEl: HTMLElement) {
  const idx = lineEl.getAttribute("data-line-index");
  if (!idx) return;
  const selector = `[data-gutter] [data-line-index="${cssEscape(idx)}"]`;
  for (const cell of root.querySelectorAll<HTMLElement>(selector)) {
    cell.setAttribute(SYNTH_GUTTER_ATTR, "");
  }
}
