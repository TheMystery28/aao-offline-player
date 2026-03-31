# The .aaocase Format

Cases can be exported as `.aaocase` files — compressed ZIP archives designed for sharing.

A `.aaocase` file contains everything needed to play the case offline:

```
MyCase.aaocase
├── manifest.json        Metadata (title, author, language, asset inventory)
├── trial_info.json      Case information
├── trial_data.json      Game script (all frames, logic, dialogue)
├── assets/              Case-specific images and audio
├── defaults/            Shared assets this case uses (sprites, backgrounds, music)
├── plugins/             (optional) Bundled plugins
│   ├── manifest.json    Plugin list
│   ├── plugin.js        Plugin code
│   └── assets/          Plugin assets (flat folder — fonts, sounds, images)
├── case_config.json     (optional) Plugin config overrides
├── plugin_params.json   (optional) Plugin parameter overrides (by_sequence/by_collection scopes)
└── saves.json           (optional) Game progress / save data
```

On import, plugins from the ZIP are added to the global `plugins/` pool and scoped to the imported case. If the plugin already exists globally, only the scope is updated — the file is not duplicated.

For multi-part sequences, a single `.aaocase` bundles all parts together:

```
MySequence.aaocase
├── sequence.json        Sequence metadata + part list
├── 99990/               Part 1 (manifest + data + assets)
├── 99991/               Part 2 (manifest + data + assets)
├── defaults/            Shared assets (deduplicated across all parts)
├── plugins/             (optional) Bundled plugins (added to global pool)
├── plugin_params.json   (optional) Plugin parameter overrides (by_sequence scope)
└── saves.json           (optional) Saves for all parts
```

Collections (user-created groups of cases and/or sequences) have their own format:

```
MyCollection.aaocase
├── collection.json      Collection metadata (title, ordered item list)
├── 86146/               Standalone case (manifest + data + assets)
├── 93013/               Sequence part 1
├── 99081/               Sequence part 2
├── ...                  All cases in the collection, standalone or sequenced
├── defaults/            Shared assets (deduplicated across everything)
├── plugins/             (optional) Bundled plugins (added to global pool)
├── plugin_params.json   (optional) Plugin parameter overrides (by_sequence + by_collection scopes)
└── saves.json           (optional) Saves for all cases
```

The `collection.json` distinguishes this from a regular sequence export. It contains the collection title and an ordered list of items — each item is either a standalone case (by ID) or a sequence (by title). On import, all cases are restored and the collection grouping is recreated automatically.

## plugin_params.json (optional)

When a `.aaocase` includes plugins with parameter overrides scoped to a specific sequence or collection, those overrides are stored in `plugin_params.json` at the ZIP root:

```json
{
    "by_sequence": {
        "A Turnabout Called Justice": {
            "my_plugin.js": {
                "font_size": 18,
                "show_timer": true
            }
        }
    },
    "by_collection": {
        "My Favorites": {
            "my_plugin.js": {
                "theme": "dark"
            }
        }
    }
}
```

Plugin parameters cascade at runtime: plugin defaults → global overrides → collection overrides → sequence overrides → per-case overrides. The `plugin_params.json` captures the sequence and collection layers so they survive export/import.

Sharing a case is as simple as exporting it and sending the `.aaocase` file. The recipient imports it into their app — no internet connection needed, no re-downloading. Save data can be optionally included, so players can share their progress.

# The .aaoplug Format

Plugins can be distributed independently from cases as `.aaoplug` files — compressed ZIP archives containing the plugin code and assets.

```
MyPlugin.aaoplug
├── manifest.json        Plugin metadata
├── plugin.js            Plugin code (or multiple .js files)
├── assets/              Plugin assets — flat folder, no subfolders
│   ├── custom_font.woff
│   ├── voice_blip1.opus
│   └── custom_bg.png
└── case_config.json     (optional) Config overrides for the case
```

The `manifest.json` declares the plugin's scripts and assets:

```json
{
    "name": "my_plugin",
    "version": "1.0",
    "author": "Author Name",
    "description": "What this plugin does",
    "scripts": ["plugin.js"],
    "assets": {
        "bundled": ["assets/custom_font.woff", "assets/voice_blip1.opus"],
        "external": [
            {"url": "https://example.com/extra.woff", "path": "assets/extra.woff"}
        ]
    },
    "config": true
}
```

