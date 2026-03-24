# AAO Offline Player

A desktop and mobile app (Tauri v2) that lets users download, manage, and play Ace Attorney Online cases completely offline.

![Main interface](docs/images/main_interface.PNG)

### Download

Grab the latest build from the [Releases page](https://github.com/TheMystery28/aao-offline-player/releases/latest):

| Platform | File | Notes |
|----------|------|-------|
| **Windows** | `AAO-Offline-Player-Windows-portable.zip` | Extract and run — no installation needed |
| **Linux** | `.AppImage` | Portable — `chmod +x` and run |
| **Linux (Debian/Ubuntu)** | `.deb` | Install with `sudo dpkg -i` |
| **Android** | `.apk` | Enable "Install from unknown sources" to install |

### How it works

The app bundles a modified version of the AAO game engine, gameplay-wise identical, just some under-the-hood improvements to performance. Since the engine is already included, downloading a case only fetches the case data (title, author, game script) and its assets (images, music, sounds, sprites). No player files, no scripts, no stylesheets.

Assets are split into two categories:
- **Shared defaults** (standard AAO sprites, backgrounds, music), downloaded once to a shared cache. If you download 20 cases that all use Phoenix Wright, the sprite is stored once.
- **Case-specific assets** (custom images/music hosted externally by the case author), stored per case.

Downloads run in parallel (configurable 1–10 concurrent), with automatic retry on failure. A manifest tracks every asset: what succeeded, what failed, and where it's stored. Failed assets can be retried later without re-downloading the whole case. Cases can also be updated to pick up changes the author made, downloading only new or modified assets.

The aaoffline project was a helpful reference for understanding how to approach offline case downloading. However, the code was written from scratch and the downloading issues I encountered while testing on Windows and Android were fixed along the way. The export/import system and its `.aaocase` format are also an original addition with a focus on practicality.

### Sharing and importing

Cases can be exported as `.aaocase` files — a minimalist ZIP-based format — and shared with others. No internet connection needed to import and play. The format supports single cases, multi-part sequences, and collections (groups of cases/sequences). Save data can optionally be included. See [FORMAT.md](FORMAT.md) for the full spec.

The app also imports from [aaoffline](https://github.com/falko17/aaoffline) HTML folders, converting them into its native managed format.

### The player

![Player settings and layout](docs/images/player_settings.gif)

The in-game player is a modified version of the AAO engine with a configurable settings panel, dark mode, and built-in keyboard/gamepad controls.

Features added to the engine:
- **Dark mode** (grey palette, on by default)
- **Responsive layout** with automatic wide/tabbed/stacked modes based on window size
- **Panel arrangement picker** to reorder the screen, evidence, and settings panels
- **Width sliders** for page, screen, evidence, and settings panels with live ghost preview
- **Fullscreen toggle** (F11 / gamepad View button)
- **Hide header** option
- **Quick save/load** (Ctrl+S / Ctrl+L / gamepad LB / LT)
- **Gamepad support** with W3C Standard mapping
- **Save management** with sorted list across sequence parts and load-latest

### Controls

| Action | Keyboard | Gamepad |
|--------|----------|---------|
| Proceed / skip | Enter, Space | A |
| Fast-forward (hold) | Shift | RB |
| Back statement | Arrow Left | B, D-Left |
| Forward statement | Arrow Right | D-Right |
| Switch tab | Tab | Y |
| Save | Ctrl+S | LB |
| Load latest save | Ctrl+L | LT |
| Toggle fullscreen | F11 | View |
| Reset settings | Ctrl+D | RB+RT |

In tabbed mode, **Switch tab** toggles between Evidence and Profiles. Double-press to open Settings, press once to return.

Keyboard controls were inspired by the [AAO Keyboard Controls userscript](https://aaonline.fr/forum/viewtopic.php?t=13534). Gamepad support uses the W3C Standard Gamepad mapping.

### Known issues

- The asset loading bars (images/sounds) may not reach 100% green on some cases, especially on Linux. All assets are still present and the game plays correctly — the bar just doesn't fully fill due to various reasons.

### Author note

Since the AAO source code is open, it would technically be possible to use it to create and share cases entirely offline — bypassing the website. That's not the primary intent, but let's be honest: the server crashes regularly — too regularly — and to make matters worse, most custom assets aren't even hosted on the website itself. While yes, they aren't hosting the assets, the site not being reachable randomly makes people unlikely to even play the games.

I believe creators should use specialized game engines like Godot/Ren'py and keep their assets safe in the first place. Yes, it may require a steeper learning curve, but you will have far more freedom. But for those interested, building an offline `.aaocase` creator would be straightforward and quick — barely different from the existing online editor, except your work would no longer be held hostage by an unstable server and you would be able to export them.
