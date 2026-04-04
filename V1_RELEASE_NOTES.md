# What's New Since v0.3.0

## Player Controls

Full keyboard and gamepad navigation — the player can now be operated entirely without a mouse.

| Action | Keyboard | Gamepad |
|--------|----------|---------|
| Proceed | Enter, Space | A |
| Back statement | Arrow Left | B, D-Left |
| Forward statement | Arrow Right | D-Right |
| Press witness | Q | LB |
| Present evidence | W | RB |
| Back | Escape | B |
| Switch tab | Tab | Y |
| Switch to settings | Tab x2 | Y x2 |
| Browse evidence/profiles | X | X |
| Select option (1-9) | Number keys | -- |
| Navigate options | Arrows | D-Pad |
| Check evidence (hold) | Enter (hold) | A (hold) |
| Save | Ctrl+S | RT |
| Load latest save | Ctrl+L | LT |
| Toggle fullscreen | F11 | Select |
| Reset settings | Ctrl+D | Start (hold) |

- Option lists and investigation menus navigable with arrows, d-pad, and number keys 1-9
- Court record navigation: X to browse, arrows/d-pad for spatial grid, Enter/A to select, hold for Check
- Escape/B as universal back across all menus and screens
- Enter/Space/A starts the case — no click needed, keyboard works immediately on load
- Controls auto-displayed in settings panel via centralized InputRegistry

## Plugin System

- Full plugin architecture with tracked auto-cleanup API (CSS, sounds, events, timers auto-removed on disable)
- Global plugin pool with scoped activation (global, per-collection, per-sequence, per-case)
- Per-scope parameter editing with cascade (global defaults < collection < sequence < case)
- Asset downloads via `@assets` comment block — downloaded automatically when plugin is attached
- Distribution as `.aaoplug` bundles, raw JS, or bundled in `.aaocase` exports
- Attach Code from the player settings panel (persists to backend)
- Control override API: plugins can disable/replace built-in controls and register custom bindings
- Plugin documentation: [PLUGINS.md](PLUGINS.md)
- Example plugins: alt_nametags, backlog, night_mode_auto, custom_blips_v2

## Player Engine

- Dark mode with grey palette (on by default) and custom CSS theming
- Responsive layout with automatic wide/tabbed/stacked modes
- 12 panel arrangement layouts to reorder screen, evidence, and settings panels
- Width sliders for page, screen, evidence, and settings with live ghost preview
- GPU-accelerated screen scaling via `transform: scale()`
- Fullscreen toggle, hide header option, toolbar drag-to-move
- Instant save loading with audio stop on load
- Save management with sorted list across sequence parts and load-latest
- Evidence grid uses CSS Grid for uniform card sizing at any column count
- Auto-generated controls table in settings (updates when config or plugins change)
- Accessibility: font scale, line spacing, reduce motion, disable screen shake, disable flash, ARIA labels, focus traps, keyboard-navigable modals

## Downloads

- Download queue — start multiple downloads without blocking
- Download speed and ETA in the progress bar
- Download cancellation with cancel button
- Automatic retry on failure with configurable concurrent connections (1-10)
- Imgur removed.png detection — marks placeholder images as failed
- Universal deduplication — every asset hashed (xxh3) and checked against persistent index

## Library & Import/Export

- Asset gallery (Inspect modal) on case cards and sequence parts for browsing assets
- Inspect modal shows missing and corrupt assets (detected at runtime by the engine)
- Import zipped aaoffline folders (ZIP without manifest.json auto-detected)
- Download from URL — paste a direct link to .aaocase, .aaoplug, .aaosave, or zipped aaoffline with progress bar
- Optimize & Fix with time-since-last-run display
- Detailed storage breakdown with collapsible sections
- Save export/import as `.aaosave` files with paste AAO share links support
- Blur spoilers setting for download progress filenames
- Collections with UUID v4 identifiers and custom ordering
- Removed library sort dropdown (default A-Z)
- Hidden folder import button on mobile

## Platform

- New app icon across all platforms
- Android: fixed audio stopping on same-music save load, fixed portrait screen centering, full-diameter round icons, case-insensitive file resolution
- Mobile: hidden "Select Folder" import (no filesystem picker)
- Linux: case-insensitive file resolution fallback

## Bug Fixes (pre-existing in v0.3.0)

- Library UI not updating on Android during downloads (requestAnimationFrame paused by WebView during heavy I/O — replaced with queueMicrotask)
- Several assets incorrectly reported as missing due to wrong path resolution
- Countless other bug fixes