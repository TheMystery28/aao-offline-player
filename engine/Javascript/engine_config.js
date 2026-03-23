"use strict";
/*
Ace Attorney Online - Engine Configuration System

Centralized JSON config with localStorage persistence and live change events.
ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'engine_config',
	dependencies : ['engine_events'],
	init : function()
	{
		EngineConfig._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var EngineConfig = (function() {
	const STORAGE_KEY = 'aao_engine_config';
	const CONFIG_PATH = 'config/default_config.json';

	let defaults = {};
	let config = {};

	// Deep clone a plain object/array (no functions, no circular refs)
	function deepClone(obj) {
		if (obj === null || typeof obj !== 'object') return obj;
		if (Array.isArray(obj)) {
			const arr = [];
			for (let i = 0; i < obj.length; i++) {
				arr[i] = deepClone(obj[i]);
			}
			return arr;
		}
		const copy = {};
		const keys = Object.keys(obj);
		for (let i = 0; i < keys.length; i++) {
			copy[keys[i]] = deepClone(obj[keys[i]]);
		}
		return copy;
	}

	// Deep merge source into target (mutates target)
	function deepMerge(target, source) {
		if (!source || typeof source !== 'object') return target;
		const keys = Object.keys(source);
		for (let i = 0; i < keys.length; i++) {
			const key = keys[i];
			if (
				source[key] !== null &&
				typeof source[key] === 'object' &&
				!Array.isArray(source[key]) &&
				target[key] !== null &&
				typeof target[key] === 'object' &&
				!Array.isArray(target[key])
			) {
				deepMerge(target[key], source[key]);
			} else {
				target[key] = deepClone(source[key]);
			}
		}
		return target;
	}

	// Traverse a dot-separated path on an object
	function getByPath(obj, dotpath) {
		const parts = dotpath.split('.');
		let current = obj;
		for (let i = 0; i < parts.length; i++) {
			if (current === null || current === undefined || typeof current !== 'object') {
				return undefined;
			}
			current = current[parts[i]];
		}
		return current;
	}

	// Set a value at a dot-separated path on an object (creates intermediates)
	function setByPath(obj, dotpath, value) {
		const parts = dotpath.split('.');
		let current = obj;
		for (let i = 0; i < parts.length - 1; i++) {
			if (current[parts[i]] === undefined || current[parts[i]] === null || typeof current[parts[i]] !== 'object') {
				current[parts[i]] = {};
			}
			current = current[parts[i]];
		}
		current[parts[parts.length - 1]] = value;
	}

	// Migrate old config keys to new names
	function migrateStorage(parsed) {
		if (parsed.layout && parsed.layout.courtRecordPosition !== undefined) {
			var mapping = { right: '1-2-3', left: '2-1-3', bottom: '12-3' };
			var mapped = mapping[parsed.layout.courtRecordPosition];
			if (mapped) {
				parsed.layout.panelArrangement = mapped;
			}
			delete parsed.layout.courtRecordPosition;
		}
		// Remove defunct keys
		if (parsed.layout) {
			delete parsed.layout.settingsPosition;
			delete parsed.layout.fullWidth;
		}
	}

	// Load localStorage overlay
	function loadFromStorage() {
		try {
			const stored = window.localStorage.getItem(STORAGE_KEY);
			if (stored) {
				const parsed = JSON.parse(stored);
				migrateStorage(parsed);
				deepMerge(config, parsed);
			}
		} catch (e) {
			console.warn('[EngineConfig] Failed to load from localStorage:', e.message);
		}
	}

	// Save current config diff to localStorage (only non-default values)
	function saveToStorage() {
		try {
			const diff = getDiff(defaults, config);
			if (diff && Object.keys(diff).length > 0) {
				window.localStorage.setItem(STORAGE_KEY, JSON.stringify(diff));
			} else {
				window.localStorage.removeItem(STORAGE_KEY);
			}
		} catch (e) {
			console.warn('[EngineConfig] Failed to save to localStorage:', e.message);
		}
	}

	// Compute diff: returns object with only keys where config differs from defaults
	function getDiff(base, current) {
		if (base === current) return undefined;
		if (base === null || current === null || typeof base !== 'object' || typeof current !== 'object') {
			return deepClone(current);
		}
		if (Array.isArray(base) || Array.isArray(current)) {
			if (JSON.stringify(base) !== JSON.stringify(current)) {
				return deepClone(current);
			}
			return undefined;
		}
		const result = {};
		const keys = Object.keys(current);
		let hasKeys = false;
		for (let i = 0; i < keys.length; i++) {
			const key = keys[i];
			const d = getDiff(base[key], current[key]);
			if (d !== undefined) {
				result[key] = d;
				hasKeys = true;
			}
		}
		return hasKeys ? result : undefined;
	}

	return {
		/**
		 * Initialize: load defaults via sync XHR, overlay localStorage.
		 * Called by module init.
		 */
		_init: function() {
			try {
				const xhr = new XMLHttpRequest();
				xhr.open('GET', CONFIG_PATH, false); // synchronous
				xhr.send();
				if (xhr.status === 200) {
					defaults = JSON.parse(xhr.responseText);
				} else {
					console.error('[EngineConfig] Failed to load defaults: HTTP ' + xhr.status);
					defaults = {};
				}
			} catch (e) {
				console.error('[EngineConfig] Failed to load defaults:', e.message);
				defaults = {};
			}
			config = deepClone(defaults);
			loadFromStorage();
		},

		/**
		 * Get a config value by dot-separated path.
		 * @param {string} dotpath - e.g. 'controls.keyboard.proceed'
		 * @returns {*} The value, or undefined if path doesn't exist
		 */
		get: function(dotpath) {
			return getByPath(config, dotpath);
		},

		/**
		 * Set a config value, persist to localStorage, emit config:changed.
		 * @param {string} dotpath - e.g. 'display.mute'
		 * @param {*} value
		 */
		set: function(dotpath, value) {
			const oldValue = getByPath(config, dotpath);
			setByPath(config, dotpath, value);
			saveToStorage();
			EngineEvents.emit('config:changed', {
				path: dotpath,
				value: value,
				oldValue: oldValue
			});
		},

		/**
		 * Reset a config path to its default value, or reset all if no path given.
		 * @param {string} [dotpath] - If omitted, resets entire config
		 */
		reset: function(dotpath) {
			if (dotpath) {
				const defaultValue = getByPath(defaults, dotpath);
				setByPath(config, dotpath, deepClone(defaultValue));
			} else {
				config = deepClone(defaults);
			}
			saveToStorage();
			EngineEvents.emit('config:changed', {
				path: dotpath || null,
				value: dotpath ? getByPath(config, dotpath) : null,
				oldValue: null
			});
		},

		/**
		 * Get the full config object (deep clone to prevent mutation).
		 * @returns {Object}
		 */
		getAll: function() {
			return deepClone(config);
		},

		/**
		 * Get the default value for a dot-path.
		 * @param {string} dotpath
		 * @returns {*}
		 */
		getDefault: function(dotpath) {
			return getByPath(defaults, dotpath);
		},

		/**
		 * Deep merge a case-specific config overlay. NOT persisted to localStorage.
		 * Used for case-bundled config in .aaocase ZIP files.
		 * @param {Object} caseConfig
		 */
		loadCaseConfig: function(caseConfig) {
			deepMerge(config, caseConfig);
			EngineEvents.emit('config:changed', {
				path: null,
				value: null,
				oldValue: null
			});
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('engine_config');
