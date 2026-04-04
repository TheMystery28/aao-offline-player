# Plugin System Documentation

The AAO Offline Player engine supports plugins — JS files that extend the player with custom behavior. Plugins get a tracked API with automatic cleanup, so disabling a plugin undoes all its effects without reloading the page.

## Quick start

```javascript
EnginePlugins.register({
    name: 'my_plugin',
    version: '1.0',
    init: function(config, events, api) {
        // Your code here
    }
});
```

Place the file in the `plugins/` folder, or paste it via **Attach Code** in the player settings panel.

## Lifecycle

1. **Register** — Plugin script loads and calls `EnginePlugins.register(descriptor)`. The descriptor is queued.
2. **Init** — After `player:init` fires, the system calls `descriptor.init(config, events, api)` with three arguments:
   - `config` — EngineConfig instance for reading/writing settings
   - `events` — Namespaced event wrapper (auto-cleaned on destroy)
   - `api` — Per-plugin tracked API (auto-cleaned on destroy)
3. **Destroy** — When the plugin is disabled or the case closes, auto-cleanup runs. If `init()` returned `{ destroy: function() {...} }`, the manual destroy runs first, then auto-cleanup.

## Plugin descriptor

```javascript
EnginePlugins.register({
    name: 'my_plugin',         // Required. Unique identifier.
    version: '1.0',            // Optional. Shown in settings.
    params: {                  // Optional. User-configurable parameters.
        fontSize: {
            type: 'number',    // 'number', 'checkbox', 'text', 'select'
            default: 12,
            label: 'Font Size (px)',
            min: 6, max: 30, step: 1    // For number type
        },
        theme: {
            type: 'select',
            default: 'dark',
            label: 'Theme',
            options: ['light', 'dark', { value: 'auto', label: 'Automatic' }]
        }
    },
    init: function(config, events, api) {
        var params = config.getPluginParams('my_plugin');
        // params.fontSize → 12 (or user override)
        // params.theme → 'dark' (or user override)

        return {
            destroy: function() {
                // Optional manual cleanup (raw DOM, etc.)
            }
        };
    }
});
```

Parameters appear automatically in the settings panel. Values cascade: plugin default < collection override < sequence override < case override < session override.

## API reference

### api.dom

```javascript
api.dom.query(selector)              // document.querySelector
api.dom.queryAll(selector)           // document.querySelectorAll
api.dom.create(tag)                  // document.createElement

api.dom.addClass(el, cls)
api.dom.removeClass(el, cls)
api.dom.hasClass(el, cls)
api.dom.toggleClass(el, cls)
api.dom.setClass(el, cls)
api.dom.emptyNode(el)
api.dom.setNodeTextContents(el, text)

// Tracked — auto-removed on destroy
api.dom.injectCSS(cssText)           // Returns { element, remove() }
api.dom.injectStylesheet(href)       // Returns { element, remove() }
api.dom.onMediaQuery(query, handler) // Returns { matches, remove() }
```

### api.player

```javascript
api.player.readFrame(index)          // Jump to frame
api.player.proceed()                 // Advance dialogue
api.player.getCurrentFrameId()       // Current frame ID
api.player.getCurrentFrameIndex()    // Current frame index
api.player.getNextFrameIndex()       // Next frame index
api.player.getStatus()               // Full player_status object
api.player.getTrialData()            // Full trial_data object
api.player.getTrialInfo()            // trial_information object
```

### api.sound

```javascript
api.sound.playMusic(id)
api.sound.stopMusic()
api.sound.playSound(id)
api.sound.fadeMusic(volume, duration)
api.sound.crossfadeMusic(id, samePosition, volume, duration)

// Tracked — auto-unloaded on destroy
api.sound.registerSound(id, { urls: [...], loop: false, volume: 100 })
api.sound.unloadSound(id)

api.sound.getSoundById(id)           // Returns Howl instance
api.sound.setSoundVolume(id, vol)    // 0-100
api.sound.mute(muted)
api.sound.isMuted()
```

### api.courtRecord

