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
	dependencies : ['engine_events', 'engine_config', 'input_registry', 'events', 'page_loaded'],
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
	const REPEAT_ACTIONS = {};

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

	// Reusable action handlers (called from keyboard and gamepad)
	function handleSave() {
		if (typeof getSaveString === 'undefined' || typeof player_status === 'undefined') return;
		if (player_status.current_frame_index === 0) return;
		if (player_status.proceed_timer && !player_status.proceed_timer_met) return;
		if (player_status.proceed_typing && !player_status.proceed_typing_met) return;
		var gs = JSON.parse(window.localStorage.getItem('game_saves')) || {};
		if (!gs[trial_information.id]) gs[trial_information.id] = {};
		var saveStr = getSaveString();
		gs[trial_information.id][(new Date()).getTime()] = saveStr;
		window.localStorage.setItem('game_saves', JSON.stringify(gs));
		EngineEvents.emit('save:created', { saveData: JSON.parse(saveStr) });
		if (typeof refreshSavesList === 'function') refreshSavesList();
	}

	function handleLoadLatest() {
		if (typeof loadSaveString === 'undefined' || typeof player_status === 'undefined') return;
		if (player_status.proceed_timer && !player_status.proceed_timer_met) return;
		if (player_status.proceed_typing && !player_status.proceed_typing_met) return;
		var gs = JSON.parse(window.localStorage.getItem('game_saves'));
		if (!gs) return;
		var latestDate = 0, latestPartId = null, latestStr = null;
		var parts = [trial_information.id];
		if (trial_information.sequence && trial_information.sequence.list) {
			for (var si = 0; si < trial_information.sequence.list.length; si++) {
				parts.push(trial_information.sequence.list[si].id);
			}
		}
		for (var pi = 0; pi < parts.length; pi++) {
			if (!gs[parts[pi]]) continue;
			var dates = Object.keys(gs[parts[pi]]).map(Number);
			for (var di = 0; di < dates.length; di++) {
				if (dates[di] > latestDate) {
					latestDate = dates[di];
					latestPartId = parts[pi];
					latestStr = gs[parts[pi]][String(dates[di])];
				}
			}
		}
		if (!latestStr) return;
		if (latestPartId == trial_information.id) {
			loadSaveString(latestStr);
		} else {
			var url = new URL(window.location.href);
			url.searchParams.set('trial_id', latestPartId);
			url.searchParams.set('save_data', Base64.encode(latestStr));
			window.location.href = url.toString();
		}
	}

	function handleFullscreenToggle() {
		var current = !!EngineConfig.get('display.fullscreen');
		EngineConfig.set('display.fullscreen', !current);
	}

	// Hardcoded shortcuts (not config-driven)
	var HARDCODED_SHORTCUTS = [
		{ ctrl: true, codes: ['KeyD'], key: 'd', handler: function(e) { e.preventDefault(); EngineConfig.reset(); } },
		{ ctrl: true, codes: ['KeyS'], key: 's', handler: function(e) { e.preventDefault(); handleSave(); } },
		{ ctrl: true, codes: ['KeyL'], key: 'l', handler: function(e) { e.preventDefault(); handleLoadLatest(); } },
		{ ctrl: false, codes: ['F11'], key: 'F11', handler: function(e) { e.preventDefault(); handleFullscreenToggle(); } }
	];

	// Gamepad actions handled directly (not emitted as input:action events)
	var GAMEPAD_SPECIAL_ACTIONS = {
		'save': handleSave,
		'loadLatest': handleLoadLatest,
		'fullscreen': handleFullscreenToggle
	};

	function onKeyDown(e) {
		// Tab: always prevent default to disable browser focus navigation
		if (e.code === 'Tab' || e.key === 'Tab') {
			e.preventDefault();
		}
		// Check hardcoded shortcuts
		for (var si = 0; si < HARDCODED_SHORTCUTS.length; si++) {
			var s = HARDCODED_SHORTCUTS[si];
			if (s.ctrl && !e.ctrlKey) continue;
			if (!s.ctrl && e.ctrlKey) continue;
			if (s.codes.indexOf(e.code) !== -1 || e.key === s.key) {
				s.handler(e);
				return;
			}
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

		EngineEvents.emitCancellable('input:action', { source: 'keyboard', action: action });
	}

	function onKeyUp(e) {
		const guardKey = e.code || e.key;
		delete pressed[guardKey];

		const action = keyboardLookup[e.code] || keyboardLookup[e.key];
		if (action) {
			EngineEvents.emit('input:release', { source: 'keyboard', action: action });
		}
	}

	// Long-press Start (button 9) to reset settings
	var resetTimer = null;
	var RESET_HOLD_MS = 500;

	function checkGamepadCombos(buttons, gamepadIndex) {
		var startKey = gamepadIndex + '_resetStart';
		if (buttons[9] && buttons[9].pressed) {
			if (!gamepadWasPressed[startKey]) {
				gamepadWasPressed[startKey] = true;
				resetTimer = setTimeout(function() {
					EngineConfig.reset();
					resetTimer = null;
				}, RESET_HOLD_MS);
			}
		} else {
			if (gamepadWasPressed[startKey]) {
				gamepadWasPressed[startKey] = false;
				if (resetTimer) {
					clearTimeout(resetTimer);
					resetTimer = null;
				}
			}
		}
	}

	function pollGamepads() {
		const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];

		for (let g = 0; g < gamepads.length; g++) {
			const gp = gamepads[g];
			if (!gp) continue;

			checkGamepadCombos(gp.buttons, g);

			for (let b = 0; b < gp.buttons.length; b++) {
				const key = g + '_' + b;
				const action = gamepadLookup[String(b)];

				if (gp.buttons[b] && gp.buttons[b].pressed) {
					if (action && !gamepadWasPressed[key]) {
						gamepadWasPressed[key] = true;
						var specialHandler = GAMEPAD_SPECIAL_ACTIONS[action];
						if (specialHandler) { specialHandler(); }
						else { EngineEvents.emitCancellable('input:action', { source: 'gamepad', action: action }); }
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
			}, 0, 'engine');

			// Register hardcoded shortcuts in the controls registry
			InputRegistry.register({ action: 'save', label: 'save', keyboard: 'Ctrl+S', gamepad: 'RT', source: 'engine', module: 'input_manager' });
			InputRegistry.register({ action: 'loadLatest', label: 'load latest', keyboard: 'Ctrl+L', gamepad: 'LT', source: 'engine', module: 'input_manager' });
			InputRegistry.register({ action: 'reset', label: 'reset settings', keyboard: 'Ctrl+D', gamepad: 'Start (hold)', source: 'engine', module: 'input_manager' });
			InputRegistry.register({ action: 'fullscreen', label: 'fullscreen', keyboard: 'F11', gamepad: 'Select', source: 'engine', module: 'input_manager' });

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
		},

		/**
		 * Module disable registry with per-source granularity.
		 * Plugins can disable built-in control modules by name and optionally
		 * by input source ('keyboard' or 'gamepad').
		 *
		 * disableModule('x')            → disable both sources
		 * disableModule('x', 'keyboard') → disable keyboard only
		 * enableModule('x')             → enable both sources
		 * enableModule('x', 'gamepad')  → enable gamepad only
		 * isModuleDisabled('x')         → true only if BOTH disabled
		 * isModuleDisabled('x', 'keyboard') → true if keyboard disabled
		 */
		_disabledModules: {},

		disableModule: function(name, source) {
			if (!this._disabledModules[name]) {
				this._disabledModules[name] = { keyboard: false, gamepad: false };
			}
			if (!source) {
				this._disabledModules[name].keyboard = true;
				this._disabledModules[name].gamepad = true;
			} else {
				this._disabledModules[name][source] = true;
			}
			EngineEvents.emit('controls:module:changed', { module: name });
		},

		enableModule: function(name, source) {
			if (!this._disabledModules[name]) return;
			if (!source) {
				delete this._disabledModules[name];
			} else {
				this._disabledModules[name][source] = false;
				if (!this._disabledModules[name].keyboard && !this._disabledModules[name].gamepad) {
					delete this._disabledModules[name];
				}
			}
			EngineEvents.emit('controls:module:changed', { module: name });
		},

		isModuleDisabled: function(name, source) {
			var entry = this._disabledModules[name];
			if (!entry) return false;
			// No source → true only if BOTH disabled (legacy compat)
			if (!source) return entry.keyboard && entry.gamepad;
			return !!entry[source];
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('input_manager');
