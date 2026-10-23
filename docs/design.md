# OpenChat UI design system

## Brand mark

OpenChat's mark uses three rounded, separated forms arranged around an open
center. Two graphite forms and one copper form make one flat, high-contrast
symbol. The mark began as an ImageGen concept at
`apps/desktop/assets/brand/source/openchat-imagegen-concept-v2.png` and was
traced into clean SVG paths for desktop assets. Do not add terminal or
developer-tool motifs, gradients, shadows, or extra surrounding badges.

Use `apps/desktop/assets/brand/logo.svg` on light surfaces and
`apps/desktop/assets/brand/logo-dark.svg` on dark surfaces. The horizontal
wordmark in `apps/desktop/assets/brand/variants/openchat-horizontal.svg` is for
documents and external presentation; the desktop UI uses the symbol alone.
Keep the mark readable at 16, 32, and 48 pixels. Use the mark without the
wordmark in app-icon spaces; do not shrink the horizontal lockup. The Tauri
bundle icon is generated from `apps/desktop/assets/brand/app-icon.svg` so its
warm light tile remains visible on both light and dark taskbars.

The wordmark uses a two-tone OpenChat name and specifies Source Sans 3 to match
the product's interface typography, followed by system sans-serif fallbacks
for external renderers. Use the provided SVG assets instead of recreating the
symbol or wordmark with text and CSS.

The NSIS installer uses `apps/desktop/assets/brand/installer/header.bmp` and
`sidebar.bmp`, with corresponding SVG source files in the same directory. Keep
the header at 150 × 57 pixels and the sidebar at 164 × 314 pixels when editing
these assets.

This document is the required implementation contract for every React UI
change. A new screen or component must follow these rules from its first
implementation; visual similarity to an existing screen does not excuse a
parallel control or an unreviewed style value.

## Product and visual direction

OpenChat is a desktop AI workspace for conversation, projects, model discovery,
and settings. Chat is the primary workflow. Projects group related
conversations, and model discovery stays separate from day-to-day conversation
controls.

Use OpenChat's neutral surfaces and restrained copper accent to establish a
clear hierarchy. Design for the task and its real content states, not for
decoration. Avoid generic dashboard layouts, gradients, unnecessary card grids,
and icons that do not explain an action or object.

The default page hierarchy is:

1. Application navigation and the current destination.
2. The page's primary content and primary action.
3. Related controls, status, and supporting details.

Keep one clear primary action per working surface. Place secondary actions near
the content they affect. Empty, loading, unavailable, and error states must
describe the actual state instead of showing sample or fabricated product data.

## Mandatory component rules

These are required rules, not suggestions:

1. **Option selection uses a shared OpenChat control.** Import `Select` from
   `@/design-system` by default for controls that choose one value from a
   finite list. Native HTML `<select>` elements and ad-hoc select popovers are
   prohibited. A named specialized picker may be used when its options need a
   richer presentation, such as permission choices with descriptions; it must
   use the shared dropdown surface and preserve single-selection semantics,
   keyboard navigation, focus, and dismissal. Use `Tabs` or
   `SegmentedControl` when choices switch between peer views or modes, and
   `Switch` for a binary setting; do not use a dropdown for those cases.
2. **Use shared controls.** Use `Button`, `Input`, `Select`, `Switch`, `Tabs`,
   `SegmentedControl`, `Dialog`, `Tooltip`, `DropdownMenu`, `Card`, and
   `ScrollArea` from `@/design-system` where their behavior fits. Radix UI
   primitives provide focus management, keyboard navigation, dismissal, and
   accessible semantics for selects, tabs, dialogs, tooltips, and dropdown
   menus. Do not fork their border, focus, keyboard, disabled, or motion
   behavior in a feature component.
   Extend the shared primitive when a real product-wide behavior is missing.
3. **Use the `@/` import alias.** Frontend code must not use relative parent
   imports such as `../` or `../../` to cross module boundaries. Keep shared
   primitives in `apps/desktop/frontend/src/design-system`; keep feature
   presentation and data with their owning module.
