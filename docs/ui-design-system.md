# OpenChat UI design system

## Visual direction

OpenChat is a desktop-first AI workspace for a broad audience. The interface
uses warm, quiet surfaces and direct labels so conversation, saved work, and
provider controls remain easy to distinguish without making every page look
like a dashboard. Chat remains the primary workflow. Workspaces group chats,
Outputs stores useful assistant responses, and model discovery stays separate
from day-to-day conversation controls.

Use the existing Material 3 interaction primitives with OpenChat's own colors,
type, dimensions, and Lucide icon language. Avoid decorative gradients,
unnecessary card grids, and icons that do not describe the action or object.

## Color roles

| Role | Light | Dark |
| --- | --- | --- |
| Background | `#F6F3EF` | `#181614` |
| Navigation | `#EEEAE4` | `#211F1C` |
| Surface | `#FFFCF8` | `#211F1C` |
| Raised surface | `#FFFFFF` | `#2A2723` |
| Composer | `#F1EBE4` | `#2D2925` |
| Selected | `#E7DED3` | `#3A332C` |
| Hover | `#EFE7DE` | `#332E29` |
| Border | `#CDC3B9` | `#4A443E` |
| Primary text | `#24211E` | `#F4EEE7` |
| Secondary text | `#625B54` | `#B7ADA2` |
| Secondary icon | `#746B62` | `#C3B8AC` |
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
- Section title: 20 px / 28 px.
- Component title: 16 px / 24 px.
- Body: 14 px / 20 px.
- Metadata: 12 px / 16 px minimum.
- Code: 13 px / 20 px.

Use sentence case and the localized product copy. Keep labels concise and
avoid artificial letter spacing. The user-controlled response font setting
changes conversation content; interface labels use Source Sans 3.

## Dimensions and surfaces

- Spacing scale: 4, 8, 12, 16, 24, 32, 40, and 48 px.
- Controls and menus: 8 px radius.
- Cards and panels: 12 px radius.
- Dialogs: 16 px radius.
- Standard icon: 20 px; inline icon: 16 px; primary action icon: 24 px.
- Desktop control height: 40 px; touch targets: at least 44 px.
- Conversation reading width: 920 px maximum; composer width: 720 px maximum.

The canonical dimensions are in `OpenChatSpacing`, `OpenChatRadii`, and
`OpenChatTypography` in `lib/app/openchat_theme.dart`.

## Navigation and icons

Primary navigation is Chats, Workspaces, Outputs, Models, and Settings. Local
models are reached from Models and from local-engine settings so model
management does not compete with the main conversation destinations.

Use Lucide icons consistently. Pair every icon-only control with a localized
tooltip or semantic label. Use distinct selected and unselected glyphs for
toggle states such as pinning and favorites. Keep selected state visible with
shape or fill as well as accent color.

## Interaction and accessibility

- Keep keyboard focus visible with the palette focus-ring role.
- Preserve keyboard order and accessible names for controls and navigation.
- Make important status understandable without relying on color alone.
- Honor reduced-motion settings; use motion only for state and feedback.
- Keep loading, empty, error, retry, disabled, and destructive states explicit.
- Reflow page headers and action groups at narrow widths instead of clipping.
- Keep text and code legible when the user enlarges text.
