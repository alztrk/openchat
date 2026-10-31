# Design System

## Brand mark

OpenChat's mark uses three rounded, separated forms around an open center: two graphite forms and one copper form. The mark began as an ImageGen concept and was traced into clean SVG paths.

- **Symbol:** Use `apps/desktop/assets/brand/logo.svg` on light surfaces, `logo-dark.svg` on dark surfaces
- **Wordmark:** For documents and external presentation; the desktop UI uses the symbol alone
- **Readability:** 16, 32, and 48 pixels minimum
- **App icon:** Generated from `apps/desktop/assets/brand/app-icon.svg`

The wordmark uses Source Sans 3 with system sans-serif fallbacks. Use the provided SVG assets instead of recreating with CSS.

## Visual direction

OpenChat is a desktop AI workspace. Chat is the primary workflow.

**Design principles:**

1. **Neutral surfaces, restrained accent.** Use OpenChat's copper accent to establish hierarchy. Design for the task and its real content states, not decoration.
2. **Clear page hierarchy.** Primary content → primary action → supporting details. One clear primary action per surface.
3. **Explicit states.** Empty, loading, unavailable, and error states describe the actual state. No sample or fabricated data.

### Color roles

| Role | Light | Dark |
|------|-------|------|
| Canvas / background | `#F4F5F6` | `#15171A` |
| Navigation | `#EBEDF0` | `#1A1D21` |
| Surface | `#FAFBFC` | `#202328` |
| Raised / overlay | `#FFFFFF` | `#282C31` |
| Composer | `#F1F3F5` | `#24282D` |
| Selected | `#E3E7EB` | `#343940` |
| Hover | `#EDF0F2` | `#2B3036` |
| Border | `#CCD1D7` | `#3D434B` |
| Primary text | `#23272C` | `#F0F2F3` |
| Secondary text | `#56606A` | `#B6BCC4` |
| Accent | `#8E563B` | `#D08B68` |

Destructive, warning, success, informational, focus-ring, and high-contrast colors are defined as theme variables. Use semantic role classes (`bg-background`, `bg-surface`, `text-text`, `border-border`) rather than raw hex values.

### Typography

| Element | Size / Line |
|---------|-------------|
| Interface | Source Sans 3, 400/500/600/700 |
| Code | Source Code Pro, 400/600 |
| Conversation | 16px / 26px |
| Page title | 24px / 32px |
| Welcome title | 28px / 36px |
| Section title | 20px / 28px |
| Body | 15px / 22px |
| Metadata | 12px / 16px min |
| Code | 13px / 20px |

All app-authored text goes through localization catalogs. Protocol identifiers and provider-supplied text stay outside UI copy.

### Spacing and dimensions

| Element | Value |
|---------|-------|
| Spacing scale | 4, 8, 12, 16, 24, 32, 40, 48 px |
| Form controls | 8px radius |
| Buttons | 12px radius |
| Cards / panels | 12px radius |
| Composer | 20px radius |
| Dialogs | 16px radius |
| Control height | 40px (touch: 44px min) |
| Icon sizes | 20px standard, 16px inline, 24px primary |
| Reading width | 920px max |
| Composer width | 800px max |

## Component rules

### Mandatory

1. **Option selection uses shared controls.** Import `Select` from `@/design-system` for finite single-value choices. Native `<select>` elements are prohibited. Use `Tabs`/`SegmentedControl` for mode switches, `Switch` for binary settings.
2. **Use shared primitives.** `Button`, `Input`, `Select`, `Switch`, `Tabs`, `Dialog`, `Tooltip`, `DropdownMenu`, `Card`, `ScrollArea` from `@/design-system`. Do not fork border, focus, keyboard, disabled, or motion behavior.
3. **Use the `@/` import alias.** No relative parent imports across module boundaries.
4. **Use named tokens.** Color, type, spacing, radius, and layout tokens from `design-system/styles/tokens.css`.
5. **No static inline styles.** Layout and appearance belong in Tailwind tokens or semantic CSS classes. Inline style is for runtime values only.
6. **Respect motion and accessibility.** Reuse the primitive's keyboard, focus, dismissal, and reduced-motion behavior.
7. **Share dropdown surfaces.** `Select` for values, `DropdownMenu` for actions. Both use the shared `openchat-dropdown-surface` class.
8. **Share selection triggers.** Uses `openchat-dropdown-trigger` class. Subtle and compact variants for embedded controls.

If a shared primitive cannot express a product need, change the primitive or propose a documented design-system extension. Do not create a second control silently.

### Enforced styling

- **No Tailwind arbitrary values** — no `text-[13px]`, arbitrary colors, spacing, dimensions, or CSS properties
- **Named font-size scale only** — `text-xs`, `text-sm`, `text-base`, `text-body`, plus semantic sizes
- **Semantic palette** — `bg-background`, `bg-surface`, `text-text`, `text-secondary-text`, `border-border`, `bg-accent`
- **No one-off negative spacing** — use semantic classes that describe the product role
- **CSS separation** — theme/layout in `tokens.css`, typography in `typography.css`, motion in `animations.css`, primitive styling in `components.css`

## Dropdown architecture

| Control | Type |
|---------|------|
| `Select` | Finite single-value (Radix listbox) |
| `DropdownMenu` | Action menus |
| Specialized picker | Richer rows, search, descriptions (keeps shared surface) |
| Search panels | Conversation sidebar search, model search, filters |
| Slash suggestions | Combobox semantics |

Dropdown panels use `openchat-dropdown-surface` with configurable alpha, backdrop-blur, backdrop-saturation, shadow, border, and radius tokens. High-contrast and forced-color modes use an opaque raised surface (no backdrop filtering).

## Navigation

Primary: **Chats**, **Models**, **Settings**. Local models are reached from Models and local-engine settings.

### Title bar (Windows)
Back/forward controls → sidebar toggle → 8px padding → 48px height

### Navigation rail (other platforms)
Icon-only at all widths → 44px controls with 8px horizontal inset → 60px rail width → localized tooltips

### Conversation sidebar
- Sticky header with "New chat" (left) and search (right)
- Independently scrollable categories up to 280px
- Bottom border while scrolled
- Conversation titles without provider/model labels
- Batch-selection controls only in selection mode

## Interaction and accessibility

- Keyboard focus visible with quiet surface highlight
- Logical keyboard order, accessible names on inputs
- `Select` retains arrow-key nav, Home/End, Escape, selected state, disabled options
- Composer message field has a localized accessible name
- Status understandable without color alone
- Reduced-motion honored; motion only for state, relationship, hierarchy, feedback
- Explicit states: loading, empty, unavailable, error, retry, disabled, selected, permission-denied, destructive

## Testing

Biome configuration rejects: unsafe type assertions, non-null assertions, banned types, incomplete React hook dependencies, accumulating spread operations, Tailwind arbitrary values, native `<select>`, dangerous HTML sinks, array-index keys, and global `isNaN`/`isFinite`.

Run `npx biome check` before submitting frontend changes.
