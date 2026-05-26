# Session layout refactor

Needs premium review

**Status:** Follow-up

## Summary

Refactor [`Session.tsx`](../frontend/src/pages/Session.tsx) so stateful panes (`DiffPane`, `GraphPane`, tools) mount once instead of living in two parallel layout trees (desktop resizable split vs mobile tab bar).

The timeline hotkeys bug exposed duplicate `DiffPane` mounting: both the desktop layout (`md:flex`) and the mobile layout (`md:hidden`) stayed in the DOM on desktop, because CSS visibility does not unmount React children. The current fix gates each tree with `isDesktop`, but that duplicates the breakpoint in both Tailwind (`md:`) and `useIsDesktop()`.

## Problem

- Two structurally different layouts share the same pane JSX (`diffPane`, `graphPane`, …).
- CSS `hidden` / `md:hidden` hides UI but still mounts components.
- Stateful children (timeline registration, selection, scroll) break when duplicated.
- `isDesktop ? … : null` alongside `md:flex` / `md:hidden` is two sources of truth for the same viewport decision.

## Proposed direction

Extract `DesktopSessionLayout` and `MobileSessionLayout` (or equivalent) and render exactly one at the session root:

```tsx
{isDesktop ? (
  <DesktopSessionLayout diffPane={diffPane} graphPane={graphPane} … />
) : (
  <MobileSessionLayout diffPane={diffPane} graphPane={graphPane} … />
)}
```

- Single mount point per pane; no shared element reused in two places.
- Pick one breakpoint authority: either JS (`useIsDesktop`) or CSS, not both for mount decisions.
- Consider changing `TabPanel` to `{active ? children : null}` so inactive mobile tabs unmount (helps mobile; does not replace the desktop/mobile tree split).

Keep the hotkey `registerTimeline` owner-id guard as defense in depth.

## Acceptance criteria

- [ ] Exactly one `DiffPane` (and one timeline registration) mounted at any viewport size.
- [ ] No redundant `md:` show/hide on wrappers whose mount is already gated by `isDesktop`.
- [ ] Desktop resizable split and mobile tab bar behavior unchanged.
- [ ] Timeline hotkeys still work after refactor (manual or automated check).

## Related

- Timeline hotkeys: [`HotkeysProvider`](../frontend/src/components/HotkeysProvider.tsx), [`CanonicalEdgePane`](../frontend/src/components/session/CanonicalEdgePane.tsx)
- Mobile tabs: [`MobileTabBar.tsx`](../frontend/src/components/session/MobileTabBar.tsx)
