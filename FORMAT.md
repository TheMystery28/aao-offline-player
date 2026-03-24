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
│   ├── manifest.json    Plugin list and metadata
│   ├── plugin.js        Plugin code
│   └── assets/          Plugin assets (flat folder — fonts, sounds, images)
├── case_config.json     (optional) Plugin config overrides
└── saves.json           (optional) Game progress / save data
```

For multi-part sequences, a single `.aaocase` bundles all parts together:

```
MySequence.aaocase
├── sequence.json        Sequence metadata + part list
├── 99990/               Part 1 (manifest + data + assets)
├── 99991/               Part 2 (manifest + data + assets)
├── defaults/            Shared assets (deduplicated across all parts)
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
└── saves.json           (optional) Saves for all cases
```

The `collection.json` distinguishes this from a regular sequence export. It contains the collection title and an ordered list of items — each item is either a standalone case (by ID) or a sequence (by title). On import, all cases are restored and the collection grouping is recreated automatically.

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

When importing a `.aaoplug`, the user selects which existing downloaded case(s) to attach it to. The plugin files are extracted to `case/{id}/plugins/`. If the manifest declares external asset URLs, they are downloaded during import. At runtime, all assets are local — the plugin engine never fetches from the internet.

## Plugin Assets

All plugin assets live in a flat `plugins/assets/` folder — no subfolders. Plugins access them via relative local paths:

```js
var baseUrl = 'case/' + api.player.getTrialInfo().id + '/plugins/assets/';
// e.g. baseUrl + 'voice_blip1.opus'
```

Assets can be:
- **Bundled** in the `.aaoplug` or `.aaocase` ZIP (extracted on import)
- **External** URLs declared in `manifest.json` → `assets.external` (downloaded during import)

At runtime, ALL assets are local. No online fetching ever happens during gameplay.
