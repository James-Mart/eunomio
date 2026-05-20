import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { parsePatchFiles, type FileDiffMetadata } from "@pierre/diffs";
import { FileDiff, Virtualizer } from "@pierre/diffs/react";
import { FileTree, useFileTree } from "@pierre/trees/react";
import { ChevronDown, ChevronRight } from "lucide-react";

import {
  ApiError,
  api,
  type FileLineRanges,
  type SynthesizedRanges,
} from "@/lib/api";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

type Props =
  | {
      sessionId: string;
      targetNodeId: string;
      fromTree?: undefined;
      toTree?: undefined;
      referenceTree?: undefined;
    }
  | {
      sessionId: string;
      targetNodeId?: undefined;
      fromTree: string;
      toTree: string;
      referenceTree?: string;
    };

type LoadedEdge = { diff: string; synthesized: SynthesizedRanges };

type DiffStyle = "unified" | "split";
type Overflow = "scroll" | "wrap";

const FILE_DATA_ATTR = "data-edge-file-path";

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

const FILEDIFF_CSS = STICKY_HEADER_CSS + SYNTHESIZED_CSS;

type SpanLookup = Map<string, Map<number, ReadonlyArray<readonly [number, number]>>>;

function buildLookup(files: FileLineRanges[]): SpanLookup {
  const m: SpanLookup = new Map();
  for (const f of files) {
    const lines = new Map<number, ReadonlyArray<readonly [number, number]>>();
    for (const l of f.lines) lines.set(l.line, l.spans);
    m.set(f.path, lines);
  }
  return m;
}

const SYNTH_TOOLTIP = {
  child: "Synthesized — a later Edge overwrites this content.",
  parent: "Synthesized removal — a later Edge restores this content.",
} as const;

type LineSpans = ReadonlyArray<readonly [number, number]>;

