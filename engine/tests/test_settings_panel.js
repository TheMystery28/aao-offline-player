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

	// --- specific sliders exist ---
	(function() {
		if (!container) return;
		var sliders = container.querySelectorAll('input[type="range"]');
		var labels = [];
		for (var i = 0; i < sliders.length; i++) {
			var label = sliders[i].closest('.regular_label');
			if (label) {
				var span = label.querySelector('[data-locale-content]');
				if (span) labels.push(span.getAttribute('data-locale-content'));
			}
		}
		TestHarness.assert(labels.indexOf('body_width') !== -1, 'Body width slider exists');
		TestHarness.assert(labels.indexOf('screen_scale') !== -1, 'Screen scale slider exists');
		TestHarness.assert(labels.indexOf('evidence_width') !== -1, 'Evidence width slider exists');
		TestHarness.assert(labels.indexOf('settings_width') !== -1, 'Settings width slider exists');
	})();

	// --- layout picker ---
	(function() {
		if (!container) return;
		var picker = container.querySelector('.layout-picker');
		TestHarness.assert(picker !== null, 'Layout picker container exists');
		var thumbs = container.querySelectorAll('.layout-thumb.row-layout');
		TestHarness.assertEqual(thumbs.length, 6, 'Layout picker has 6 row thumbnails');
	})();

	// --- narrowMode select ---
	(function() {
		if (!container) return;
		var selects = container.querySelectorAll('select');
		var nmSelect = null;
		for (var i = 0; i < selects.length; i++) {
			var label = selects[i].closest('.regular_label');
			if (label && label.querySelector('[data-locale-content="narrow_mode"]')) {
				nmSelect = selects[i];
				break;
			}
		}
		TestHarness.assert(nmSelect !== null, 'narrowMode select exists');
		if (nmSelect) {
			TestHarness.assertEqual(nmSelect.options.length, 2, 'narrowMode select has 2 options (tabs, stack)');
		}
	})();

	// --- body width slider dynamic bounds ---
	(function() {
		if (!container) return;
		var bwLabel = container.querySelector('[data-locale-content="body_width"]');
		if (!bwLabel) return;
		var slider = bwLabel.closest('.regular_label').querySelector('input[type="range"]');
		if (!slider) return;
		var min = parseFloat(slider.min);
		var max = parseFloat(slider.max);
		TestHarness.assert(!isNaN(min), 'Body width slider min is a number');
		TestHarness.assert(!isNaN(max), 'Body width slider max is a number');
		TestHarness.assert(min > 0, 'Body width slider min > 0');
		TestHarness.assert(max <= 2.0 && max > 0, 'Body width slider max is a valid positive number (got ' + max + ')');
	})();

	// --- hide header and fullscreen checkboxes ---
	(function() {
		if (!container) return;
		var allLabels = container.querySelectorAll('[data-locale-content]');
		var found = {};
		for (var i = 0; i < allLabels.length; i++) {
			found[allLabels[i].getAttribute('data-locale-content')] = true;
		}
		TestHarness.assert(found['hide_header'] === true, 'Hide header checkbox exists');
		TestHarness.assert(found['fullscreen'] === true, 'Fullscreen checkbox exists');
	})();

	// --- updateLayoutTier visibility ---
	(function() {
		if (!container) return;
		SettingsPanel.updateLayoutTier('wide', true);
		var picker = container.querySelector('.layout-picker');
		TestHarness.assert(picker && picker.style.display !== 'none', 'updateLayoutTier wide: picker visible');

		SettingsPanel.updateLayoutTier('narrow', false);
		var layoutDetails = container.querySelector('details:nth-of-type(2)');
		if (layoutDetails) {
			TestHarness.assertEqual(layoutDetails.style.display, 'none', 'updateLayoutTier narrow: Layout section hidden');
		}
		// Restore
		SettingsPanel.updateLayoutTier('wide', true);
	})();

	// --- Regression: pin behavior ---
	(function() {
		if (!container) return;
		// Reset button exists
		var buttons = container.querySelectorAll('button');
		var resetBtn = null;
		for (var i = 0; i < buttons.length; i++) {
			if (buttons[i].textContent.indexOf('Reset') !== -1) { resetBtn = buttons[i]; break; }
		}
		TestHarness.assert(resetBtn !== null, 'Regression: Reset button exists');

		// Display section has expected control count
		var displayDetails = container.querySelector('details:nth-of-type(1)');
		if (displayDetails) {
			var controls = displayDetails.querySelectorAll('input, select');
			TestHarness.assert(controls.length >= 9, 'Regression: Display section has >= 9 controls (got ' + controls.length + ')');
		}
	})();

	// --- updateLayoutTier with tabs+wideIsPossible ---
	(function() {
		if (!container) return;
		SettingsPanel.updateLayoutTier('tabs', true);
		var nmLabel = container.querySelector('[data-locale-content="narrow_mode"]');
		var nmWrapper = nmLabel ? nmLabel.closest('.regular_label') : null;
		if (nmWrapper) {
			TestHarness.assert(nmWrapper.style.display !== 'none', 'updateLayoutTier tabs+wideIsPossible: narrowMode visible');
		}
		SettingsPanel.updateLayoutTier('wide', true);
	})();

	// --- tab mode hides row-picker legend, keeps tab legend ---
	(function() {
		if (!container) return;
		var rowLegend = container.querySelector('.layout-picker-wrapper .layout-picker-legend');
		var tabLegend = container.querySelector('.tab-position-picker .layout-picker-legend');
		if (!rowLegend || !tabLegend) return;
		SettingsPanel.updateLayoutTier('tabs', true);
		TestHarness.assertEqual(getComputedStyle(rowLegend).display, 'none', 'tab mode: Screen/Evidence/Settings legend hidden');
		TestHarness.assert(getComputedStyle(tabLegend).display !== 'none', 'tab mode: Screen/Tabs legend visible');
		SettingsPanel.updateLayoutTier('wide', true);
		TestHarness.assert(getComputedStyle(rowLegend).display !== 'none', 'wide mode: Screen/Evidence/Settings legend visible');
	})();

	// Cleanup
	EngineConfig.reset();
	window.localStorage.removeItem('aao_engine_config');

	// --- tab position picker ---
	(function() {
		if (!container) return;
		// Fresh panel build with defaults only — the default thumb must already be
		// selected/outlined (row-picker parity), before any user interaction.
		EngineConfig.reset();
		window.localStorage.removeItem('aao_engine_config');
		SettingsPanel._init();
		var freshPicker = document.querySelector('.tab-position-picker');
		TestHarness.assert(freshPicker !== null, 'Tab position picker exists after fresh init');
		if (freshPicker) {
			var freshRight = freshPicker.querySelector('.layout-thumb[data-value="right"]');
			TestHarness.assert(
				freshRight && freshRight.classList.contains('selected'),
				'Tab position picker: default right thumb selected on fresh build'
			);
			var freshLeft = freshPicker.querySelector('.layout-thumb[data-value="left"]');
			TestHarness.assert(
				freshLeft && !freshLeft.classList.contains('selected'),
				'Tab position picker: left thumb not selected on fresh build'
			);
		}

		var picker = container.querySelector('.tab-position-picker');
		TestHarness.assert(picker !== null, 'Tab position picker exists');
		if (!picker) return;
		var thumbs = picker.querySelectorAll('.layout-thumb.tab-layout');
		TestHarness.assertEqual(thumbs.length, 2, 'Tab position picker has 2 thumbnails');
		TestHarness.assert(
			container.querySelector('[data-locale-content="tab_position"]') !== null,
			'Tab position label exists'
		);
		TestHarness.assert(
			picker.querySelector('.layout-picker-legend') !== null,
			'Tab position picker has a legend'
		);

		// Selection highlight follows the current config (row-picker parity)
		var leftThumb = picker.querySelector('.layout-thumb[data-value="left"]');
		var rightThumb = picker.querySelector('.layout-thumb[data-value="right"]');
		EngineConfig.set('layout.tabPosition', 'left');
		TestHarness.assert(
			leftThumb && leftThumb.classList.contains('selected'),
			'Tab position picker: left thumb selected when config is left'
		);
		TestHarness.assert(
			rightThumb && !rightThumb.classList.contains('selected'),
			'Tab position picker: right thumb not selected when config is left'
		);
		EngineConfig.set('layout.tabPosition', 'right');
		TestHarness.assert(
			rightThumb && rightThumb.classList.contains('selected'),
			'Tab position picker: right thumb selected when config is right'
		);
		EngineConfig.reset();
	})();

	// --- updateLayoutTier: tab position picker visible only in tabs ---
	(function() {
		if (!container) return;
		var picker = container.querySelector('.tab-position-picker');
		if (!picker) return;

		SettingsPanel.updateLayoutTier('tabs', true);
		TestHarness.assert(
			picker.style.display !== 'none',
			'updateLayoutTier tabs: tab position picker visible'
		);
		// Becoming visible must re-apply the selection so the default thumb is
		// already outlined the moment tab mode shows the picker.
		var rightThumb = picker.querySelector('.layout-thumb[data-value="right"]');
		TestHarness.assert(
			rightThumb && rightThumb.classList.contains('selected'),
			'updateLayoutTier tabs: default thumb selected when picker becomes visible'
		);

		// Even if the selection was lost (panel rebuilt, classes cleared), showing
		// the picker must restore it from the current config.
		var allThumbs = picker.querySelectorAll('.layout-thumb');
		for (var ti = 0; ti < allThumbs.length; ti++) {
			allThumbs[ti].classList.remove('selected');
		}
		SettingsPanel.updateLayoutTier('tabs', true);
		TestHarness.assert(
			rightThumb && rightThumb.classList.contains('selected'),
			'updateLayoutTier tabs: lost selection restored when picker shown'
		);

		SettingsPanel.updateLayoutTier('wide', true);
		TestHarness.assert(
			picker.style.display === 'none',
			'updateLayoutTier wide: tab position picker hidden'
		);

		SettingsPanel.updateLayoutTier('narrow', false);
		TestHarness.assert(
			picker.style.display === 'none',
			'updateLayoutTier narrow: tab position picker hidden'
		);

		// Restore
		SettingsPanel.updateLayoutTier('wide', true);
	})();
}
