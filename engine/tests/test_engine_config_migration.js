"use strict";
/**
 * EngineConfig migration and pruning tests.
 * Tests that stale localStorage keys are cleaned up on load.
 */
function testEngineConfigMigration() {
	TestHarness.suite('EngineConfig Migration');

	// --- pruneToDefaults prevents stale keys from reaching config ---
	(function() {
		window.localStorage.setItem('aao_engine_config', JSON.stringify({
			display: { nightMode: true, staleKey: 'should be removed' },
			layout: { screenScale: 1.0, nonExistent: 42 }
		}));
		EngineConfig._init();
		// Stale keys should NOT be accessible via get() — pruneToDefaults strips them
		// before merging into config.
		TestHarness.assert(EngineConfig.get('display.staleKey') === undefined, 'pruneToDefaults: staleKey not accessible via get()');
		TestHarness.assert(EngineConfig.get('layout.nonExistent') === undefined, 'pruneToDefaults: nonExistent not accessible via get()');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- migrateStorage maps old courtRecordPosition ---
	(function() {
		window.localStorage.setItem('aao_engine_config', JSON.stringify({
			layout: { courtRecordPosition: 'left' }
		}));
		EngineConfig._init();
		var val = EngineConfig.get('layout.courtRecordPosition');
		TestHarness.assert(val === undefined, 'migrateStorage: courtRecordPosition is removed after migration');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- migrateStorage removes settingsPosition and fullWidth ---
	(function() {
		window.localStorage.setItem('aao_engine_config', JSON.stringify({
			layout: { settingsPosition: 'auto', fullWidth: true }
		}));
		EngineConfig._init();
		TestHarness.assert(EngineConfig.get('layout.settingsPosition') === undefined, 'migrateStorage: settingsPosition removed');
		TestHarness.assert(EngineConfig.get('layout.fullWidth') === undefined, 'migrateStorage: fullWidth removed');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- pruneToDefaults preserves valid keys that ARE in defaults ---
	(function() {
		window.localStorage.setItem('aao_engine_config', JSON.stringify({
			display: { nightMode: false }
		}));
		EngineConfig._init();
		TestHarness.assertEqual(EngineConfig.get('display.nightMode'), false, 'pruneToDefaults: preserves valid nightMode override');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- Valid config persists across _init() reload ---
	(function() {
		EngineConfig.set('display.nightMode', false);
		EngineConfig._init();
		TestHarness.assertEqual(EngineConfig.get('display.nightMode'), false, 'Config value persists across _init() reload');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- Default values accessible for all top-level sections ---
	(function() {
		TestHarness.assertType(EngineConfig.get('controls'), 'object', 'Default: controls is an object');
		TestHarness.assertType(EngineConfig.get('display'), 'object', 'Default: display is an object');
		TestHarness.assertType(EngineConfig.get('layout'), 'object', 'Default: layout is an object');
		TestHarness.assertType(EngineConfig.get('features'), 'object', 'Default: features is an object');
		TestHarness.assertType(EngineConfig.get('accessibility'), 'object', 'Default: accessibility is an object');
		TestHarness.assertType(EngineConfig.get('theme'), 'object', 'Default: theme is an object');
	})();

	// --- Plugin namespace: set and get ---
	(function() {
		EngineConfig.set('plugins.myPlugin.foo', 'bar');
		TestHarness.assertEqual(EngineConfig.get('plugins.myPlugin.foo'), 'bar', 'Plugin config: set and get works');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- Plugin namespace: survives _init() reload ---
	(function() {
		EngineConfig.set('plugins.testPlugin.enabled', true);
		EngineConfig.set('plugins.testPlugin.volume', 50);
		EngineConfig._init(); // reload
		TestHarness.assertEqual(EngineConfig.get('plugins.testPlugin.enabled'), true, 'Plugin config: enabled survives reload');
		TestHarness.assertEqual(EngineConfig.get('plugins.testPlugin.volume'), 50, 'Plugin config: volume survives reload');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- Non-plugin stale keys still pruned after reload with plugin data ---
	(function() {
		window.localStorage.setItem('aao_engine_config', JSON.stringify({
			display: { staleAgain: 'remove me' },
			plugins: { myPlugin: { keepMe: true } }
		}));
		EngineConfig._init();
		TestHarness.assert(EngineConfig.get('display.staleAgain') === undefined, 'Stale keys pruned even when plugins namespace exists');
		TestHarness.assertEqual(EngineConfig.get('plugins.myPlugin.keepMe'), true, 'Plugin data preserved while stale keys pruned');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// --- plugins key exists in defaults ---
	(function() {
		var plugins = EngineConfig.get('plugins');
		TestHarness.assertType(plugins, 'object', 'Default: plugins is an object');
	})();

	// Cleanup
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig._init();
}