4. **Use named design tokens.** Use the color, type, spacing, radius, and
   layout tokens defined in
   `apps/desktop/frontend/src/design-system/styles/tokens.css`. When a repeated
   or product-specific value is missing, add a clearly named token or semantic
   CSS class instead of embedding the value at the call site.
5. **Do not add static inline styles.** Static layout and appearance belong in
   named Tailwind tokens or semantic CSS classes. Inline style is allowed only
   for runtime values that cannot be represented as a class, such as a value
   derived from measured content or a live chart measurement.
6. **Respect the shared motion and accessibility behavior.** Reuse the
   primitive's keyboard interaction, focus treatment, dismissal, and reduced
   motion behavior. A custom control requires the same keyboard, pointer,
   touch, disabled, and dismissal support before it can replace a shared
   primitive. Preserve semantic names and states for controls.
7. **Share dropdown surfaces.** Use `Select` for choosing one value and
   `DropdownMenu` for actions. Their content panels use the shared
   `openchat-dropdown-surface` class. A specialized dropdown may use custom
   content and interaction when it needs richer options, but it must keep the
   same surface class and preserve the appropriate accessible semantics and
   keyboard behavior. Customize placement, width, spacing, and content in the
   owning component; change the shared glass surface only through a named
   design-system token or documented variant.
8. **Share selection triggers.** `Select` and custom value pickers use the
   `openchat-dropdown-trigger` class for height, border, alignment, hover,
   focus, and disabled states. Use `openchat-dropdown-trigger--subtle` for
   embedded controls that need a transparent resting fill, and
   `openchat-dropdown-trigger--compact` only in a genuinely dense layout.
   Action-menu buttons and search inputs keep their own control semantics.

If a shared primitive cannot express a real product need, change the shared
primitive or propose a named design-system extension. Do not silently create a
second control with different behavior.

## Enforced styling rules

- Tailwind arbitrary values are forbidden in UI classes. This includes
  `text-[13px]`, arbitrary colors, spacing, dimensions, grid tracks, radii, and
  arbitrary CSS properties. Do not work around the rule with a different
  arbitrary-value spelling.
- Use the existing font-size scale (`text-xs`, `text-sm`, `text-base`,
  `text-body`, and named semantic sizes such as `text-settings-description`).
  Add or reuse a named token when the design needs a size not on that scale.
- Use semantic palette utilities such as `bg-background`, `bg-surface`,
  `text-text`, `text-secondary-text`, `border-border`, `bg-accent`, and
  `focus-visible:ring-focus`. Do not introduce page-specific hex/RGB values or
  raw palette colors such as `text-slate-300` for product UI.
- Static responsive values must use named breakpoints and tokens. Add a named
  breakpoint or layout token when an existing one does not describe the
  content constraint.
- Do not use one-off negative spacing or custom utility classes to bypass a
  shared component's contract. Prefer a semantic class whose name describes
  the product role.

Keep design-system CSS separated by role: theme and layout values in
`styles/tokens.css`, text and rendered Markdown in `styles/typography.css`,
motion in `styles/animations.css`, and shared primitive styling in
`styles/components.css`. Keep app-shell and feature layout in `src/index.css`
or the owning feature stylesheet.

The frontend's pinned Biome configuration enables the recommended lint preset
and the project's additional strict rules. It rejects unsafe TypeScript type
assertions, non-null assertions, banned TypeScript types, incomplete React hook
dependencies, accumulating spread operations, Tailwind arbitrary values, and
native `<select>` usage. It also rejects dangerous HTML sinks, array-index
React keys, direct prototype-method calls, and global `isNaN` or `isFinite`
checks. TypeScript enables `noUncheckedIndexedAccess`, so array and index-signature
reads must account for absent values, and `noImplicitOverride` so inherited class
members must mark intentional overrides explicitly. Use runtime guards or
explicit types instead of asserting a value's type; use mutable accumulators when
building arrays or objects in a loop. The custom native select rule is kept in
`apps/desktop/frontend/plugins/no-native-select.grit`; its diagnostic directs
contributors to `Select` from `@/design-system`. Run `npx biome check` from
`apps/desktop/frontend` before submitting frontend changes. Windows CI runs the
same formatter and lint checks. Other semantic-token requirements in this
document are reviewed as part of code review. Biome currently classifies the
arbitrary-value and type-assertion checks as nursery rules, so revalidate their
behavior before changing the pinned Biome version.