// `@pierre/diffs` renders each `<FileDiff />` into a custom element with its
// own shadow root. Decorations have to be applied inside that root since
// styles and DOM from outside don't reach in. The library exposes
// `onPostRender(node, instance)` after each render pass; we walk
// `node.shadowRoot` and wrap synthesized character ranges in spans tagged with
// `.eunomia-synthesized`, then mark matching gutter cells.
function decorateFileContainer(
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

export default function EdgePane(props: Props) {
  const { sessionId } = props;
  const targetNodeId = "targetNodeId" in props ? props.targetNodeId : undefined;
  const fromTree = "fromTree" in props ? props.fromTree : undefined;
  const toTree = "toTree" in props ? props.toTree : undefined;
  const referenceTree =
    "referenceTree" in props ? props.referenceTree : undefined;
  const [edge, setEdge] = useState<LoadedEdge | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("unified");
  const [overflow, setOverflow] = useState<Overflow>("scroll");
  const [collapsedFiles, setCollapsedFiles] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [pendingScrollTo, setPendingScrollTo] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  // Held in a ref so onPostRender callbacks always read the freshest lookup
  // without forcing FileDiff option identity to change across edge fetches.
  const lookupRef = useRef<{ child: SpanLookup; parent: SpanLookup }>({
    child: new Map(),
    parent: new Map(),
  });

  const toggleCollapsed = useCallback((path: string) => {
    setCollapsedFiles((prev) => {
      const willCollapse = !prev.has(path);
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      if (willCollapse) setPendingScrollTo(path);
      return next;
    });
  }, []);

  useEffect(() => {
    if (!pendingScrollTo) return;
    const root = rootRef.current;
    if (root) {
      const el = root.querySelector<HTMLElement>(
        `[${FILE_DATA_ATTR}="${cssEscape(pendingScrollTo)}"]`,
      );
      el?.scrollIntoView({ block: "start" });
    }
    setPendingScrollTo(null);
  }, [pendingScrollTo, collapsedFiles]);

  useEffect(() => {
    let cancelled = false;
    setEdge(null);
    setError(null);
    const fetch = targetNodeId !== undefined
      ? api
          .getEdge(sessionId, targetNodeId)
          .then((e) => ({ diff: e.diff, synthesized: e.synthesized }))
      : api
          .getDiff(sessionId, fromTree!, toTree!, referenceTree)
          .then((d) => ({ diff: d.diff, synthesized: d.synthesized }));
    fetch
      .then((e) => {
        if (!cancelled) setEdge(e);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(
          e instanceof ApiError || e instanceof Error
            ? e.message
            : "Failed to load diff",
        );
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, targetNodeId, fromTree, toTree, referenceTree]);

  const fileDiffs: FileDiffMetadata[] = useMemo(() => {
    if (!edge || edge.diff.length === 0) return [];
    const parsed = parsePatchFiles(edge.diff);
    return parsed.flatMap((p) => p.files);
  }, [edge]);

  useEffect(() => {
    lookupRef.current = {
      child: buildLookup(edge?.synthesized.child ?? []),
      parent: buildLookup(edge?.synthesized.parent ?? []),
    };
  }, [edge]);

  const onFileDiffPostRender = useCallback(
    (file: FileDiffMetadata) => (node: HTMLElement) => {
      const childForFile = lookupRef.current.child.get(file.name);
      const parentForFile = lookupRef.current.parent.get(file.prevName ?? file.name);
      if (!childForFile && !parentForFile) return;
      decorateFileContainer(node, childForFile, parentForFile);
    },
    [],
  );

  const paths = useMemo(() => fileDiffs.map((f) => f.name), [fileDiffs]);

  const scrollFileIntoView = useCallback((path: string) => {
    const root = rootRef.current;
    if (!root) return;
    const el = root.querySelector<HTMLElement>(
      `[${FILE_DATA_ATTR}="${cssEscape(path)}"]`,
    );
    el?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, []);

  const handleSelectionChange = useCallback(
    (selected: readonly string[]) => {
      const path = selected[selected.length - 1];
      if (path) scrollFileIntoView(path);
    },
    [scrollFileIntoView],
  );

  const { model } = useFileTree({
    paths,
    search: true,
    initialExpansion: "open",
    onSelectionChange: handleSelectionChange,
  });

  useEffect(() => {
    model.resetPaths(paths);
  }, [model, paths]);

  const treeSplitLayout = useDefaultLayout({
    id: "edge-pane-tree-split-v1",
    panelIds: ["tree", "diff"],
  });

  const isDesktop = useIsDesktop();

  if (error) {
    return (
      <div className="flex h-full items-center p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  if (!edge) {
    return <EdgePaneSkeleton />;
  }

  if (fileDiffs.length === 0) {
    const message = targetNodeId !== undefined ? "No diff — this is the base Node." : "No diff.";
    return (
      <div className="flex h-full items-center justify-center p-4 text-sm text-muted-foreground">
        {message}
      </div>
    );
  }

  const diffBody = (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-b py-1.5 pl-2 pr-12 md:pr-2">
        <SegmentedToggle
          value={diffStyle}
          onChange={setDiffStyle}
          options={[
            { value: "unified", label: "Unified" },
            { value: "split", label: "Split" },
          ]}
        />
        <SegmentedToggle
          value={overflow}
          onChange={setOverflow}
          options={[
            { value: "scroll", label: "Scroll" },
            { value: "wrap", label: "Wrap" },
          ]}
        />
      </div>
      <Virtualizer className="flex-1 min-w-0 h-full overflow-y-auto overflow-x-hidden touch-pan-y">
        {fileDiffs.map((file, i) => {
          const isCollapsed = collapsedFiles.has(file.name);
          const onWrapperClick = (e: React.MouseEvent) => {
            const path = e.nativeEvent.composedPath();
            const inHeader = path.some(
              (n) =>
                n instanceof Element &&
                typeof n.matches === "function" &&
                n.matches("[data-diffs-header]"),
            );
            if (inHeader) toggleCollapsed(file.name);
          };
          return (
            <div
              key={`${file.name}-${i}`}
              {...{ [FILE_DATA_ATTR]: file.name }}
              onClick={onWrapperClick}
            >
              <FileDiff
                fileDiff={file}
                options={{
                  theme: "github-dark",
                  diffStyle,
                  overflow,
                  collapsed: isCollapsed,
                  unsafeCSS: FILEDIFF_CSS,
                  onPostRender: onFileDiffPostRender(file),
                }}
                renderHeaderPrefix={() => (
                  <span
                    aria-hidden="true"
                    className="inline-flex h-5 w-5 items-center justify-center text-muted-foreground"
                  >
                    {isCollapsed ? (
                      <ChevronRight className="h-4 w-4" />
                    ) : (
                      <ChevronDown className="h-4 w-4" />
                    )}
                  </span>
                )}
              />
            </div>
          );
        })}
      </Virtualizer>
    </div>
  );

  return (
    <div ref={rootRef} className="h-full min-h-0 w-full">
      <ResizablePanelGroup
        orientation="horizontal"
        defaultLayout={treeSplitLayout.defaultLayout}
        onLayoutChanged={treeSplitLayout.onLayoutChanged}
        className="h-full"
      >
        {isDesktop && (
          <>
            <ResizablePanel
              id="tree"
              defaultSize="16rem"
              minSize="10rem"
              maxSize="40%"
              className="min-w-0 border-r"
            >
              <FileTree model={model} className="h-full min-w-0" />
            </ResizablePanel>
            <ResizableHandle
              withHandle
              aria-label="Resize file tree"
              className="mx-1"
            />
          </>
        )}
        <ResizablePanel id="diff" minSize="30%" className="min-w-0">
          {diffBody}
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

function SegmentedToggle<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (next: T) => void;
  options: ReadonlyArray<{ value: T; label: string }>;
}) {
  return (
    <div
      role="radiogroup"
      className="inline-flex rounded-md border bg-muted p-0.5 text-xs"
    >
      {options.map((o) => {
        const active = value === o.value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(o.value)}
            className={cn(
              "h-7 rounded-sm px-2.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              active
                ? "bg-background text-foreground shadow-sm ring-1 ring-border"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

function EdgePaneSkeleton() {
  return (
    <div className="flex h-full w-full">
      <div className="hidden md:flex w-64 shrink-0 flex-col gap-2 border-r p-3">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton
            key={i}
            className="h-4"
            style={{ width: `${60 + ((i * 13) % 35)}%` }}
          />
        ))}
      </div>
      <div className="flex-1 min-w-0 space-y-4 p-3">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="space-y-2">
            <Skeleton className="h-6 w-1/3" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
            <Skeleton className="h-4 w-2/3" />
          </div>
        ))}
      </div>
    </div>
  );
}

function cssEscape(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value);
  }
  return value.replace(/(["\\])/g, "\\$1");
}

const DESKTOP_QUERY = "(min-width: 768px)";

function useIsDesktop(): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window !== "undefined"
      ? window.matchMedia(DESKTOP_QUERY).matches
      : true,
  );
  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia(DESKTOP_QUERY);
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);
  return matches;
}
