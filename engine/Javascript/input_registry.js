"use strict";
/*
Ace Attorney Online - Input Controls Registry

Centralized registry where all control sources (config, engine modules,
plugins) register their bindings. The settings panel reads from this
to auto-generate the controls table.

Entry format:
  { action, label, keyboard, gamepad, source, order }

Sources: 'config' (auto-synced from EngineConfig), 'engine' (hardcoded
in modules), 'plugin:<name>' (registered by plugins).

ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'input_registry',
	dependencies : ['engine_events', 'engine_config'],
	init : function()
	{
		InputRegistry._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var InputRegistry = (function() {
	var _entries = [];
	var _orderCounter = 0;
	var _updatePending = false;

	// Action name → readable display label (for config-driven entries)
	var ACTION_LABELS = {
		'proceed': 'proceed',
		'back': 'back statement',
		'forward': 'forward statement',
		'crSwitchTab': 'switch tab',
		'press': 'press witness',
		'present': 'present evidence'
	};

	// Gamepad button index → readable name (W3C standard mapping)
	var GAMEPAD_NAMES = {
		0: 'A', 1: 'B', 2: 'X', 3: 'Y',
		4: 'LB', 5: 'RB', 6: 'LT', 7: 'RT',
		8: 'Select', 9: 'Start', 10: 'L3', 11: 'R3',
		12: 'D-Up', 13: 'D-Down', 14: 'D-Left', 15: 'D-Right', 16: 'Xbox'
	};

	// CR keybindings that are non-functional — skip from config display
	var HIDDEN_CONFIG_ACTIONS = [
		'courtRecordToggle', 'courtRecordEvidence', 'courtRecordProfiles',
		'crCheck', 'crNavigateUp', 'crNavigateDown', 'crNavigateLeft',
		'crNavigateRight', 'crSelect'
	];

	function scheduleEmit() {
		if (!_updatePending) {
			_updatePending = true;
			Promise.resolve().then(function() {
				_updatePending = false;
				EngineEvents.emit('controls:registry:changed');
			});
		}
	}

	function syncFromConfig() {
		// Remove existing config-driven entries
		for (var i = _entries.length - 1; i >= 0; i--) {
			if (_entries[i].source === 'config') _entries.splice(i, 1);
		}

		var kbConfig = EngineConfig.get('controls.keyboard') || {};
		var gpConfig = EngineConfig.get('controls.gamepad') || {};
		var actions = Object.keys(kbConfig);

		for (var a = 0; a < actions.length; a++) {
			var action = actions[a];
			if (HIDDEN_CONFIG_ACTIONS.indexOf(action) !== -1) continue;

			var keys = kbConfig[action];
			if (!Array.isArray(keys) || keys.length === 0) continue;

			var gpButtons = gpConfig[action];
			var gpLabel = '';
			if (Array.isArray(gpButtons) && gpButtons.length > 0) {
				var parts = [];
				for (var b = 0; b < gpButtons.length; b++) {
					parts.push(GAMEPAD_NAMES[gpButtons[b]] || ('Btn' + gpButtons[b]));
				}
				gpLabel = parts.join(', ');
			}

			_entries.push({
				action: action,
				label: ACTION_LABELS[action] || action,
				keyboard: keys.join(', '),
				gamepad: gpLabel,
				source: 'config',
				order: a
			});
		}

		scheduleEmit();
	}

	return {
		_init: function() {
			syncFromConfig();

			// Re-sync when config changes
			EngineEvents.on('config:changed', function(data) {
				if (!data.path || data.path.indexOf('controls') === 0) {
					syncFromConfig();
				}
			}, 0, 'engine');
		},

		/**
		 * Register a control binding entry.
		 * @param {Object} entry - { action, label, keyboard, gamepad, source }
		 */
		register: function(entry) {
			entry.order = _orderCounter++;
			// Replace existing entry with same action+source
			for (var i = 0; i < _entries.length; i++) {
				if (_entries[i].action === entry.action && _entries[i].source === entry.source) {
					_entries[i] = entry;
					scheduleEmit();
					return;
				}
			}
			_entries.push(entry);
			scheduleEmit();
		},

		/**
		 * Remove a specific entry by action and source.
		 */
		unregister: function(action, source) {
			for (var i = _entries.length - 1; i >= 0; i--) {
				if (_entries[i].action === action && _entries[i].source === source) {
					_entries.splice(i, 1);
				}
			}
			scheduleEmit();
		},

		/**
		 * Remove all entries from a given source (e.g. 'plugin:myPlugin').
		 */
		unregisterBySource: function(source) {
			var changed = false;
			for (var i = _entries.length - 1; i >= 0; i--) {
				if (_entries[i].source === source) {
					_entries.splice(i, 1);
					changed = true;
				}
			}
			if (changed) scheduleEmit();
		},

		/**
		 * Get all entries, sorted: config first, then engine, then plugins.
		 * @returns {Array}
		 */
		getAll: function() {
			return _entries.slice().sort(function(a, b) {
				var sa = a.source === 'config' ? 0 : a.source === 'engine' ? 1 : 2;
				var sb = b.source === 'config' ? 0 : b.source === 'engine' ? 1 : 2;
				if (sa !== sb) return sa - sb;
				return a.order - b.order;
			});
		},

		/**
		 * Get the human-readable name for a gamepad button index.
		 * @param {number} index
		 * @returns {string}
		 */
		getGamepadName: function(index) {
			return GAMEPAD_NAMES[index] || ('Btn' + index);
		},

		/**
		 * Get the ACTION_LABELS map (for external use if needed).
		 */
		getActionLabels: function() {
			return ACTION_LABELS;
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('input_registry');
