# Eunomia documentation site

User-facing docs built with [Fumadocs](https://fumadocs.dev) and Next.js. This file is for contributors only — it is not published on the site.

## Prerequisites

- Node.js 20+ (22+ recommended)
- npm

## Run locally

From the repo root:

```bash
npm run dev:docs
```

Or from this directory:

```bash
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000). The landing page is at `/`; documentation starts at `/docs`.

## Build

From the repo root:

```bash
npm run build:docs
```

Or from this directory:

```bash
npm run build
```

## Content

MDX pages live in `content/docs/`. Sidebar navigation is configured via `meta.json` in each section folder.

Site metadata (title, GitHub URL) is in `site.config.ts`.