## Color roles

Use CSS variables and the semantic Tailwind colors rather than copying these
values into components. The standard light and dark palettes are:

| Role | Light | Dark |
| --- | --- | --- |
| Canvas / background | `#F4F5F6` | `#15171A` |
| Navigation | `#EBEDF0` | `#1A1D21` |
| Surface | `#FAFBFC` | `#202328` |
| Raised / overlay surface | `#FFFFFF` | `#282C31` |
| Composer | `#F1F3F5` | `#24282D` |
| Selected | `#E3E7EB` | `#343940` |
| Hover | `#EDF0F2` | `#2B3036` |
| Border | `#CCD1D7` | `#3D434B` |
| Primary text | `#23272C` | `#F0F2F3` |
| Secondary text | `#56606A` | `#B6BCC4` |
| Secondary icon | `#68727D` | `#AAB1BA` |
| Accent | `#8E563B` | `#D08B68` |

Destructive, warning, success, informational, focus-ring, and high-contrast
colors are defined with the theme variables in
`apps/desktop/frontend/src/design-system/styles/tokens.css`. `Canvas` maps to
the page background, `Surface` to the base panel, `Raised` to floating panels,
and `Overlay` aliases the raised surface for portalled controls. Theme classes
can override the standard palette. Use these semantic roles so theme changes
remain consistent across the application.

## Typography

- Interface: Source Sans 3 variable font, weights 400, 500, 600, and 700.
- Code and technical identifiers: Source Code Pro, weights 400 and 600.
- Conversation: 16 px / 26 px.
- Page title: 24 px / 32 px.
- Fresh-chat welcome title: 28 px / 36 px.
- Section title: 20 px / 28 px.
- Component title: 16 px / 24 px.
- Body: 15 px / 22 px.
- Metadata: 12 px / 16 px minimum.
- Code: 13 px / 20 px.

Use sentence case and localized product copy with correct Turkish casing. Keep
labels concise and avoid artificial letter spacing. The user-controlled
response font setting changes conversation content; interface labels use
Source Sans 3. Use the configured font-size tokens so text scaling remains
available to users.

All app-authored text shown to users, including empty and error states, tool
descriptions, native permission dialogs, and accessible names, must come from
the localization catalogs. Keep protocol identifiers, provider-supplied text,
and diagnostic logs outside UI copy; map application errors to stable codes and
localized messages before rendering them.

## Dimensions and surfaces

- Spacing scale: 4, 8, 12, 16, 24, 32, 40, and 48 px.
- Form controls: 8 px radius; buttons: 12 px radius. Menus use their shared
  component radius.
- Cards and panels: 12 px radius; the composer: 20 px radius.
- Dialogs: 16 px radius.
- Standard icon: 20 px; inline icon: 16 px; primary action icon: 24 px.
- Desktop control height: 40 px; touch targets: at least 44 px.
- Conversation reading width: 920 px maximum; composer width: 800 px maximum.

Use shared surface, border, radius, and motion tokens from
`apps/desktop/frontend/src/design-system/styles/tokens.css`. Keep elevated effects restrained, honor
reduced motion, and make loading feedback understandable without animation.