```javascript
api.courtRecord.setHidden(type, id, hidden)  // type: 'evidence' or 'profiles'
api.courtRecord.refresh()
api.courtRecord.getElement(type, id)          // Returns DOM element
```

### api.input

```javascript
// Tracked — auto-removed on destroy
api.input.onKeyDown(handler)
api.input.onKeyUp(handler)
api.input.offKeyDown(handler)         // Manual removal
api.input.offKeyUp(handler)           // Manual removal

// Tracked — listens to input:action events for a specific action name
api.input.registerAction(actionName, handler)

// Tracked — registers a control binding in the settings panel display
api.input.registerBinding({
    action: 'myAction',
    label: 'My Custom Action',
    keyboard: 'Ctrl+M',
    gamepad: 'LB+RB'
})
api.input.unregisterBinding(action)   // Manual removal
```

### api.controls

Disable/replace built-in control modules. Tracked — auto-re-enabled on destroy.

```javascript
// Modules: 'keyboard_controls', 'gamepad_controls', 'option_navigator', 'courtrecord_navigator'
api.controls.disable('option_navigator')
api.controls.enable('option_navigator')
api.controls.isDisabled('option_navigator')
```

### api.settings

```javascript
api.settings.addSection('My Plugin', [
    { type: 'checkbox', key: 'plugins.my_plugin.params.enabled', label: 'Enabled' },
    { type: 'slider', key: 'plugins.my_plugin.params.volume', label: 'Volume', min: 0, max: 100, step: 5 },
    { type: 'select', key: 'plugins.my_plugin.params.mode', label: 'Mode', options: [...] }
])
api.settings.removeSection('My Plugin')
```

### api.timers

All tracked — auto-cleared on destroy.

```javascript
api.timers.setInterval(fn, delay)
api.timers.clearInterval(id)
api.timers.setTimeout(fn, delay)
api.timers.clearTimeout(id)
api.timers.requestAnimationFrame(fn)
api.timers.cancelAnimationFrame(id)
```

### api.display

```javascript
api.display.getTopScreen()           // screen_display object
api.display.getBottomScreen()        // screen_display object
```

## Events

Listen via `events.on(name, handler, priority)`. All listeners are auto-namespaced to `plugin:{name}` and auto-removed on destroy.

| Event | Data | When |
|-------|------|------|
| `player:init` | `{}` | Player fully initialized |
| `frame:before` | `{ frameIndex, frameId, frameData }` | Before frame executes |
| `frame:after` | `{ frameIndex, frameId, frameData }` | After frame executes |
| `frame:willLeave` | `{ dialogueText, dialogueHTML, speakerName }` | Before leaving current frame |
| `proceed` | — | Proceed button clicked |
| `input:action` | `{ source, action }` | Input action fired (cancellable) |
| `input:release` | `{ source, action }` | Input released |
| `courtrecord:select` | `{ type, id }` | Evidence/profile selected |
| `courtrecord:visibility` | `{ type, id, hidden }` | Evidence hidden/shown |
| `sound:play` | `{ soundId }` | Sound plays |
| `music:play` | `{ musicId }` | Music plays |
| `music:stop` | `{}` | Music stops |
| `save:created` | `{ saveData }` | Save created |
| `save:loaded` | `{ saveData }` | Save loaded |
| `config:changed` | `{ key, value }` | Config value changed |
| `options:highlight` | `{ index, element, mode }` | Option highlight moved |
| `options:select` | `{ index, element, mode }` | Option selected |
| `options:enter` | `{ mode }` | Option panel appeared |
| `options:leave` | `{ mode }` | Option panel disappeared |
| `courtrecord:nav:enter` | `{}` | CR navigation activated |
| `courtrecord:nav:leave` | `{}` | CR navigation deactivated |
| `courtrecord:nav:highlight` | `{ index, element, tab }` | CR highlight moved |
| `courtrecord:nav:select` | `{ index, element, tab }` | CR item selected |
| `courtrecord:nav:check` | `{ index, element, tab }` | CR item check opened |
| `controls:registry:changed` | — | Controls table needs rebuild |

### Cancelling input actions

