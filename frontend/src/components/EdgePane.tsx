import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { processFile, type FileDiffMetadata } from "@pierre/diffs";
import { FileDiff, Virtualizer } from "@pierre/diffs/react";
import { FileTree, useFileTree, useFileTreeSelection } from "@pierre/trees/react";
import { ChevronDownIcon, ChevronRightIcon } from "@primer/octicons-react";

import {
  ApiError,
  api,
  type FileBlob,
  type SynthesizedRanges,
} from "@/lib/api";
import {
  buildLookup,
  decorateFileContainer,
  FILEDIFF_CSS,
  type SpanLookup,
} from "@/lib/decorateSynthesized";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";
import { DiffPaneSkeleton } from "@/components/session/DiffPaneSkeleton";
import { useIsDesktop } from "@/lib/useIsDesktop";
import { cn, cssEscape } from "@/lib/utils";

type Props =
  | {
      sessionId: string;
      targetNodeId: string;
      fromTree?: undefined;
      toTree?: undefined;
      beforeRef?: undefined;
      afterRef?: undefined;
    }
  | {
      sessionId: string;
      targetNodeId?: undefined;
      fromTree: string;
      toTree: string;
      beforeRef?: string;
      afterRef?: string;
    };

type LoadedEdge = {
  diff: string;
  files: FileBlob[];
  synthesized: SynthesizedRanges;
};

type DiffStyle = "unified" | "split";
type Overflow = "scroll" | "wrap";

const FILE_DATA_ATTR = "data-edge-file-path";

export default function EdgePane(props: Props) {
  const { sessionId } = props;
  const targetNodeId = "targetNodeId" in props ? props.targetNodeId : undefined;
  const fromTree = "fromTree" in props ? props.fromTree : undefined;
  const toTree = "toTree" in props ? props.toTree : undefined;
  const beforeRef = "beforeRef" in props ? props.beforeRef : undefined;
  const afterRef = "afterRef" in props ? props.afterRef : undefined;
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
          .then((e) => ({
            diff: e.diff,
            files: e.files,
            synthesized: e.synthesized,
          }))
      : api
          .getDiff(sessionId, fromTree!, toTree!, beforeRef, afterRef)
          .then((d) => ({
            diff: d.diff,
            files: d.files,
            synthesized: d.synthesized,
          }));
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
  }, [sessionId, targetNodeId, fromTree, toTree, beforeRef, afterRef]);

  // Parse each file individually with `processFile`, handing it the full
  // `oldFile`/`newFile` blob contents so the library's "N unmodified lines"
  // expand chevrons can reveal context that's not in the small `-U3` patch.
  const fileDiffs: FileDiffMetadata[] = useMemo(() => {
    if (!edge || edge.diff.length === 0) return [];
    const byPath = new Map<string, FileBlob>();
    for (const f of edge.files) {
      if (f.newPath) byPath.set(f.newPath, f);
      if (f.oldPath) byPath.set(f.oldPath, f);
    }
    return splitDiffByFile(edge.diff)
      .flatMap((section) => {
        const { oldPath, newPath } = extractPathsFromGitHeader(section);
        const blob = byPath.get(newPath ?? "") ?? byPath.get(oldPath ?? "");
        const oldFile =
          blob?.oldContent != null
            ? {
                name: blob.oldPath ?? oldPath ?? "",
                contents: blob.oldContent,
              }
            : undefined;
        const newFile =
          blob?.newContent != null
            ? {
                name: blob.newPath ?? newPath ?? "",
                contents: blob.newContent,
              }
            : undefined;
        const meta = processFile(section, {
          oldFile,
          newFile,
          isGitDiff: true,
        });
        return meta ? [meta] : [];
      })
      .sort((a, b) => compareTreePaths(a.name, b.name));
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

  const selectedPaths = useFileTreeSelection(model);
  const activeFilePath = selectedPaths.at(-1) ?? null;

  const selectFileInTree = useCallback(
    (path: string) => {
      for (const p of model.getSelectedPaths()) {
        if (p !== path) model.getItem(p)?.deselect();
      }
      const item = model.getItem(path);
      if (item && !item.isSelected()) item.select();
      item?.focus();
    },
    [model],
  );

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
    return <DiffPaneSkeleton />;
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
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-b px-3 py-1.5 pr-12 md:pr-3">
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
      <Virtualizer
        className="flex-1 min-w-0 h-full overflow-y-auto overflow-x-hidden touch-pan-y"
        contentClassName="px-3"
      >
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
            if (inHeader) {
              toggleCollapsed(file.name);
              selectFileInTree(file.name);
            }
          };
          return (
            <div
              key={`${file.name}-${i}`}
              {...{ [FILE_DATA_ATTR]: file.name }}
              onClick={onWrapperClick}
              className={cn(
                "my-2 overflow-hidden rounded-md",
                activeFilePath === file.name && "border-2 border-link",
              )}
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
                      <ChevronRightIcon className="h-4 w-4" />
                    ) : (
                      <ChevronDownIcon className="h-4 w-4" />
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

// Mirrors `@pierre/trees`'s `defaultChildrenComparator` over full paths so
// the scroll list matches the tree's DFS walk. Duplicating ~15 lines is
// preferred over draining the order from the tree model after render, which
// would create a render-cycle dependency between the scroll view and the
// tree.
function compareTreePaths(a: string, b: string): number {
  const aSegs = a.split("/");
  const bSegs = b.split("/");
  const shared = Math.min(aSegs.length, bSegs.length);
  for (let d = 0; d < shared; d++) {
    const aSeg = aSegs[d];
    const bSeg = bSegs[d];
    if (aSeg === bSeg) continue;
    const aIsFolder = d < aSegs.length - 1;
    const bIsFolder = d < bSegs.length - 1;
    if (aIsFolder !== bIsFolder) return aIsFolder ? -1 : 1;
    const aDot = aSeg.charCodeAt(0) === 46;
    const bDot = bSeg.charCodeAt(0) === 46;
    if (aDot !== bDot) return aDot ? -1 : 1;
    return aSeg.toLowerCase().localeCompare(bSeg.toLowerCase());
  }
  return aSegs.length - bSegs.length;
}

// Splits a multi-file `git diff` blob into per-file sections at each
// `diff --git ` boundary. Anything before the first boundary (none in
// practice for our backend) is dropped.
function splitDiffByFile(diff: string): string[] {
  const sections: string[] = [];
  const re = /^diff --git /gm;
  let lastIdx = -1;
  let match: RegExpExecArray | null;
  while ((match = re.exec(diff)) !== null) {
    if (lastIdx >= 0) sections.push(diff.slice(lastIdx, match.index));
    lastIdx = match.index;
  }
  if (lastIdx >= 0) sections.push(diff.slice(lastIdx));
  return sections;
}

// Pulls the old/new paths from `diff --git a/<old> b/<new>`. Returns
// `undefined` for either side when the path can't be extracted (e.g. the
// rare quoted-path form used for filenames with spaces); callers fall back
// to rendering the file without blob-backed expansion in that case.
function extractPathsFromGitHeader(section: string): {
  oldPath?: string;
  newPath?: string;
} {
  const newlineIdx = section.indexOf("\n");
  const firstLine =
    newlineIdx >= 0 ? section.slice(0, newlineIdx) : section;
  const m = /^diff --git a\/(.+) b\/(.+)$/.exec(firstLine);
  if (!m) return {};
  return { oldPath: m[1], newPath: m[2] };
}
