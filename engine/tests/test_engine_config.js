"use strict";
/**
 * EngineConfig regression tests.
 * IMPORTANT: Do NOT call EngineEvents.clear() — it destroys other modules'
 * internal listeners (InputManager, etc.). Use targeted on/off instead.
 */
function testEngineConfig() {
	TestHarness.suite('EngineConfig');

	// Clean up any stale localStorage from previous test runs
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig.reset();
	EngineConfig._init(); // Re-initialize from clean state

	// Module is loaded
	TestHarness.assertEqual(
		Modules.request_list['engine_config'], 3,
		'engine_config module is loaded (status 3)'
	);

	// EngineConfig global exists with expected API
	TestHarness.assertDefined(EngineConfig, 'EngineConfig global is defined');
	TestHarness.assertType(EngineConfig.get, 'function', 'EngineConfig.get is a function');
	TestHarness.assertType(EngineConfig.set, 'function', 'EngineConfig.set is a function');
	TestHarness.assertType(EngineConfig.reset, 'function', 'EngineConfig.reset is a function');
	TestHarness.assertType(EngineConfig.getAll, 'function', 'EngineConfig.getAll is a function');
	TestHarness.assertType(EngineConfig.getDefault, 'function', 'EngineConfig.getDefault is a function');
	TestHarness.assertType(EngineConfig.loadCaseConfig, 'function', 'EngineConfig.loadCaseConfig is a function');

	// --- Default values accessible via get() ---
	(function() {
		var proceed = EngineConfig.get('controls.keyboard.proceed');
		TestHarness.assert(Array.isArray(proceed), 'get(controls.keyboard.proceed) returns an array');
		TestHarness.assert(proceed.includes('Enter'), 'Default keyboard proceed includes Enter');
		TestHarness.assert(proceed.includes('Space'), 'Default keyboard proceed includes Space');
	})();

	(function() {
		TestHarness.assertEqual(EngineConfig.get('display.mute'), false, 'Default display.mute is false');
		TestHarness.assertEqual(EngineConfig.get('display.textSpeed'), 1.0, 'Default display.textSpeed is 1.0');
		TestHarness.assertEqual(EngineConfig.get('features.autoSave'), true, 'Default features.autoSave is true');
		TestHarness.assertEqual(EngineConfig.get('theme.name'), 'default', 'Default theme.name is "default"');
	})();

	// --- Dot-path works for nested objects ---
	(function() {
		var gamepadProceed = EngineConfig.get('controls.gamepad.proceed');
		TestHarness.assert(Array.isArray(gamepadProceed), 'get(controls.gamepad.proceed) returns an array');
		TestHarness.assertEqual(gamepadProceed[0], 0, 'Default gamepad proceed is button 0');
	})();

	// --- Invalid paths return undefined ---
	(function() {
		TestHarness.assertEqual(
			EngineConfig.get('nonexistent.path.here'), undefined,
			'Invalid path returns undefined'
		);
		TestHarness.assertEqual(
			EngineConfig.get('controls.keyboard.nonexistent'), undefined,
			'Invalid leaf path returns undefined'
		);
	})();

	// --- set() persists to localStorage ---
	(function() {
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig.reset();
		EngineConfig.set('display.mute', true);
		TestHarness.assertEqual(EngineConfig.get('display.mute'), true, 'set() updates the config value');

		var stored = window.localStorage.getItem('aao_engine_config');
		TestHarness.assert(stored !== null, 'set() persists to localStorage');
		var parsed = JSON.parse(stored);
		TestHarness.assertEqual(parsed.display.mute, true, 'localStorage contains the set value');

		EngineConfig.reset();
		window.localStorage.removeItem('aao_engine_config');
	})();

	// --- set() emits config:changed ---
	(function() {
		var received = null;
		var handler = function(data) { received = data; };
		EngineEvents.on('config:changed', handler);

		EngineConfig.set('display.pixelated', true);
		TestHarness.assert(received !== null, 'set() emits config:changed event');
		TestHarness.assertEqual(received.path, 'display.pixelated', 'config:changed carries correct path');
		TestHarness.assertEqual(received.value, true, 'config:changed carries new value');
		TestHarness.assertEqual(received.oldValue, false, 'config:changed carries old value (pixelated defaults to false)');

		EngineEvents.off('config:changed', handler);
		EngineConfig.reset();
		window.localStorage.removeItem('aao_engine_config');
	})();

	// --- reset() restores defaults ---
	(function() {
		EngineConfig.set('display.mute', true);
		EngineConfig.set('display.textSpeed', 2.5);
		TestHarness.assertEqual(EngineConfig.get('display.mute'), true, 'Pre-reset: mute is true');

		EngineConfig.reset('display.mute');
		TestHarness.assertEqual(EngineConfig.get('display.mute'), false, 'reset(path) restores single value to default');
		TestHarness.assertEqual(EngineConfig.get('display.textSpeed'), 2.5, 'reset(path) leaves other values unchanged');

		EngineConfig.reset();
		TestHarness.assertEqual(EngineConfig.get('display.textSpeed'), 1.0, 'reset() restores all values to defaults');

		window.localStorage.removeItem('aao_engine_config');
	})();

	// --- getAll() returns full config (deep clone) ---
	(function() {
		var all = EngineConfig.getAll();
		TestHarness.assertType(all, 'object', 'getAll() returns an object');
		TestHarness.assertDefined(all.controls, 'getAll() has controls section');
		TestHarness.assertDefined(all.display, 'getAll() has display section');
		TestHarness.assertDefined(all.layout, 'getAll() has layout section');
		TestHarness.assertDefined(all.features, 'getAll() has features section');
		TestHarness.assertDefined(all.accessibility, 'getAll() has accessibility section');
		TestHarness.assertDefined(all.theme, 'getAll() has theme section');

		all.display.mute = true;
		TestHarness.assertEqual(EngineConfig.get('display.mute'), false, 'getAll() returns deep clone — mutation safe');
	})();

	// --- getDefault() returns default value ---
	(function() {
		EngineConfig.set('display.mute', true);
		TestHarness.assertEqual(EngineConfig.getDefault('display.mute'), false, 'getDefault returns original default even after set()');
		EngineConfig.reset();
		window.localStorage.removeItem('aao_engine_config');
	})();

	// --- loadCaseConfig() merges without persisting ---
	(function() {
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig.reset();

		EngineConfig.loadCaseConfig({
			display: { nightMode: true },
			features: { debugPanel: true }
		});

		TestHarness.assertEqual(EngineConfig.get('display.nightMode'), true, 'loadCaseConfig merges display.nightMode');
		TestHarness.assertEqual(EngineConfig.get('features.debugPanel'), true, 'loadCaseConfig merges features.debugPanel');
		TestHarness.assertEqual(EngineConfig.get('display.mute'), false, 'loadCaseConfig preserves unaffected values');

		var stored = window.localStorage.getItem('aao_engine_config');
		TestHarness.assert(stored === null, 'loadCaseConfig does not persist to localStorage');

		EngineConfig.reset();
	})();

	// --- loadCaseConfig emits config:changed ---
	(function() {
		var received = null;
		var handler = function(data) { received = data; };
		EngineEvents.on('config:changed', handler);

		EngineConfig.loadCaseConfig({ display: { pixelated: true } });
		TestHarness.assert(received !== null, 'loadCaseConfig emits config:changed');

		EngineEvents.off('config:changed', handler);
		EngineConfig.reset();
	})();

	// Final cleanup
	EngineConfig.reset();
	window.localStorage.removeItem('aao_engine_config');
}
