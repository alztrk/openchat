# OpenChat UI design system

## Visual direction

OpenChat is a desktop-first AI workspace for a broad audience. Use neutral,
clearly separated surfaces with a restrained copper accent so conversations
and provider controls have a readable hierarchy. Chat remains the primary
workflow. Projects group related conversations, saved responses stay available
inside chats, and model discovery stays separate from day-to-day conversation
controls.

Use Shadcn Flutter interaction primitives with OpenChat's own colors, type,
dimensions, and Lucide icon language. The migration is in progress; unconverted
surfaces still use Material controls. Avoid decorative gradients, unnecessary
card grids, and icons that do not describe the action or object.

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

Destructive, warning, success, informational, and focus-ring colors are also
defined in `OpenChatPalette`. High-contrast light and dark palettes are
available through `OpenChatTheme`. Use semantic palette roles instead of
introducing page-specific color values.

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
- Cards and panels: 12 px radius.
- Dialogs: 16 px radius.
- Standard icon: 20 px; inline icon: 16 px; primary action icon: 24 px.
- Desktop control height: 40 px; touch targets: at least 44 px.
- Conversation reading width: 920 px maximum; composer width: 800 px maximum.

The canonical dimensions are in `OpenChatSpacing`, `OpenChatRadii`, and
`OpenChatTypography` in `lib/app/openchat_theme.dart`.

## Navigation and icons

Primary navigation is Chats, Models, and Settings. Local models are reached
from Models and from local-engine settings so model management does not
compete with the main conversation destinations.

Use Lucide icons consistently. Keep primary navigation icon-only at every
window width, give its 44 px controls a 4 px horizontal inset for a 52 px rail,
and show localized labels in tooltips and semantic names. Use distinct selected
and unselected glyphs for toggle states such as pinning and favorites. Keep
selected state visible with shape or fill as well as accent color.

Keep the conversation sidebar's top row for search and run controls. Put chat
creation in the Chats heading, and show conversation titles without provider
or model route labels. Show batch-selection controls only while selection mode
is active.

## Interaction and accessibility

- Keep keyboard focus visible with a quiet surface highlight. The composer
  keeps its distinct, animated focus outline.
- Preserve keyboard order and accessible names for controls and navigation.
- Make important status understandable without relying on color alone.
- Honor reduced-motion settings; use motion only for state and feedback.
- Keep loading, empty, error, retry, disabled, and destructive states explicit.
- On a new chat, keep the welcome text and composer together. Explain model
  selection when no model is selected; keep the composer docked below messages
  after the conversation starts.
- Reflow page headers and action groups at narrow widths instead of clipping.
- Keep text and code legible when the user enlarges text.
