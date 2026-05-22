# Product UI Brand Guidance

## 1. Core visual direction

A developer-oriented product surface should feel precise, developer-native, trustworthy, structured, and readable without becoming sterile. The strongest expression comes from:

- neutral-heavy UI;
- clear hierarchy;
- strict grids;
- high-contrast text;
- restrained use of green;
- code- and repository-native details;
- product-like cards, borders, tables, forms, and navigation;
- sparse expressive details, used only when they clarify the experience.

A useful rule of thumb: build the layout like a developer tool, then add one carefully chosen developer-native detail.

## 2. Product-facing brand attributes

The source guide defines five attributes. For a developer-oriented product UI, the most relevant interpretation is practical rather than marketing-oriented.

### 2.1 Nerdy

Nerdy means technically precise, detail-loving, and comfortable with developer-native concepts. It is not performative cleverness; it is earned fluency.

Copy implications:

- use exact language;
- use technical vocabulary where it helps;
- make technical language approachable;
- avoid vague marketing phrasing.

Design implications:

- use monospace and code-inspired elements where they are useful;
- use precise grids and mathematical proportions;
- make complexity understandable through hierarchy.

### 2.2 Confident

Confidence comes from putting the product and task forward. The UI should feel capable and high-quality without being flashy.

Copy implications:

- be clear and concise;
- explain problems and solutions directly;
- avoid inflated claims.

Design implications:

- use strong hierarchy;
- use consistent defaults;
- keep the primary task obvious.

### 2.3 Authentic

Authenticity means the interface feels like it was made for developers, not for generic SaaS positioning.

Copy implications:

- be conversational but precise;
- use short, plainspoken sentences;
- avoid unnecessary jargon and empty marketing language.

Design implications:

- use clean layouts and generous whitespace;
- create delight through interaction and utility, not decoration for decoration's sake;
- let product structure carry the design.

### 2.4 Empathetic

Empathy means prioritizing user comprehension and task completion.

Copy implications:

- optimize for readability;
- use "you" when it helps;
- write words that help people accomplish something.

Design implications:

- maintain contrast;
- keep type legible;
- use clear cues, symbols, empty states, and layouts;
- avoid relying on color alone.

## 3. Voice and tone

For a developer-oriented product site:

- sound technically credible, not inflated;
- prefer clarity over cleverness;
- use approachable precision;
- avoid generic marketing speak;
- make the reader's task easier.

## 4. Color

### 4.1 Overall color model

Use color for recognition, status, and emphasis. The system should be neutral-heavy, with green as the primary expressive thread.

The overall effect should be technical, sophisticated, serious, and uncomplicated. Secondary colors should not compete with green.

### 4.2 Primary palette

| Name                    | Hex       | Role                 |
| ----------------------- | --------- | -------------------- |
| Primary Green / Green 4 | `#0FBF3E` | Primary brand accent |
| Gray 1                  | `#F2F5F3` | Light neutral        |
| Gray 2                  | `#E4EBE6` | Light neutral        |
| Gray 3                  | `#B6BFB8` | Mid neutral          |
| Gray 4                  | `#909692` | Mid/dark neutral     |
| Gray 5                  | `#232925` | Dark neutral         |
| Gray 6                  | `#101411` | Near-black neutral   |
| Green 1                 | `#BFFFD1` | Light green          |
| Green 2                 | `#8CF2A6` | Light/mid green      |
| Green 3                 | `#5FED83` | Mid green            |
| Green 5                 | `#08872B` | Dark green           |
| Green 6                 | `#0A241B` | Very dark green      |

Recommended proportion: mostly neutrals, with small but meaningful green moments.

### 4.3 Color usage rules

Do:

- use neutral backgrounds for most product surfaces;
- use green sparingly for emphasis, confirmation, progress, or brand thread;
- keep text on high-contrast neutral surfaces;
- make status colors redundant with labels, icons, or shape.

Do not:

- make the UI feel like a generic colorful SaaS dashboard;
- use color as the only meaning carrier;
- let secondary colors compete with the main product hierarchy.

## 5. Typography

The type system should center on a clean sans family with a compatible monospace companion.

### 5.1 Font family

Use a modern sans family with optical sizing if available, multiple weights, and a matching monospace companion. Prefer open-source fonts where practical.

### 5.2 Type scale

Use a restrained scale with clear hierarchy:

- title styles for major page identity;
- headline styles for sections and cards;
- body styles for descriptions, labels, metadata, and help text.

Point sizes and line heights are starting points, not universal constants. Adapt them to the environment while preserving hierarchy, legibility, and relationships.

### 5.3 Typography usage rules

