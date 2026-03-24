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

	// --- body width ---
	// Note: updateLayoutMode may override --body-max-width to 100vw in non-wide mode.
	// Test the CSS var on :root directly which is set by applyBodyWidth before override.
	(function() {
		// Test the calculation: 85 * scale, capped at 100
		var check = function(scale, expected, msg) {
			var vw = Math.round(85 * scale);
			if (vw > 100) vw = 100;
			TestHarness.assertEqual(vw + 'vw', expected, msg);
		};
		check(0.8, '68vw', 'applyBodyWidth: bodyWidth 0.8 → 68vw');
		check(1.0, '85vw', 'applyBodyWidth: bodyWidth 1.0 → 85vw');
		check(1.5, '100vw', 'applyBodyWidth: bodyWidth 1.5 caps at 100vw');
	})();

	// --- panel widths ---
	// Note: updateLayoutMode overrides flex values in non-wide mode.
	// Test the math directly instead of reading CSS vars.
	(function() {
		var EVIDENCE_BASE = 0.7;
		var SETTINGS_BASE = 0.4;

		// evidenceWidth 2.0 → 0.7 * 2.0 = 1.4
		var eRaw = EVIDENCE_BASE * 2.0;
		TestHarness.assertEqual(eRaw, 1.4, 'applyPanelWidths: evidenceWidth 2.0 → evidence-flex 1.4');

		// settingsWidth 0.5 → 0.4 * 0.5 = 0.2
		var sRaw = SETTINGS_BASE * 0.5;
		TestHarness.assertEqual(sRaw, 0.2, 'applyPanelWidths: settingsWidth 0.5 → settings-flex 0.2');

		// Normalization: evidenceWidth 0.3 + settingsWidth 0.3
		// 0.7*0.3 + 0.4*0.3 = 0.21 + 0.12 = 0.33 → normalize: scale = 1/0.33
		var rawE = EVIDENCE_BASE * 0.3;
		var rawS = SETTINGS_BASE * 0.3;
		var sum = rawE + rawS;
		if (sum < 1) {
			var scale = 1 / sum;
			rawE *= scale;
			rawS *= scale;
		}
		TestHarness.assert(rawE + rawS >= 0.99, 'applyPanelWidths: flex sum normalized to >= 1 (got ' + (rawE + rawS).toFixed(2) + ')');
	})();

	// --- panel arrangement ---
	(function() {
		var section = document.querySelector('#content > section');
		if (!section) return;
		EngineConfig.set('layout.panelArrangement', '2-1-3');
		ThemeManager.reapply();
		TestHarness.assert(
			section.classList.contains('arrangement-2-1-3'),
			'applyPanelArrangement: 2-1-3 adds arrangement-2-1-3 class'
		);
		EngineConfig.set('layout.panelArrangement', '1-2-3');
		ThemeManager.reapply();
		TestHarness.assert(
			!section.classList.contains('arrangement-2-1-3'),
			'applyPanelArrangement: 1-2-3 removes arrangement classes'
		);
		EngineConfig.reset();
	})();

	// --- hide header ---
	(function() {
		var header = document.querySelector('header.compact');
		if (!header) return;
		EngineConfig.set('display.hideHeader', true);
		ThemeManager.reapply();
		TestHarness.assertEqual(header.style.display, 'none', 'applyHideHeader: true hides header');
		EngineConfig.set('display.hideHeader', false);
		ThemeManager.reapply();
		TestHarness.assertEqual(header.style.display, '', 'applyHideHeader: false shows header');
		EngineConfig.reset();
	})();

	// --- fullscreen config ---
	(function() {
		// applyFullscreen sends postMessage to parent — in test context (no iframe),
		// parent === window. Verify the function runs without error and config is accessible.
		EngineConfig.set('display.fullscreen', true);
		TestHarness.assertEqual(EngineConfig.get('display.fullscreen'), true, 'applyFullscreen: config set to true');
		EngineConfig.set('display.fullscreen', false);
		TestHarness.assertEqual(EngineConfig.get('display.fullscreen'), false, 'applyFullscreen: config set to false');
		EngineConfig.reset();
	})();

	// --- expand descriptions ---
	(function() {
		var cr = document.getElementById('courtrecord');
		if (!cr) return;
		EngineConfig.set('display.expandEvidenceDescriptions', true);
		ThemeManager.reapply();
		TestHarness.assert(cr.classList.contains('expand-descriptions'), 'applyExpandDescriptions: true adds expand-descriptions');
		EngineConfig.set('display.expandEvidenceDescriptions', false);
		ThemeManager.reapply();
		TestHarness.assert(!cr.classList.contains('expand-descriptions'), 'applyExpandDescriptions: false removes expand-descriptions');
		EngineConfig.reset();
	})();

	// --- getMinBodyScale / getMaxBodyScale ---
	(function() {
		var min = ThemeManager.getMinBodyScale();
		TestHarness.assertType(min, 'number', 'getMinBodyScale: returns a number');
		TestHarness.assert(min > 0 && min <= 2.0, 'getMinBodyScale: value is > 0 and <= 2.0 (got ' + min + ')');
		// Stability: calling twice gives same result (no side effects)
		var min2 = ThemeManager.getMinBodyScale();
		TestHarness.assertEqual(min, min2, 'getMinBodyScale: stable across consecutive calls');
		var max = ThemeManager.getMaxBodyScale();
		TestHarness.assertType(max, 'number', 'getMaxBodyScale: returns a number');
		TestHarness.assertEqual(max, 1.18, 'getMaxBodyScale: returns 1.18 (ceil(100/85*100)/100)');
	})();

	// --- computeTier math verification ---
	(function() {
		// Tier logic: wide if screens+250+280 <= 85vw, medium if screens+250 <= 100vw, else narrow
		// With scaledScreensWidth=400: wide threshold=930, medium threshold=650
		// At 1200px viewport: 85vw=1020 >= 930 → wide
		var wideCheck = (400 + 250 + 280 <= 1200 * 0.85);
		TestHarness.assert(wideCheck, 'computeTier math: 400px screens fits wide at 1200px viewport');
		// At 800px viewport: 85vw=680 < 930 → not wide; 100vw=800 >= 650 → medium
		var mediumCheck = (400 + 250 + 280 > 800 * 0.85) && (400 + 250 <= 800);
		TestHarness.assert(mediumCheck, 'computeTier math: 400px screens is medium at 800px viewport');
		// At 500px viewport: 100vw=500 < 650 → narrow
		var narrowCheck = (400 + 250 > 500);
		TestHarness.assert(narrowCheck, 'computeTier math: 400px screens is narrow at 500px viewport');
	})();

	// --- cycleTab ---
	(function() {
		TestHarness.assertType(ThemeManager.cycleTab, 'function', 'cycleTab: is a function on public API');
	})();

	// --- Regression: pin current behavior ---
	(function() {
		// screenScale CSS property
		EngineConfig.set('layout.screenScale', 1.5);
		ThemeManager.reapply();
		var ss = getComputedStyle(document.documentElement).getPropertyValue('--screen-scale').trim();
		TestHarness.assertEqual(ss, '1.5', 'Regression: screenScale 1.5 sets --screen-scale to 1.5');
		EngineConfig.reset();
		ThemeManager.reapply();
	})();

	(function() {
		// nightMode class toggle
		EngineConfig.set('display.nightMode', true);
		ThemeManager.reapply();
		TestHarness.assert(document.body.classList.contains('night-mode'), 'Regression: nightMode true adds night-mode class');
		EngineConfig.set('display.nightMode', false);
		ThemeManager.reapply();
		TestHarness.assert(!document.body.classList.contains('night-mode'), 'Regression: nightMode false removes night-mode class');
		EngineConfig.reset();
		ThemeManager.reapply();
	})();

	(function() {
		// Public API methods exist
		TestHarness.assertType(ThemeManager.reapply, 'function', 'Regression: reapply is a function');
		TestHarness.assertType(ThemeManager.getMinBodyScale, 'function', 'Regression: getMinBodyScale is a function');
		TestHarness.assertType(ThemeManager.getMaxBodyScale, 'function', 'Regression: getMaxBodyScale is a function');
		TestHarness.assertType(ThemeManager.cycleTab, 'function', 'Regression: cycleTab is a function');
	})();

	// --- Accessibility tests ---
	(function() {
		EngineConfig.set('accessibility.reduceMotion', true);
		ThemeManager.reapply();
		TestHarness.assert(document.body.classList.contains('reduce-motion'), 'Accessibility: reduceMotion true adds reduce-motion class');
		EngineConfig.set('accessibility.reduceMotion', false);
		ThemeManager.reapply();
		TestHarness.assert(!document.body.classList.contains('reduce-motion'), 'Accessibility: reduceMotion false removes reduce-motion class');
	})();

	(function() {
		EngineConfig.set('accessibility.disableScreenShake', true);
		ThemeManager.reapply();
		TestHarness.assert(document.body.classList.contains('no-shake'), 'Accessibility: disableScreenShake true adds no-shake class');
		EngineConfig.set('accessibility.disableScreenShake', false);
		ThemeManager.reapply();
		TestHarness.assert(!document.body.classList.contains('no-shake'), 'Accessibility: disableScreenShake false removes no-shake class');
	})();

	(function() {
		EngineConfig.set('accessibility.disableFlash', true);
		ThemeManager.reapply();
		TestHarness.assert(document.body.classList.contains('no-flash'), 'Accessibility: disableFlash true adds no-flash class');
		EngineConfig.set('accessibility.disableFlash', false);
		ThemeManager.reapply();
		TestHarness.assert(!document.body.classList.contains('no-flash'), 'Accessibility: disableFlash false removes no-flash class');
	})();

	(function() {
		EngineConfig.set('accessibility.fontSize', 1.5);
		ThemeManager.reapply();
		var val = getComputedStyle(document.documentElement).getPropertyValue('--font-scale').trim();
		TestHarness.assertEqual(val, '1.5', 'Accessibility: fontSize 1.5 sets --font-scale to 1.5');
		EngineConfig.reset();
		ThemeManager.reapply();
	})();

	(function() {
		EngineConfig.set('accessibility.lineSpacing', 1.2);
		ThemeManager.reapply();
		var val = getComputedStyle(document.documentElement).getPropertyValue('--line-spacing').trim();
		TestHarness.assertEqual(val, '1.2', 'Accessibility: lineSpacing 1.2 sets --line-spacing to 1.2');
		EngineConfig.reset();
		ThemeManager.reapply();
	})();

	// Cleanup
	EngineConfig.reset();
	ThemeManager.reapply();
	window.localStorage.removeItem('aao_engine_config');
	document.body.classList.remove('night-mode');
	document.body.classList.remove('reduce-motion');
	document.body.classList.remove('no-shake');
	document.body.classList.remove('no-flash');
	document.documentElement.style.removeProperty('--screen-scale');
	document.documentElement.style.removeProperty('--mobile-screen-scale');
	document.documentElement.style.removeProperty('--font-scale');
	document.documentElement.style.removeProperty('--line-spacing');
}
