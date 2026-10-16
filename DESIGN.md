# OpenChat UI design system

## Visual direction

OpenChat is a desktop-first AI workspace for a broad audience. Use neutral,
clearly separated surfaces with a restrained copper accent so conversations
and provider controls have a readable hierarchy. Chat remains the primary
workflow. Projects group related conversations, saved responses stay available
inside chats, and model discovery stays separate from day-to-day conversation
controls.

Use the existing React components and Tailwind/CSS primitives in
`frontend/src/shared/ui` with OpenChat's own tokens and Lucide icons. Avoid
decorative gradients, unnecessary card grids, and icons that do not describe
the action or object.

## Color roles

| Role | Light | Dark |
| --- | --- | --- |
| Background | `#F4F5F6` | `#15171A` |
| Navigation | `#EBEDF0` | `#1A1D21` |
| Surface | `#FAFBFC` | `#202328` |
| Raised surface | `#FFFFFF` | `#282C31` |
| Composer | `#F1F3F5` | `#24282D` |
| Selected | `#E3E7EB` | `#343940` |
| Hover | `#EDF0F2` | `#2B3036` |
| Border | `#CCD1D7` | `#3D434B` |
| Primary text | `#23272C` | `#F0F2F3` |
| Secondary text | `#56606A` | `#B6BCC4` |
| Secondary icon | `#68727D` | `#AAB1BA` |
| Accent | `#8E563B` | `#D08B68` |

Destructive, warning, success, informational, and focus-ring colors are
defined alongside the theme variables in `frontend/src/index.css`. Use
semantic palette roles instead of introducing page-specific color values.

## Typography

- Interface: Source Sans 3, variable font, weights 400, 500, 600, and 700.
- Code and technical identifiers: Source Code Pro, weights 400 and 600.
- Conversation: 16 px / 26 px.
- Page title: 24 px / 32 px.
- Fresh-chat welcome title: 28 px / 36 px.
- Section title: 20 px / 28 px.
- Component title: 16 px / 24 px.
- Body: 15 px / 22 px.
- Metadata: 12 px / 16 px minimum.
- Code: 13 px / 20 px.

Use sentence case and the localized product copy. Keep labels concise and
avoid artificial letter spacing. The user-controlled response font setting
changes conversation content; interface labels use Source Sans 3.

## Dimensions and surfaces

- Spacing scale: 4, 8, 12, 16, 24, 32, 40, and 48 px.
- Form controls: 8 px radius; buttons: 12 px radius. Menus use their
  component-specific radius.
- Cards and panels: 12 px radius; the composer: 20 px radius.
- Dialogs: 16 px radius.
- Standard icon: 20 px; inline icon: 16 px; primary action icon: 24 px.
- Desktop control height: 40 px; touch targets: at least 44 px.
- Conversation reading width: 920 px maximum; composer width: 800 px maximum.

Use shared surface and motion tokens from `frontend/src/index.css` and
`frontend/tailwind.config.js`. Keep elevated effects restrained, honor reduced
motion, and make loading feedback understandable without animation.

## Navigation and icons

Primary navigation is Chats, Models, and Settings. Local models are reached
from Models and from local-engine settings so model management does not
compete with the main conversation destinations.

On Windows, place page back and forward controls in the custom title bar,
followed by the sidebar toggle. Give the title bar 8 px of vertical padding
within its 48 px height, and leave 8 px between these navigation controls. The
navigation history covers Chats, Models, Local Models, and Settings; it does
not track conversation selection or subsections within Settings. Keep the
sidebar toggle in the navigation rail on other platforms.

Use Lucide icons consistently. Keep primary navigation icon-only at every
window width, give its 44 px controls an 8 px horizontal inset for a 60 px rail,
and show localized labels in tooltips and semantic names. Use distinct selected
and unselected glyphs for toggle states such as pinning and favorites. Keep
selected state visible with shape or fill as well as accent color.

Keep the conversation sidebar header sticky, with New chat on the left and
search on the right. Open message search in a modal with the input in its
header and results in its scrollable body. Keep search filters and saved
searches available in the modal. Keep each expanded category's contents
independently scrollable up to 280 px while the sidebar body scrolls between
categories. Show a bottom border while the list is scrolled. Show conversation
titles without provider or model route labels, and show batch-selection
controls only while selection mode is active.

## Interaction and accessibility

- Keep keyboard focus visible with a quiet surface highlight. The composer
  keeps its distinct, animated focus outline.
- Preserve keyboard order and accessible names for controls and navigation.
- Give the composer message field a localized accessible name on the editable
  text field itself, and exclude the visual placeholder from semantics to avoid
  duplicate announcements.
- Make important status understandable without relying on color alone.
- Honor reduced-motion settings; use motion only for state and feedback.
- Keep loading, empty, error, retry, disabled, and destructive states explicit.
- On a new chat, keep the welcome text and composer together. Explain model
  selection when no model is selected; keep the composer docked below messages
  after the conversation starts.
- Reflow page headers and action groups at narrow widths instead of clipping.
- Keep text and code legible when the user enlarges text.
