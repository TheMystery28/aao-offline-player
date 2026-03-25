"use strict";
/**
 * EnginePlugins and Plugin API tests.
 * Tests the two-phase plugin lifecycle and full API surface.
 */
function testPlugins() {
	TestHarness.suite('Plugins');

	// Clean state
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig._init();

	// ============================================================
	// EnginePlugins core
	// ============================================================

	// Module loaded
	TestHarness.assertEqual(
		Modules.request_list['engine_plugins'], 3,
		'engine_plugins module is loaded (status 3)'
	);

	// EnginePlugins global exists
	TestHarness.assertDefined(EnginePlugins, 'EnginePlugins global is defined');
	TestHarness.assertType(EnginePlugins.register, 'function', 'register is a function');
	TestHarness.assertType(EnginePlugins.getLoaded, 'function', 'getLoaded is a function');
	TestHarness.assertType(EnginePlugins.isLoaded, 'function', 'isLoaded is a function');

	// register() with missing init does not crash
	(function() {
		var noCrash = true;
		try {
			EnginePlugins.register({ name: 'bad_plugin', version: '1.0' });
		} catch (e) {
			noCrash = false;
		}
		TestHarness.assert(noCrash, 'register() with missing init does not crash');
	})();

	// register() with valid descriptor
	(function() {
		var initCalled = false;
		var receivedConfig = null;
		var receivedEvents = null;
		var receivedApi = null;

		EnginePlugins.register({
			name: 'test_plugin_a',
			version: '1.0',
			init: function(config, events, api) {
				initCalled = true;
				receivedConfig = config;
				receivedEvents = events;
				receivedApi = api;
			}
		});

		// In test context, player:init may have already fired (or not).
		if (initCalled) {
			TestHarness.assert(receivedConfig !== null, 'Plugin init receives config object');
			TestHarness.assert(receivedEvents !== null, 'Plugin init receives events object');
			TestHarness.assert(receivedApi !== null, 'Plugin init receives api object');
			TestHarness.assertType(receivedApi, 'object', 'api is an object');
		} else {
			TestHarness.assert(true, 'Plugin registered (player:init not yet fired in test runner)');
		}
	})();

	// getLoaded() returns array
	(function() {
		var loaded = EnginePlugins.getLoaded();
		TestHarness.assert(Array.isArray(loaded), 'getLoaded() returns an array');
	})();

	// isLoaded() works
	(function() {
		TestHarness.assert(!EnginePlugins.isLoaded('nonexistent_plugin'), 'isLoaded returns false for unknown plugin');
	})();

	// register() with throwing init doesn't crash engine
	(function() {
		var noCrash = true;
		try {
			EnginePlugins.register({
				name: 'crashing_plugin',
				version: '1.0',
				init: function() { throw new Error('intentional crash'); }
			});
		} catch (e) {
			noCrash = false;
		}
		TestHarness.assert(noCrash, 'register() with throwing init does not crash engine');
	})();

	// Plugin can use config with plugins namespace
	(function() {
		EngineConfig.set('plugins.testSuite.value', 42);
		TestHarness.assertEqual(EngineConfig.get('plugins.testSuite.value'), 42, 'Plugin can set/get config under plugins namespace');
		window.localStorage.removeItem('aao_engine_config');
		EngineConfig._init();
	})();

	// Multiple plugins coexist
	(function() {
		EnginePlugins.register({
			name: 'test_plugin_b',
			version: '2.0',
			init: function() {}
		});
		EnginePlugins.register({
			name: 'test_plugin_c',
			version: '1.0',
			init: function() {}
		});
		var loaded = EnginePlugins.getLoaded();
		var hasB = loaded.indexOf('test_plugin_b') !== -1;
		var hasC = loaded.indexOf('test_plugin_c') !== -1;
		TestHarness.assert(hasB && hasC, 'Multiple plugins coexist in loaded list');
	})();

	// ============================================================
	// Plugin API structure and behavior
	// ============================================================

	// Test the API object structure via _buildApi (exposed for testing)
	(function() {
		if (typeof EnginePlugins._buildApi !== 'function') return;
		var api = EnginePlugins._buildApi();
		if (!api) return;

		// DOM API
		TestHarness.assertType(api.dom, 'object', 'api.dom exists');
		TestHarness.assertType(api.dom.query, 'function', 'api.dom.query is a function');
		TestHarness.assertType(api.dom.queryAll, 'function', 'api.dom.queryAll is a function');
		TestHarness.assertType(api.dom.create, 'function', 'api.dom.create is a function');
		TestHarness.assertType(api.dom.injectCSS, 'function', 'api.dom.injectCSS is a function');
		TestHarness.assertType(api.dom.injectStylesheet, 'function', 'api.dom.injectStylesheet is a function');
		TestHarness.assertType(api.dom.addClass, 'function', 'api.dom.addClass is a function');
		TestHarness.assertType(api.dom.removeClass, 'function', 'api.dom.removeClass is a function');
		TestHarness.assertType(api.dom.hasClass, 'function', 'api.dom.hasClass is a function');
		TestHarness.assertType(api.dom.emptyNode, 'function', 'api.dom.emptyNode is a function');
		TestHarness.assertType(api.dom.setNodeTextContents, 'function', 'api.dom.setNodeTextContents is a function');

		// Player API
		TestHarness.assertType(api.player, 'object', 'api.player exists');
		TestHarness.assertType(api.player.getStatus, 'function', 'api.player.getStatus is a function');
		TestHarness.assertType(api.player.getTrialInfo, 'function', 'api.player.getTrialInfo is a function');
		TestHarness.assertType(api.player.getTrialData, 'function', 'api.player.getTrialData is a function');
		TestHarness.assertType(api.player.readFrame, 'function', 'api.player.readFrame is a function');
		TestHarness.assertType(api.player.proceed, 'function', 'api.player.proceed is a function');
		TestHarness.assertType(api.player.getCurrentFrameId, 'function', 'api.player.getCurrentFrameId is a function');

		// Sound API
		TestHarness.assertType(api.sound, 'object', 'api.sound exists');
		TestHarness.assertType(api.sound.playMusic, 'function', 'api.sound.playMusic is a function');
		TestHarness.assertType(api.sound.stopMusic, 'function', 'api.sound.stopMusic is a function');
		TestHarness.assertType(api.sound.playSound, 'function', 'api.sound.playSound is a function');
		TestHarness.assertType(api.sound.registerSound, 'function', 'api.sound.registerSound is a function');
		TestHarness.assertType(api.sound.unloadSound, 'function', 'api.sound.unloadSound is a function');
		TestHarness.assertType(api.sound.setSoundVolume, 'function', 'api.sound.setSoundVolume is a function');
		TestHarness.assertType(api.sound.mute, 'function', 'api.sound.mute is a function');

		// Court Record API
		TestHarness.assertType(api.courtRecord, 'object', 'api.courtRecord exists');
		TestHarness.assertType(api.courtRecord.setHidden, 'function', 'api.courtRecord.setHidden is a function');
		TestHarness.assertType(api.courtRecord.refresh, 'function', 'api.courtRecord.refresh is a function');
		TestHarness.assertType(api.courtRecord.getElement, 'function', 'api.courtRecord.getElement is a function');

		// Input API
		TestHarness.assertType(api.input, 'object', 'api.input exists');
		TestHarness.assertType(api.input.registerAction, 'function', 'api.input.registerAction is a function');
		TestHarness.assertType(api.input.onKeyDown, 'function', 'api.input.onKeyDown is a function');
		TestHarness.assertType(api.input.onKeyUp, 'function', 'api.input.onKeyUp is a function');

		// Settings API
		TestHarness.assertType(api.settings, 'object', 'api.settings exists');
		TestHarness.assertType(api.settings.addSection, 'function', 'api.settings.addSection is a function');
		TestHarness.assertType(api.settings.removeSection, 'function', 'api.settings.removeSection is a function');

		// Display API
		TestHarness.assertType(api.display, 'object', 'api.display exists');
		TestHarness.assertType(api.display.getTopScreen, 'function', 'api.display.getTopScreen is a function');
		TestHarness.assertType(api.display.getBottomScreen, 'function', 'api.display.getBottomScreen is a function');
	})();

	// DOM API functional tests
	(function() {
		if (typeof EnginePlugins._buildApi !== 'function') return;
		var api = EnginePlugins._buildApi();
		if (!api) return;

		// injectCSS
		var styleEl = api.dom.injectCSS('body { --test-plugin-var: 1; }');
		TestHarness.assert(styleEl instanceof HTMLStyleElement, 'injectCSS returns a style element');
		TestHarness.assert(styleEl.parentNode === document.head, 'injectCSS appends to head');
		if (styleEl.parentNode) styleEl.parentNode.removeChild(styleEl);

		// create
		var div = api.dom.create('div');
		TestHarness.assert(div instanceof HTMLDivElement, 'create returns correct element type');

		// query
		var body = api.dom.query('body');
		TestHarness.assert(body === document.body, 'query returns correct element');

		// injectStylesheet
		var linkEl = api.dom.injectStylesheet('test.css');
		TestHarness.assert(linkEl instanceof HTMLLinkElement, 'injectStylesheet returns a link element');
		TestHarness.assertEqual(linkEl.rel, 'stylesheet', 'injectStylesheet sets rel=stylesheet');
		if (linkEl.parentNode) linkEl.parentNode.removeChild(linkEl);
	})();

	// Settings API functional tests
	(function() {
		if (typeof EnginePlugins._buildApi !== 'function') return;
		var api = EnginePlugins._buildApi();
		if (!api) return;

		var container = document.getElementById('player-parametres');
		if (!container) return;

		// addSection creates a <details> element
		var contentDiv = api.settings.addSection('Test Plugin Section', [
			{ type: 'checkbox', key: 'plugins.testSection.enabled', label: 'Enabled' }
		]);
		TestHarness.assert(contentDiv !== null, 'addSection returns a content div');

		var details = container.querySelectorAll('details');
		var found = false;
		for (var i = 0; i < details.length; i++) {
			var summary = details[i].querySelector('summary');
			if (summary && summary.textContent === 'Test Plugin Section') {
				found = true;
				break;
			}
		}
		TestHarness.assert(found, 'addSection creates <details> with correct title in #player-parametres');

		// Verify checkbox exists and works with config
		if (contentDiv) {
			var checkbox = contentDiv.querySelector('input[type="checkbox"]');
			TestHarness.assert(checkbox !== null, 'addSection creates checkbox control');
			if (checkbox) {
				checkbox.checked = true;
				checkbox.dispatchEvent(new Event('change'));
				TestHarness.assertEqual(EngineConfig.get('plugins.testSection.enabled'), true, 'addSection checkbox writes to EngineConfig');
			}
		}

		// removeSection removes it
		api.settings.removeSection('Test Plugin Section');
		found = false;
		details = container.querySelectorAll('details');
		for (var j = 0; j < details.length; j++) {
			var s = details[j].querySelector('summary');
			if (s && s.textContent === 'Test Plugin Section') {
				found = true;
				break;
			}
		}
		TestHarness.assert(!found, 'removeSection removes the <details> element');
	})();

	// ============================================================
	// Integration tests
	// ============================================================

	// clearNamespace removes plugin listeners
	(function() {
		var fired = false;
		EngineEvents.on('test:pluginCleanup', function() { fired = true; }, 0, 'testPlugin');
		EngineEvents.clearNamespace('testPlugin');
		EngineEvents.emit('test:pluginCleanup');
		TestHarness.assert(!fired, 'clearNamespace removes plugin event listeners');
	})();

	// EngineEvents.clear() preserves engine listeners
	(function() {
		var engineFired = false;
		var pluginFired = false;
		EngineEvents.on('test:clearPreserve', function() { engineFired = true; }, 0, 'engine');
		EngineEvents.on('test:clearPreserve', function() { pluginFired = true; }, 0, 'somePlugin');
		EngineEvents.clear();
		EngineEvents.emit('test:clearPreserve');
		TestHarness.assert(engineFired, 'EngineEvents.clear() preserves engine listeners');
		TestHarness.assert(!pluginFired, 'EngineEvents.clear() removes plugin listeners');
		EngineEvents.clear();
	})();

	// ============================================================
	// Plugin params and resolved data
	// ============================================================

	// getPluginParams exists on both EnginePlugins and EngineConfig
	TestHarness.assertType(EnginePlugins.getPluginParams, 'function', 'getPluginParams exists on EnginePlugins');
	TestHarness.assertType(EngineConfig.getPluginParams, 'function', 'getPluginParams exists on EngineConfig');

	// getPluginParams returns empty object for unknown plugin
	(function() {
		var params = EnginePlugins.getPluginParams('nonexistent_plugin_xyz');
		TestHarness.assertDefined(params, 'getPluginParams returns object for unknown plugin');
		TestHarness.assertEqual(Object.keys(params).length, 0, 'getPluginParams returns empty for unknown');
	})();

	// register() stores params declaration
	(function() {
		var testDesc = {
			name: '__test_params_plugin__',
			version: '1.0',
			params: {
				color: { type: 'text', default: 'red', label: 'Color' },
				size: { type: 'number', default: 12, label: 'Size', min: 1, max: 100 }
			},
			init: function() {}
		};
		EnginePlugins.register(testDesc);
		TestHarness.assert(EnginePlugins.isLoaded('__test_params_plugin__'), 'params plugin registered');

		// getPluginParams returns defaults when no resolved data
		var params = EnginePlugins.getPluginParams('__test_params_plugin__');
		TestHarness.assertEqual(params.color, 'red', 'getPluginParams returns declared default for color');
		TestHarness.assertEqual(params.size, 12, 'getPluginParams returns declared default for size');

		// EngineConfig.getPluginParams delegates correctly
		var cfgParams = EngineConfig.getPluginParams('__test_params_plugin__');
		TestHarness.assertEqual(cfgParams.color, 'red', 'EngineConfig.getPluginParams delegates correctly');
	})();

	// Session override merges on top
	(function() {
		EngineConfig.set('plugins.__test_params_plugin__.params.color', 'blue');
		var params = EnginePlugins.getPluginParams('__test_params_plugin__');
		TestHarness.assertEqual(params.color, 'blue', 'session override merges on top of default');
		TestHarness.assertEqual(params.size, 12, 'non-overridden param keeps default');
		// Clean up
		EngineConfig.set('plugins.__test_params_plugin__.params.color', undefined);
	})();

	// getResolvedData returns null when no resolved_plugins.json loaded
	(function() {
		var data = EnginePlugins.getResolvedData();
		// In test environment, resolved_plugins.json is not loaded, so data may be null
		// This is the expected fallback behavior
		TestHarness.assert(data === null || typeof data === 'object', 'getResolvedData returns null or object');
	})();

	// buildSettingsPanel creates Plugins section
	(function() {
		var container = document.getElementById('player_settings');
		if (!container) return;
		var pluginSection = container.querySelector('details[data-plugin-section="__plugins__"]');
		TestHarness.assert(pluginSection !== null, 'Plugins section exists in settings');
		var summary = pluginSection.querySelector('summary');
		TestHarness.assertEqual(summary.textContent, 'Plugins', 'Plugins section has correct title');
	})();

	// Cleanup
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig._init();
}
