"use strict";
/**
 * SettingsPanel regression tests.
 * Do NOT call EngineEvents.clear() — use targeted on/off.
 */
function testSettingsPanel() {
	TestHarness.suite('SettingsPanel');

	// Clean state
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig.reset();
	EngineConfig._init();

	// Module is loaded
	TestHarness.assertEqual(
		Modules.request_list['settings_panel'], 3,
		'settings_panel module is loaded (status 3)'
	);

	// SettingsPanel global exists
	TestHarness.assertDefined(SettingsPanel, 'SettingsPanel global is defined');

	// Re-init the panel since the DOM was rebuilt after module init
	var container = document.getElementById('player_settings');
	if (container) {
		SettingsPanel._init();
	}

	// --- Settings controls render in #player_settings ---
	(function() {
		if (!container) {
			TestHarness.assert(false, 'Settings container #player_settings not found (skipping)');
			return;
		}

		// Check that sections were created (using <details> elements)
		var sections = container.querySelectorAll('details');
		TestHarness.assert(sections.length >= 3, 'Settings panel has at least 3 sections (Display, Layout, Controls)');

		// Check that checkboxes were created
		var checkboxes = container.querySelectorAll('input[type="checkbox"]');
		TestHarness.assert(checkboxes.length >= 4, 'Settings panel has at least 4 checkboxes');

		// Check that sliders were created
		var sliders = container.querySelectorAll('input[type="range"]');
		TestHarness.assert(sliders.length >= 2, 'Settings panel has at least 2 sliders');

		// Check that a select dropdown exists
		var selects = container.querySelectorAll('select');
		TestHarness.assert(selects.length >= 1, 'Settings panel has at least 1 select dropdown');

		// Check that reset button exists
		var buttons = container.querySelectorAll('button');
		TestHarness.assert(buttons.length >= 1, 'Settings panel has at least 1 button (reset)');
	})();

	// --- Toggle checkbox updates config ---
	(function() {
		if (!container) return;

		var checkboxes = container.querySelectorAll('input[type="checkbox"]');
		if (checkboxes.length === 0) return;

		var muteCheckbox = checkboxes[0];
		var origValue = EngineConfig.get('display.mute');

		muteCheckbox.checked = !origValue;
		muteCheckbox.dispatchEvent(new Event('change', { bubbles: true }));

		TestHarness.assertEqual(
			EngineConfig.get('display.mute'), !origValue,
			'Toggling mute checkbox updates EngineConfig'
		);

		EngineConfig.set('display.mute', origValue);
	})();

	// Cleanup
	EngineConfig.reset();
	window.localStorage.removeItem('aao_engine_config');
}
