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

	// Cleanup
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig._init();
}