Do not:

- use alternate the chosen sans family width styles casually;
- use uppercase monospace for multiline text treatments;
- use ligatures in headlines or body copy if they distract or reduce readability;
- manually track/letterspace text unless there is a specific reason.

When unsure, optimize for readability and hierarchy.

## 6. Accessibility

Accessibility is a core part of the product feel, not a post-process.

### 6.1 Text and contrast

Required practices:

- use legible type pairings;
- do not arbitrarily rescale text sizes;
- meet at least WCAG AA contrast for all text;
- never rely on color alone to convey meaning;
- use restricted palettes with strong contrast;
- keep text and visuals distinctly separated where possible.

### 6.2 Charts and diagrams

Charts and diagrams should build trust. They should be legible, detailed, accessible, and clear.

Required practices:

- clear labels;
- sufficient text size;
- no color-only distinction between data series;
- alt text for complex visualizations;
- avoid gradients or decorative chart styling when it weakens clarity.

## 7. Diagrams, charts, and infographics

Developer-oriented diagrams and graphs should be accurate, informative, and trustworthy. Every line and color choice should feel deliberate.

Charts and graphs:

- prioritize detail, legibility, and accessibility;
- avoid illustration, gradients, and decorative adjacent color;
- use color for grouping only when essential.

Diagrams:

- should feel technical, clear, and instructional;
- may use restrained color blocking for clarity;
- should explain structure rather than decorate the page.

Infographics:

- can use icons, type, and simple layouts to communicate why a number matters;
- should still feel grounded, precise, and product-adjacent.

## 8. Iconography

The default icon language should be simple, geometric, and product-native. Icons should provide a throughline from product UI to surrounding brand surfaces.

Use the default icon set for:

- navigation-like contexts;
- product-adjacent iconography;
- labels, cards, buttons, metadata, and empty states;
- places where the design should feel close to the product surface.

Usage rules:

- keep icons simple and semantic;
- do not let icons dominate the layout;
- use restrained color;
- pair icons with text when meaning could be ambiguous.

## 9. Layouts

the product's layout style depends on strict grid adherence. The grid creates a technical, consistent feel.

### 9.1 Grids and margins

Start by deciding:

- the primary page division;
- the margin inside layout boxes;
- the relationship between navigation, content, sidebars, and cards.

Every later layout decision should feel consistent with those initial divisions.

### 9.2 Borders

The parent-level grid may be visible, but not all of it should be shown. Reveal the grid with borders selectively, then use the grid invisibly through whitespace.

Use borders for:

- cards;
- panels;
- tables;
- split layouts;
- sidebars;
- input groups;
- product UI containers.

Avoid excessive borders that make the page feel noisy.

### 9.3 Text and visuals

Text should sit on clear, accessible surfaces. Visuals can be expressive, but they should not compromise comprehension.

A common common product-style split:

- text represents product clarity, repositories, developer tools, and infrastructure;
- visuals represent programming, contribution, collaboration, and code.

All text should meet WCAG AA contrast at normal text size.

## 10. Product UI

Product UI is one of the strongest ways to create a product-native feel because it shows actual utility.

Before stylizing UI, define the story and focal point. Remove noise so the main takeaway is obvious.

### 10.1 Backgrounds

Product UI usually works best on neutral backgrounds. More expressive backgrounds should support focus and hierarchy, not compete with the interface.

### 10.2 Borders

Use borders and corner radius to separate UI from backgrounds. Stroke thickness should relate to the surrounding layout scale. Borders can be subtle, but should not disappear.

### 10.3 Simplify and emphasize

Do:

- trim to essential information;
- crop, zoom, or offset layers to emphasize the main point;
- remove unnecessary borders, sidebars, footers, code, buttons, and links;
- maintain recognizable product consistency;
- keep spacing consistent across multiple images or components.

Do not:

- heavily alter UI composition;
- make product screenshots or mockups unrecognizable;
- include irrelevant details that compete with the story.

### 10.4 Product UI accessibility

Add alt text to all UI images. If code or content is shown, the alt text should include enough of it to communicate the demo's meaning.

## 11. Activity-grid motifs

An activity grid can act as a recognizable developer-product motif. It can appear literally or abstractly in light branding contexts.

For restrained product-inspired branding, it can appear as a simple row or column of aligned squares.

Important: activity-grid motifs are easy to overuse. Keep them grid-aligned, subtle, and connected to the surrounding layout.

Do not:

- break the grid;
- scatter activity-grid squares randomly;
- use activity-grid squares as arbitrary decorative pixels;
- supersize individual contribution graph elements;
- mix too many texture systems together.