When importing a `.aaoplug`, the plugin is installed to the global `plugins/` pool. The user can configure its scope — enabling or disabling it for specific cases, sequences, or collections. Plugin code and assets are stored once; multiple cases share the same files. The plugin is reference-counted: it's deleted when the last case using it removes it.

If the manifest declares external asset URLs, they are downloaded during import. At runtime, all assets are local — the plugin engine never fetches from the internet.

### Standalone JS Plugins and @assets

Plugins can also be distributed as standalone `.js` files (without a `.aaoplug` ZIP). These can declare required remote assets via a JSDoc `@assets` block:

```js
/**
 * My Plugin
 *
 * @assets
 * voice_blip1.opus = https://example.com/voice_blip1.opus
 * custom_font.woff = https://example.com/custom_font.woff
 */
EnginePlugins.register({ ... });
```

When attached via "Attach Code", the backend parses the `@assets` block and downloads each URL to `plugins/assets/`. The plugin works fully offline after import.

## Plugin Assets

All plugin assets live in a flat `plugins/assets/` folder shared across all plugins. Plugins access them via relative paths at runtime:

```js
var baseUrl = 'plugins/assets/';
// e.g. baseUrl + 'voice_blip1.opus'
```

Assets can be:
- **Bundled** in the `.aaoplug` or `.aaocase` ZIP (extracted on import)
- **External** URLs declared in `manifest.json` → `assets.external` (downloaded during import)
- **Declared via `@assets`** in standalone JS plugins (downloaded when attached)

At runtime, ALL assets are local. No online fetching ever happens during gameplay.

## Plugin Auto-Cleanup

Plugins receive a per-plugin tracked API. The engine automatically cleans up resources when a plugin is disabled:

| API method | Auto-cleaned |
|---|---|
| `api.dom.injectCSS()` | Style element removed |
| `api.dom.injectStylesheet()` | Link element removed |
| `api.dom.onMediaQuery()` | Media listener removed |
| `api.sound.registerSound()` | Sound unloaded |
| `api.input.onKeyDown()` / `onKeyUp()` | DOM listener removed |
| `api.timers.setInterval()` / `setTimeout()` / `requestAnimationFrame()` | Timer cleared |
| `events.on()` | Listener removed by namespace |

Plugins can optionally return `{ destroy: function() { ... } }` from `init()` for custom cleanup (e.g., removing manually added DOM elements). The manual `destroy()` runs before auto-cleanup.

# The .aaosave Format

Saves can be exported as lightweight `.aaosave` files — compressed ZIP archives for sharing game progress without re-exporting the full case and assets.

```
MySave.aaosave
├── saves.json           Save data (required)
├── metadata.json        Export metadata (required)
├── plugins/             (optional) Bundled plugins per case
│   └── 99990/
│       ├── manifest.json
│       ├── plugin.js
│       └── assets/
└── case_config/         (optional) Per-case config overrides
    └── 99990.json
```

## saves.json

Contains the raw save data, keyed by case ID and timestamp:

```json
{
    "99990": {
        "1700000000000": "{\"trial_id\":99990,\"trial_data_diffs\":[...],\"current_frame_index\":42}"
    }
}
```

Each save entry is a JSON string (not parsed — stored as-is). The timestamp is the millisecond epoch when the save was created.

## metadata.json

Provides human-readable context without parsing saves.json:

```json
{
    "version": 1,
    "export_date": "2026-03-25T14:30:00Z",
    "cases": [
        { "id": 99990, "title": "My Case", "save_count": 3 }
    ],
    "has_plugins": false
}
```

## Plugins (optional)

When exporting, the user can choose to include plugins. Active plugins for each case are read from the global `plugins/` pool and bundled per-case under `plugins/{case_id}/`. On import, plugins are added to the global pool and scoped to the target case.

## Importing from a Save Link

AAO online share links contain save data as a URL parameter:

```
https://aaonline.fr/player.php?trial_id=69063&save_data=eyJ0cmlhbF9pZCI6...
```

The `save_data` value is `Base64(JSON.stringify(saveObject))`. The app can import saves by pasting:
- A full URL with `save_data=...`
- A raw base64 string (just the save_data value)
- A raw JSON string (the decoded save object)

The save object must contain a numeric `trial_id` field to identify which case it belongs to.