`input:action` events are cancellable. Register at a lower priority number to fire before built-in handlers:

```javascript
events.on('input:action', function(data) {
    if (data.action === 'proceed') {
        data.preventDefault(); // Built-in controls won't see this
        // Custom handling here
    }
}, -1); // Fires before priority 0 (built-in)
```

## Overriding built-in controls

A plugin can fully replace a built-in control module:

```javascript
init: function(config, events, api) {
    // 1. Disable the built-in module
    api.controls.disable('option_navigator');

    // 2. Register your replacement in the controls table
    api.input.registerBinding({
        action: 'customNav',
        label: 'custom option navigation',
        keyboard: 'WASD',
        gamepad: 'Left Stick'
    });

    // 3. Implement your own handling
    api.input.onKeyDown(function(e) {
        // Your navigation logic
    });
}
// When plugin is destroyed:
// - option_navigator re-enabled automatically
// - Custom bindings removed from controls table
// - keyDown listener removed
```

## Asset downloads (@assets)

Plugins can declare remote assets that are downloaded automatically when attached to a case:

```javascript
/**
 * My Plugin
 *
 * @assets
 * blip.opus = https://example.com/sounds/blip.opus
 * icon.png = https://example.com/images/icon.png
 */
EnginePlugins.register({
    name: 'my_plugin',
    init: function(config, events, api) {
        var trialInfo = api.player.getTrialInfo();
        var baseUrl = 'case/' + trialInfo.id + '/plugins/assets/';
        api.sound.registerSound('blip', {
            urls: [baseUrl + 'blip.opus'],
            volume: 80
        });
    }
});
```

Rules:
- `@assets` block inside a `/** */` comment
- Each line: `filename = https://url`
- Files downloaded to `plugins/assets/` when plugin is attached
- Filename collisions with other plugins auto-resolved by appending `_2`, `_3`, etc.

## Distribution

### Raw JS file
Place in `plugins/` folder or paste via **Attach Code** in the player settings.

### .aaoplug bundle
A ZIP with `.aaoplug` extension containing:
```
my_plugin.aaoplug
  manifest.json
  my_plugin.js
  assets/
    blip.opus
    icon.png
```

**manifest.json:**
```json
{
    "name": "my_plugin",
    "version": "1.0",
    "description": "My plugin description",
    "scripts": ["my_plugin.js"]
}
```

### Bundled in .aaocase exports
Case exports can include plugins. On import, plugins are extracted and activated for the case.

## Scoping

Plugins are stored in a global `plugins/` folder with scoped activation:

| Scope | Meaning |
|-------|---------|
| Global | Active for all cases |
| Collection | Active for all cases in a collection |
| Sequence | Active for all parts of a sequence |
| Case | Active for one specific case |

When a plugin is enabled individually for every case in a sequence, the scope is auto-promoted to sequence-level. Plugins are reference-counted: when the last case using a plugin removes it, the plugin files are deleted.

## Auto-cleanup summary

Everything marked "tracked" in the API is automatically undone when the plugin is destroyed:

| What | How it's cleaned |
|------|-----------------|
| Injected CSS | Removed from DOM |
| Injected stylesheets | Removed from DOM |
| Registered sounds | Unloaded via SoundHowler |
| keydown/keyup listeners | removeEventListener |
| Event bus listeners | Namespace cleared |
| setInterval | clearInterval |
| setTimeout | clearTimeout |
| requestAnimationFrame | cancelAnimationFrame |
| Media query listeners | removeEventListener |
| Disabled control modules | Re-enabled |
| Registered input bindings | Unregistered from InputRegistry |

Manual `destroy()` runs first (if provided), then auto-cleanup. All cleanup steps are wrapped in try/catch — one failure doesn't prevent others.

## Examples

See `engine/plugins_examples/` for working examples:
- **alt_nametags.js** — CSS injection with configurable params
- **backlog.js** — DOM creation, event listening, manual destroy
- **night_mode_auto.js** — Media query listener, config manipulation
- **custom_blips_v2-standalone.js** — Sound replacement with @assets