## 12. Web and design-system direction

the product's web presence expresses code-oriented, accessible technical design. For a developer-oriented site, prefer reusable design-system primitives over one-off visual invention.

Useful direction:

- make the UI feel intentional and technical;
- use consistent spacing, borders, radii, and type styles;
- prefer product-like components over marketing decoration;
- keep pages accessible by default;
- use product-native visual cues only where they support comprehension.

## 13. Motion

For a product-style site, motion should be restrained and useful.

Do:

- use motion to clarify state changes;
- keep transitions quick and legible;
- establish consistent rhythm and timing;
- add polish only when it improves comprehension.

Do not:

- create obstacles or distractions;
- reduce legibility or accessibility;
- use generic stock presentation effects;
- make motion the main visual idea.

## 14. Practical design heuristics

### 14.1 Start with the grid

A developer-oriented layout usually begins with a strict grid, visible or invisible. Decide the primary division and margins first, then place content. Borders can reveal the grid, but should not overwhelm the page.

### 14.2 Use green as the brand thread

Primary Green should be the primary expressive color in core product-inspired applications. Use secondary colors only when the topic or status meaning justifies them.

### 14.3 Keep text readable

Text belongs on neutral, high-contrast surfaces. Avoid overlaying copy on busy visuals. Separate copy and visuals structurally when possible.

### 14.4 Use product UI when it tells the story

Product-like UI is one of the strongest visual tools. Use it to demonstrate actual value. Remove noise. Preserve recognizability.

### 14.5 Use contribution-graph motifs carefully

Activity-grid squares are not arbitrary pixels. Keep them grid-aligned, subtle, and intentional.

### 14.6 Avoid generic SaaS polish

The design should be polished, but developer-native. Overly glossy, vague, or generic enterprise visuals lose the developer-native feel.

## 15. Quick rules checklist

### Do

- Use a clean sans / monospace pair.
- Use strict grids and clear hierarchy.
- Use Primary Green as the main accent.
- Keep most color neutral.
- Meet WCAG AA contrast.
- Use the default icon set as the default icon language.
- Use product-like UI to demonstrate actual value.
- Use diagrams and charts with precision and accessibility.
- Use motion to clarify and elevate, not distract.
- Use reusable design-system primitives where possible.

### Don't

- Create new base graphic systems casually.
- Rely on color alone for meaning.
- Put text over busy imagery.
- Use charts as decorative illustration.
- Scatter activity-grid squares randomly.
- Use stock-y motion effects.
- Let decorative expression compete with product clarity.
- Imitate protected the product marks as your own branding.

## 16. Condensed mental model

A strong developer-oriented product execution is:

- structured like code;
- precise like documentation;
- useful like product UI;
- warm like developer community;
- expressive only where expression adds meaning.

When in doubt: simplify, use the grid, preserve contrast, show useful product structure, and add one developer-native detail rather than five decorative ones.

## 17. Product UI tokens (eunomio implementation)

The eunomio frontend implements these guidelines with a concrete token system in `frontend/src/tokens.css`:

| Token / asset | Value / role |
| --- | --- |
| Canvas (Primer dark) | `#0d1117` background, `#161b22` elevated surfaces, `#30363d` borders |
| Text | `#e6edf3` primary, `#8b949e` muted |
| `--link` | `#58a6ff` — links, focus rings, interactive text |
| `--primary` | `#0FBF3E` (BRAND green) — affirmative submit buttons only |
| `--success` | `#3fb950` (Primer) — done/open lifecycle and status (not primary buttons) |
| `--attention` | `#d29922` — running/warning states, active tab underline |
| `--danger` | `#f85149` — errors, blocked states |
| `--synth` / `--synth-muted` | Purple — AI-synthesized diff hunks (eunomio-specific) |
| Typography | Mona Sans (`@fontsource/mona-sans`) |
| Icons | Octicons (`@primer/octicons-react`) |
| Header | `#010409` surface (`--header-bg`); tier 1: logo + `owner / repo` breadcrumb; session routes show mono `baseRef ← sourceRef` (emphasis on sourceRef); tier 2: underline **Session \| Settings** tabs |
| Settings page | Full-page layout at `/settings`; desktop: fixed left category sidebar + scrolling main; mobile: drill-down category index → detail with sticky back bar; **Subagents** group for Surveyor / Planner / Constructor |

Button policy: most actions use outline/bordered buttons; green primary is reserved for high-commit submits (e.g. Create session). Code diffs use the `github-dark` theme from `@pierre/diffs`.

---

## Source note

These guidelines are adapted from the GitHub style guide / GitHub Brand Guidelines 2026.
