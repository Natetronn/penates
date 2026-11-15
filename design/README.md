# Design

The Penates logo: a house holding a flame, for the Roman household gods who kept the hearth.

## Files

| File | What |
|---|---|
| `lockup-dark.svg` | Icon and the word "Penates", for dark backgrounds (light text) |
| `lockup-light.svg` | The same for light backgrounds (dark text) |
| `canvas/` | Source of the design canvas: app icon, lockup and the icon at desktop sizes. `.dc.html` files need the canvas editor to render; the SVG inside each is plain and can be copied out. |
| `build-icons.sh` | Rebuilds every PNG, `icon.ico` and `icon.icns` in `src-tauri/icons/` from the SVGs |

The drawings the app ships live in `src-tauri/icons/`:

| File | Use |
|---|---|
| `icon.svg` | Main icon with glow; renders 128 px and up |
| `icon-64.svg` | 48–64 px: no glow, stroke 32 |
| `icon-32.svg` | 24–32 px: stroke 38, flame without its inner cut-out |
| `icon-16.svg` | 16 px: stroke 44 |
| `tray-mono.svg` | One-color glyph for a monochrome tray, cropped to the house |

`ui/logo-small.svg` (the manager's header) is `icon-32.svg` shown at 22 px. `docs/images/lockup.png` is the
lockup on `#050409`, rendered from the canvas.

## Colors

| | |
|---|---|
| Gradient | `#22E6FF` → `#8B5CFF` → `#FF4FD8` (cyan, violet, pink), drawn top-left to bottom-right across the house |
| Tile | `#0C0B16`, corner radius 116 on a 512 grid, 10% white hairline |
| Lockup background | `#050409` |
| Text on dark | `#F4F2FF` |
| Text on light | `#17152A` |
| Muted text | `#A9A5C4` |
| Manager accent | `#6D3FE0` light, `#8B5CFF` dark (`ui/tokens.css`) |

## Type

Word mark: Space Grotesk SemiBold (600), letter-spacing −0.03em. In the lockup files it is converted to
outlines, so no font is needed to show them. Space Grotesk is free under the SIL Open Font License
(Google Fonts).

## Geometry

512 × 512 grid. House: `M136 388V228L256 124L376 228V388` with round caps and joins; house and flame are
scaled 1.2× about the center. The glow is the same drawing blurred (`stdDeviation` 16) at 60% opacity
behind the sharp one, used only at 128 px and up.

## Rebuilding the icons

```sh
design/build-icons.sh                 # writes src-tauri/icons/
design/build-icons.sh /tmp/icons      # or anywhere else to compare first
```
Needs `rsvg-convert` (librsvg), ImageMagick 7 and the Tauri CLI. The output matches the committed icons
pixel for pixel.
