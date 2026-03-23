"use strict";
/*
Ace Attorney Online - Unified Input Manager

Config-driven input mapping. Reads bindings from EngineConfig,
handles keyboard events and gamepad polling, emits input:action/input:release.
ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'input_manager',
	dependencies : ['engine_events', 'engine_config', 'events', 'page_loaded'],
	init : function()
	{
		InputManager._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var InputManager = (function() {
	// Reverse lookup maps: configValue → actionName
	// For keyboard: event.code or event.key → action
	// For gamepad: buttonIndex (as string) → action
	let keyboardLookup = {};
	let gamepadLookup = {};

	// Actions that allow key repeat (no pressed guard)
	const REPEAT_ACTIONS = { 'skip': true };

	// Track pressed state to prevent repeat-firing
	const pressed = {};

	// Gamepad polling state
	let gamepadPolling = false;
	const gamepadWasPressed = {};

	function buildLookups() {
		keyboardLookup = {};
		gamepadLookup = {};

		const kbConfig = EngineConfig.get('controls.keyboard');
		if (kbConfig) {
			const actions = Object.keys(kbConfig);
			for (let i = 0; i < actions.length; i++) {
				const action = actions[i];
				const keys = kbConfig[action];
				if (Array.isArray(keys)) {
					for (let j = 0; j < keys.length; j++) {
						keyboardLookup[keys[j]] = action;
					}
				}
			}
		}

		const gpConfig = EngineConfig.get('controls.gamepad');
		if (gpConfig) {
			const actions = Object.keys(gpConfig);
			for (let i = 0; i < actions.length; i++) {
				const action = actions[i];
				const buttons = gpConfig[action];
				if (Array.isArray(buttons)) {
					for (let j = 0; j < buttons.length; j++) {
						const btnKey = String(buttons[j]);
						// First action wins — don't overwrite if already mapped
						if (!gamepadLookup[btnKey]) {
							gamepadLookup[btnKey] = action;
						}
					}
				}
			}
		}
	}

	function onKeyDown(e) {
		// Ctrl+D: reset all settings to defaults
		if (e.ctrlKey && (e.code === 'KeyD' || e.key === 'd')) {
			e.preventDefault();
			EngineConfig.reset();
			return;
		}

		// F11: toggle fullscreen
		if (e.code === 'F11' || e.key === 'F11') {
			e.preventDefault();
			var current = !!EngineConfig.get('display.fullscreen');
			EngineConfig.set('display.fullscreen', !current);
			return;
		}

		// Try event.code first (physical key), then event.key (logical key)
		const action = keyboardLookup[e.code] || keyboardLookup[e.key];
		if (!action) return;

		// Prevent default for mapped keys
		e.preventDefault();

		// Check repeat guard (skip action allows repeat)
		const guardKey = e.code || e.key;
		if (!REPEAT_ACTIONS[action] && pressed[guardKey]) return;
		pressed[guardKey] = true;

		EngineEvents.emit('input:action', { source: 'keyboard', action: action });
	}

	function onKeyUp(e) {
		const guardKey = e.code || e.key;
		delete pressed[guardKey];

		const action = keyboardLookup[e.code] || keyboardLookup[e.key];
		if (action) {
			EngineEvents.emit('input:release', { source: 'keyboard', action: action });
		}
	}

	function pollGamepads() {
		const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];

		for (let g = 0; g < gamepads.length; g++) {
			const gp = gamepads[g];
			if (!gp) continue;

			const buttons = gp.buttons;
			for (let b = 0; b < buttons.length; b++) {
				const key = g + '_' + b;
				const action = gamepadLookup[String(b)];

				if (buttons[b] && buttons[b].pressed) {
					if (action && !gamepadWasPressed[key]) {
						gamepadWasPressed[key] = true;
						EngineEvents.emit('input:action', { source: 'gamepad', action: action });
					}
				} else {
					if (gamepadWasPressed[key]) {
						gamepadWasPressed[key] = false;
						if (action) {
							EngineEvents.emit('input:release', { source: 'gamepad', action: action });
						}
					}
				}
			}
		}

		requestAnimationFrame(pollGamepads);
	}

	function startGamepadPolling() {
		if (!gamepadPolling) {
			gamepadPolling = true;
			requestAnimationFrame(pollGamepads);
		}
	}

	return {
		_init: function() {
			buildLookups();

			// Listen for config changes to rebuild lookups
			EngineEvents.on('config:changed', function(data) {
				if (!data.path || data.path.indexOf('controls') === 0) {
					buildLookups();
				}
			});

			// Keyboard event listeners
			document.addEventListener('keydown', onKeyDown);
			document.addEventListener('keyup', onKeyUp);

			// Gamepad: start polling when a gamepad connects
			window.addEventListener('gamepadconnected', function() {
				startGamepadPolling();
			});

			// Also start immediately if a gamepad is already connected
			if (navigator.getGamepads) {
				const existing = navigator.getGamepads();
				for (let i = 0; i < existing.length; i++) {
					if (existing[i]) {
						startGamepadPolling();
						break;
					}
				}
			}
		},

		/**
		 * Get the current keyboard lookup map (for testing/debugging).
		 * @returns {Object} Map of configValue → actionName
		 */
		getKeyboardLookup: function() {
			return keyboardLookup;
		},

		/**
		 * Get the current gamepad lookup map (for testing/debugging).
		 * @returns {Object} Map of buttonIndex → actionName
		 */
		getGamepadLookup: function() {
			return gamepadLookup;
		},

		/**
		 * Force rebuild of lookup tables from current config.
		 * Useful after EngineEvents.clear() destroys the config:changed listener.
		 */
		rebuildLookups: function() {
			buildLookups();
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('input_manager');