Dropdown panels share a glass surface defined by `openchat-dropdown-surface` in
`styles/components.css`. The surface uses the `--dropdown-surface-alpha`,
`--dropdown-backdrop-blur`, `--dropdown-backdrop-saturation`, and
`--dropdown-surface-shadow` tokens, with a shared border and radius from
`--dropdown-surface-radius`. Selects, action menus, and specialized
dropdowns may arrange their contents differently, while keeping this shared
surface. High-contrast and forced-color modes use an opaque raised surface and
remove backdrop filtering and shadows.

Use `Select` for finite single-value controls, `DropdownMenu` for action menus,
and a named specialized picker only when its content needs richer rows, search,
or descriptions. The project picker is a searchable action menu; the model and
permission pickers are specialized value pickers. Title-bar and slash-command
suggestions keep combobox/listbox semantics and share the panel surface. Context
usage and statistics filters are floating information/filter panels, not value
selectors. In-chat search is a floating tool panel, not a selector. Static model
search results, tabs, switches, and disclosure sections remain in their
existing layouts instead of becoming dropdowns. The conversation-row menu may
keep its custom viewport-aware positioning, but it uses the shared panel
surface and preserves menu keyboard semantics.

## Navigation and responsive layout

Primary navigation is Chats, Models, and Settings. Local models are reached
from Models and local-engine settings so model management does not compete
with the main conversation destinations.

On Windows, place page back and forward controls in the custom title bar,
followed by the sidebar toggle. Give the title bar 8 px of vertical padding
within its 48 px height, and leave 8 px between these navigation controls. The
navigation history covers Chats, Models, Local Models, and Settings; it does
not track conversation selection or subsections within Settings. Keep the
sidebar toggle in the navigation rail on other platforms.

Use Lucide icons consistently. Keep primary navigation icon-only at every
window width, give its 44 px controls an 8 px horizontal inset for a 60 px rail,
and show localized labels in tooltips and semantic names. Use distinct
selected and unselected glyphs for toggle states such as pinning and favorites.
Keep selected state visible with shape or fill as well as accent color.

Design every surface responsively. Reflow headers and action groups at narrow
widths instead of clipping. Preserve reading order, control access, and
essential information at narrow and wide mobile widths, tablet widths, desktop
widths, intermediate sizes, text enlargement, and zoom. Use content-driven
breakpoints and intentional mobile layouts; do not hide failures with fixed
widths or unintended horizontal scrolling.

For the conversation sidebar, keep its header sticky, with New chat on the
left and search on the right. Open message search in a modal with the input in
its header and results in its scrollable body. Keep search filters and saved
searches available in the modal. Keep each expanded category's contents
independently scrollable up to 280 px while the sidebar body scrolls between
categories. Show a bottom border while the list is scrolled. Show conversation
titles without provider or model route labels, and show batch-selection
controls only while selection mode is active.

## Interaction and accessibility

- Keep keyboard focus visible with a quiet surface highlight. The composer
  keeps its distinct focus outline.
- Preserve logical keyboard order and accessible names for controls and
  navigation. Inputs and dropdowns need programmatic labels even when the
  visible label is visually hidden.
- The shared `Select` uses Radix listbox semantics and must retain arrow-key
  navigation, Home/End navigation, Escape dismissal, selected state, and
  disabled-option behavior.
- `Select` options are listbox options, not action buttons. Radix may render
  them as `div` elements with `role="option"`; keep their listbox semantics and
  use native buttons for actions instead.
- Give the composer message field a localized accessible name on the editable
  text field itself, and exclude the visual placeholder from semantics to avoid
  duplicate announcements.
- Make important status understandable without relying on color alone.
- Honor reduced-motion settings; use motion only for state, relationship,
  hierarchy, and feedback.
- Keep loading, empty, unavailable, error, retry, disabled, selected,
  permission-denied, and destructive states explicit where applicable. Explain
  what failed and how the user can recover.
- On a new chat, keep the welcome text and composer together. Explain model
  selection when no model is selected; keep the composer docked below messages
  after the conversation starts.
- Make long labels, large numbers, missing data, slow or failed requests, and
  permission limits fit the relevant states without clipping or losing their
  meaning.
