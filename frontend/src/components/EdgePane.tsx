/* SPDX-License-Identifier: Apache-2.0 */

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { processFile, type FileDiffMetadata } from "@pierre/diffs";
import { FileDiff, Virtualizer } from "@pierre/diffs/react";
import {
  FileTree,
  useFileTree,
  useFileTreeSelection,
} from "@pierre/trees/react";
import { ChevronDownIcon, ChevronRightIcon } from "@primer/octicons-react";

import {
  ApiError,
  api,
  type Diff,
  type FileBlob,
  type SynthesizedRanges,
} from "@/lib/api";
import {
  buildLookup,
  decorateFileContainer,
  FILEDIFF_CSS,
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

type ViewedProps = {
  viewedPaths?: ReadonlySet<string>;
  onToggleViewed?: (path: string, viewed: boolean) => void;
  header?: ReactNode;
  footer?: ReactNode;
};

type Props = ViewedProps &
  (
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
        loadedEdge?: Diff;
      }
  );

type LoadedEdge = {
  diff: string;
  files: FileBlob[];
  synthesized: SynthesizedRanges;
};

type DiffStyle = "unified" | "split";
type Overflow = "scroll" | "wrap";

const FILE_DATA_ATTR = "data-edge-file-path";

export default function EdgePane(props: Props) {
  const { sessionId, viewedPaths, onToggleViewed, header, footer } = props;
  const targetNodeId = "targetNodeId" in props ? props.targetNodeId : undefined;
  const fromTree = "fromTree" in props ? props.fromTree : undefined;
  const toTree = "toTree" in props ? props.toTree : undefined;
  const beforeRef = "beforeRef" in props ? props.beforeRef : undefined;
  const afterRef = "afterRef" in props ? props.afterRef : undefined;
  const loadedEdge = "loadedEdge" in props ? props.loadedEdge : undefined;
  const [fetchedEdge, setFetchedEdge] = useState<LoadedEdge | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("unified");
  const [overflow, setOverflow] = useState<Overflow>("scroll");
  const [collapsedFiles, setCollapsedFiles] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [pendingScrollTo, setPendingScrollTo] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

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
    if (loadedEdge) {
      setFetchedEdge(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setFetchedEdge(null);
    setError(null);
    const fetch =
      targetNodeId !== undefined
        ? api.getEdge(sessionId, targetNodeId).then((e) => ({
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
        if (!cancelled) setFetchedEdge(e);
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
  }, [
    sessionId,
    targetNodeId,
    fromTree,
    toTree,
    beforeRef,
    afterRef,
    loadedEdge,
  ]);

  const edge = loadedEdge ?? fetchedEdge;

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

  const synthesizedLookup = useMemo(
    () => ({
      child: buildLookup(edge?.synthesized.child ?? []),
      parent: buildLookup(edge?.synthesized.parent ?? []),
    }),
    [edge],
  );

  const onFileDiffPostRender = useCallback(
    (file: FileDiffMetadata) => (node: HTMLElement) => {
      const childForFile = synthesizedLookup.child.get(file.name);
      const parentForFile = synthesizedLookup.parent.get(
        file.prevName ?? file.name,
      );
      if (!childForFile && !parentForFile) return;
      decorateFileContainer(node, childForFile, parentForFile);
    },
    [synthesizedLookup],
  );

  const paths = useMemo(() => fileDiffs.map((f) => f.name), [fileDiffs]);

  const viewedPathsInDiff = useMemo(() => {
    if (!viewedPaths) return undefined;
    const inDiff = new Set(paths);
    return new Set([...viewedPaths].filter((p) => inDiff.has(p)));
  }, [viewedPaths, paths]);

  const viewedInDiff = viewedPathsInDiff?.size ?? 0;

  useEffect(() => {
    if (!viewedPathsInDiff || viewedPathsInDiff.size === 0) return;
    setCollapsedFiles((prev) => {
      let changed = false;
      const next = new Set(prev);
      for (const p of viewedPathsInDiff) {
        if (!next.has(p)) {
          next.add(p);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [viewedPathsInDiff]);

  const handleToggleViewed = useCallback(
    (path: string, viewed: boolean) => {
      onToggleViewed?.(path, viewed);
      setCollapsedFiles((prev) => {
        const next = new Set(prev);
        if (viewed) {
          next.add(path);
          setPendingScrollTo(path);
        } else {
          next.delete(path);
        }
        return next;
      });
    },
    [onToggleViewed],
  );

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
    (path: string, options?: { focus?: boolean }) => {
      const focus = options?.focus ?? true;
      for (const p of model.getSelectedPaths()) {
        if (p !== path) model.getItem(p)?.deselect();
      }
      const item = model.getItem(path);
      if (item && !item.isSelected()) item.select();
      if (focus) item?.focus();
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
    const message =
      targetNodeId !== undefined
        ? "No diff — this is the base Node."
        : "No diff.";
    return (
      <div className="flex h-full items-center justify-center p-4 text-sm text-muted-foreground">
        {message}
      </div>
    );
  }

  const diffBody = (
    <div className="flex h-full min-w-0 flex-col">
      {header}
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-b px-3 py-1.5 pr-12 md:pr-3">
        {viewedPathsInDiff && paths.length > 0 ? (
          <span className="text-xs text-muted-foreground tabular-nums">
            {viewedInDiff}/{paths.length} viewed
          </span>
        ) : null}
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
        className="flex-1 min-w-0 h-full overflow-y-auto overflow-x-hidden overscroll-y-contain touch-pan-y"
        contentClassName="px-3"
      >
        {fileDiffs.map((file, i) => {
          const isCollapsed = collapsedFiles.has(file.name);
          const isViewed = viewedPathsInDiff?.has(file.name) ?? false;
          const onWrapperClick = (e: React.MouseEvent) => {
            const path = e.nativeEvent.composedPath();
            const clickedViewed = path.some(
              (n) =>
                n instanceof Element &&
                typeof n.closest === "function" &&
                n.closest("[data-edge-viewed-control]") !== null,
            );
            const inMetadata = path.some(
              (n) =>
                n instanceof Element &&
                typeof n.closest === "function" &&
                n.closest("[data-metadata]") !== null,
            );
            const inHeader = path.some(
              (n) =>
                n instanceof Element &&
                typeof n.matches === "function" &&
                n.matches("[data-diffs-header]"),
            );
            if (onToggleViewed && inMetadata && !clickedViewed) {
              handleToggleViewed(file.name, !isViewed);
              selectFileInTree(file.name, { focus: false });
              return;
            }
            if (inHeader && !clickedViewed) {
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
                "my-2 rounded-md",
                activeFilePath === file.name && "border-2 border-link",
                isViewed && "opacity-80",
              )}
            >
              <FileDiff
                key={
                  fromTree != null && toTree != null
                    ? `${fromTree}-${toTree}-${file.name}`
                    : undefined
                }
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
                renderHeaderMetadata={() =>
                  onToggleViewed ? (
                    <div
                      data-edge-viewed-control
                      role="checkbox"
                      aria-checked={isViewed}
                      aria-label={
                        isViewed
                          ? `Mark ${file.name} as not viewed`
                          : `Mark ${file.name} as viewed`
                      }
                      className="flex h-full min-h-full flex-1 cursor-pointer items-center justify-end gap-1.5 self-stretch pl-2 text-xs text-muted-foreground"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleToggleViewed(file.name, !isViewed);
                        selectFileInTree(file.name, { focus: false });
                      }}
                      onMouseDown={(e) => e.preventDefault()}
                      onKeyDown={(e) => {
                        e.stopPropagation();
                        if (e.key === " " || e.key === "Enter") {
                          e.preventDefault();
                          handleToggleViewed(file.name, !isViewed);
                          selectFileInTree(file.name, { focus: false });
                        }
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={isViewed}
                        readOnly
                        tabIndex={-1}
                        aria-hidden="true"
                        className="pointer-events-none h-4 w-4 rounded border-border"
                      />
                      Viewed
                    </div>
                  ) : null
                }
              />
            </div>
          );
        })}
      </Virtualizer>
      {footer}
    </div>
  );

  return (
    <div ref={rootRef} className="h-full min-h-0 w-full overflow-hidden">
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
  const firstLine = newlineIdx >= 0 ? section.slice(0, newlineIdx) : section;
  const m = /^diff --git a\/(.+) b\/(.+)$/.exec(firstLine);
  if (!m) return {};
  return { oldPath: m[1], newPath: m[2] };
}
