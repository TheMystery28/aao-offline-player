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
