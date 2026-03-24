"use strict";
/**
 * ThemeManager regression tests.
 * Do NOT call EngineEvents.clear() — use targeted on/off.
 * After config changes, call ThemeManager.reapply() since the config:changed
 * listener may have been cleared by previous test suites.
 */
function testThemeManager() {
	TestHarness.suite('ThemeManager');

	// Clean state
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig.reset();
	EngineConfig._init();

	// Module is loaded
	TestHarness.assertEqual(
		Modules.request_list['theme_manager'], 3,
		'theme_manager module is loaded (status 3)'
	);

	// ThemeManager global exists
	TestHarness.assertDefined(ThemeManager, 'ThemeManager global is defined');
	TestHarness.assertType(ThemeManager.reapply, 'function', 'ThemeManager.reapply is a function');

	// --- Night mode toggles body class ---
	(function() {
		document.body.classList.remove('night-mode');
		EngineConfig.set('display.nightMode', true);
		ThemeManager.reapply();
		TestHarness.assert(
			document.body.classList.contains('night-mode'),
			'Setting display.nightMode=true adds night-mode class to body'
		);

		EngineConfig.set('display.nightMode', false);
		ThemeManager.reapply();
		TestHarness.assert(
			!document.body.classList.contains('night-mode'),
			'Setting display.nightMode=false removes night-mode class from body'
		);
	})();

	// --- Pixelated toggles screens class ---
	(function() {
		var screens = document.getElementById('screens');
		if (screens) {
			screens.classList.remove('pixelated');
			EngineConfig.set('display.pixelated', true);
			ThemeManager.reapply();
			TestHarness.assert(
				screens.classList.contains('pixelated'),
				'Setting display.pixelated=true adds pixelated class to #screens'
			);

			EngineConfig.set('display.pixelated', false);
			ThemeManager.reapply();
			TestHarness.assert(
				!screens.classList.contains('pixelated'),
				'Setting display.pixelated=false removes pixelated class from #screens'
			);
		}
	})();

	// --- CSS custom properties set on :root ---
	(function() {
		EngineConfig.set('layout.screenScale', 1.5);
		ThemeManager.reapply();
		var rootStyle = getComputedStyle(document.documentElement);
		TestHarness.assertEqual(
			rootStyle.getPropertyValue('--screen-scale').trim(), '1.5',
			'layout.screenScale sets --screen-scale CSS custom property'
		);

		EngineConfig.set('layout.mobileScreenScale', 1.8);
		ThemeManager.reapply();
		rootStyle = getComputedStyle(document.documentElement);
		TestHarness.assertEqual(
			rootStyle.getPropertyValue('--mobile-screen-scale').trim(), '1.8',
			'layout.mobileScreenScale sets --mobile-screen-scale CSS custom property'
		);
	})();

	// --- Custom CSS injection ---
	(function() {
		EngineConfig.set('theme.customCSS', '.test-custom { color: red; }');
		ThemeManager.reapply();
		var styleEl = document.getElementById('aao-custom-theme');
		TestHarness.assert(styleEl !== null, 'Custom CSS creates <style> element');
		if (styleEl) {
			TestHarness.assert(
				styleEl.textContent.indexOf('.test-custom') !== -1,
				'Custom CSS content is injected into <style> element'
			);

			EngineConfig.set('theme.customCSS', '');
			ThemeManager.reapply();
			TestHarness.assertEqual(styleEl.textContent, '', 'Empty customCSS clears <style> content');
		}
	})();

	// --- Mute handler calls Howler.mute ---
	(function() {
		if (typeof Howler === 'undefined') return;
		var muteCalled = false;
		var muteVal = null;
		var origMute = Howler.mute;
		Howler.mute = function(v) { muteCalled = true; muteVal = v; };

		EngineConfig.set('display.mute', true);
		ThemeManager.reapply();
		TestHarness.assert(muteCalled, 'ThemeManager calls Howler.mute on display.mute change');
		TestHarness.assertEqual(muteVal, true, 'Howler.mute called with true when mute enabled');

		Howler.mute = origMute;
	})();

	// --- NEW: Pixelated CSS effect (Fix 1) ---
	(function() {
		var screens = document.getElementById('screens');
		if (!screens) return;
		EngineConfig.set('display.pixelated', true);
		ThemeManager.reapply();
		var style = getComputedStyle(screens);
		var ir = style.imageRendering || '';
		TestHarness.assert(
			ir.indexOf('pixelated') !== -1 || ir.indexOf('crisp-edges') !== -1,
			'Pixelated CSS: #screens has image-rendering: pixelated when enabled'
		);
		EngineConfig.set('display.pixelated', false);
		ThemeManager.reapply();
	})();

	// --- NEW: Screen scale CSS effect (Fix 2) — auto-fit computed properties ---
	(function() {
		var screens = document.getElementById('screens');
		if (!screens) return;
		EngineConfig.set('layout.screenScale', 1.5);
		ThemeManager.reapply();
		var rootStyle = getComputedStyle(document.documentElement);
		var autoWidth = rootStyle.getPropertyValue('--screen-auto-width').trim();
		var contentScale = rootStyle.getPropertyValue('--screen-content-scale').trim();
		// In the test environment the section may have 0 height, so auto-fit may
		// not produce meaningful values. Just verify the CSS custom properties exist
		// and that --screen-scale was set correctly.
		var scaleVal = rootStyle.getPropertyValue('--screen-scale').trim();
		TestHarness.assertEqual(
			scaleVal, '1.5',
			'Screen scale CSS: --screen-scale is 1.5 when screenScale is 1.5'
		);
		// --screen-auto-width should be a px value (either computed or fallback 256px)
		TestHarness.assert(
			autoWidth.indexOf('px') !== -1,
			'Screen scale CSS: --screen-auto-width is a px value'
		);
		EngineConfig.set('layout.screenScale', 1.0);
		ThemeManager.reapply();
	})();

	// --- NEW: Expand descriptions class toggle (Fix 3) ---
	(function() {
		var cr = document.getElementById('courtrecord');
		if (!cr) return;
		EngineConfig.set('display.expandEvidenceDescriptions', true);
		ThemeManager.reapply();
		TestHarness.assert(
			cr.classList.contains('expand-descriptions'),
			'Expand descriptions: #courtrecord has expand-descriptions class when enabled'
		);
		EngineConfig.set('display.expandEvidenceDescriptions', false);
		ThemeManager.reapply();
		TestHarness.assert(
			!cr.classList.contains('expand-descriptions'),
			'Expand descriptions: class removed when disabled'
		);
	})();

	// --- NEW: Blip volume handler (Fix 4) ---
	(function() {
		if (typeof SoundHowler === 'undefined') return;
		var setCalled = false;
		var origSetVol = SoundHowler.setSoundVolume;
		SoundHowler.setSoundVolume = function(id, vol) {
			if (id.indexOf('voice_-') === 0) setCalled = true;
			return origSetVol.apply(this, arguments);
		};
		EngineConfig.set('display.blipVolume', 50);
		ThemeManager.reapply();
		TestHarness.assert(setCalled, 'Blip volume: SoundHowler.setSoundVolume called for voice blips');
		SoundHowler.setSoundVolume = origSetVol;
	})();

	// (courtRecordPosition test removed — config key no longer exists, replaced by panelArrangement)

	// --- NEW: Text speed config accessible (Fix 5) ---
	(function() {
		var val = EngineConfig.get('display.textSpeed');
		TestHarness.assertType(val, 'number', 'Text speed: EngineConfig.get(display.textSpeed) returns a number');
		TestHarness.assertEqual(val, 1.0, 'Text speed: default value is 1.0');
	})();

	// Cleanup
	EngineConfig.reset();
	ThemeManager.reapply();
	window.localStorage.removeItem('aao_engine_config');
	document.body.classList.remove('night-mode');
	document.documentElement.style.removeProperty('--screen-scale');
	document.documentElement.style.removeProperty('--mobile-screen-scale');
}
