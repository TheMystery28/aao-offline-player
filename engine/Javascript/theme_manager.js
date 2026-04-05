"use strict";
/*
Ace Attorney Online - Theme Manager

Config-driven CSS theming, display settings, and layout controls.
Reacts to EngineConfig changes and updates the DOM/CSS accordingly.
ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'theme_manager',
	dependencies : ['engine_config', 'engine_events', 'input_registry', 'page_loaded'],
	init : function()
	{
		ThemeManager._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var ThemeManager = (function() {
	let customStyleEl = null;
	let currentLayoutTier = 'wide';
	let tabbedZoneBar = null;
	let tabbedZoneContent = null;
	let userOverrodeNarrowMode = false;
	let activeTab = 'evidence';
	let lastNonSettingsTab = 'evidence';
	let lastTabPressTime = 0;

	// ============================================================
	// SECTION: Constants
	// ============================================================

	// Valid arrangement values and their types
	var ARRANGEMENT_CLASSES = [
		'1-2-3', '1-3-2', '2-1-3', '2-3-1', '3-1-2', '3-2-1',
		'12-3', '21-3', '13-2', '31-2',
		'1-23', '1-32'
	];

	function getArrangementType(value) {
		// Row: X-Y-Z (3 single digits separated by dashes)
		if (value && value.length === 5 && value.charAt(1) === '-' && value.charAt(3) === '-') return 'row';
		// Screen-top: X-YZ (digit-dash-two digits)
		if (value && value.length === 4 && value.charAt(1) === '-') return 'top';
		// Mixed: XY-Z (two digits-dash-digit)
		if (value && value.length === 4 && value.charAt(2) === '-') return 'mixed';
		return 'row'; // fallback
	}

	// ============================================================
	// SECTION: Apply Functions (config → DOM/CSS)
	// ============================================================

	function applyAll() {
		applyBodyWidth();
		applyScale();
		applyNightMode();
		applyPixelated();
		applyCustomCSS();
		applyMute();
		applyInstantText();
		applyExpandDescriptions();
		applyBlipVolume();
		applyHideHeader();
		applyFullscreen();
		applyPanelWidths();
		applyPanelArrangement();
		applyNarrowMode();
		applyAccessibility();
	}

	/**
	 * Compute the scaled screens width based on viewport height and screen scale.
	 * Shared by updateLayoutMode() and getMinBodyScale().
	 */
	function computeScaledScreensWidth() {
		var header = document.querySelector('header.compact');
		var headerH = (header && header.style.display !== 'none') ? header.offsetHeight : 0;
		var boundedHeight = window.innerHeight - headerH;
		var metaH = 18;
		var gapPx = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.7;
		var totalPreZoomH = metaH + (2 * gapPx) + (2 * 192);
		var fitScale = boundedHeight / totalPreZoomH;
		var screenScale = EngineConfig.get('layout.screenScale') || 1;
		return 256 * fitScale * screenScale;
	}

	// Base flex values: slider 1.0 = these values (original panel proportions)
	var EVIDENCE_BASE_FLEX = 0.7;
	var SETTINGS_BASE_FLEX = 0.4;
	// Saved values when overridden in non-wide/tabs modes
	var savedEvidenceScale = null;
	var savedSettingsScale = null;
	var savedScreenScale = null;
	var flexOverridden = false;

	function applyPanelWidths() {
		var evidenceScale = EngineConfig.get('layout.evidenceWidth') || 1;
		var settingsScale = EngineConfig.get('layout.settingsWidth') || 1;
		var rawE = EVIDENCE_BASE_FLEX * evidenceScale;
		var rawS = SETTINGS_BASE_FLEX * settingsScale;
		// Normalize so sum >= 1.0. When flex-grow values sum below 1,
		// CSS only distributes that fraction of free space, leaving a gap.
		var sum = rawE + rawS;
		if (sum < 1) {
			var scale = 1 / sum;
			rawE *= scale;
			rawS *= scale;
		}
		var root = document.documentElement;
		root.style.setProperty('--evidence-flex', String(rawE));
		root.style.setProperty('--settings-flex', String(rawS));
	}

	function computeAutoFitScreenSize() {
		const section = document.querySelector('#content > section');
		if (!section) return;

		// On mobile portrait, skip auto-fit (CSS zoom handles it).
		// Desktop portrait windows should still use auto-fit.
		var isMobilePortrait = window.matchMedia &&
			window.matchMedia('(orientation: portrait) and (hover: none)').matches;
		if (isMobilePortrait) return;

		const sectionHeight = section.clientHeight;
		const sectionWidth = section.clientWidth;
		if (sectionHeight <= 0 || sectionWidth <= 0) return;

		const metaHeight = 18; // --meta-height
		const gapPx = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.7; // 0.7em in px

		let singleScreenWidth, singleScreenHeight;

		// Base screens height: meta + 2 gaps + 2 screens (+ 32px if examination mode).
		// Hoisted before tier check so margin compensation works for ALL tiers.
		let totalPreZoomH = metaHeight + (2 * gapPx) + (2 * 192);
		var screenBottom = document.getElementById('screen-bottom');
		if (screenBottom && screenBottom.classList.contains('examination')) {
			totalPreZoomH += 32;
		}

		if (currentLayoutTier === 'narrow') {
			// In narrow mode, fit screens to section width (not height)
			singleScreenWidth = sectionWidth;
			singleScreenHeight = singleScreenWidth * (192 / 256);
		} else {
			// In wide/medium mode, fit total #screens height to section height.
			// Transform applies to everything inside #screens (meta + gaps + both screens),
			// so divide sectionHeight by the total pre-scale height to get the scale.
			let fitScale = sectionHeight / totalPreZoomH;
			singleScreenWidth = 256 * fitScale;
			singleScreenHeight = 192 * fitScale;

			// Cap to section width so screens don't overflow horizontally
			if (singleScreenWidth > sectionWidth) {
				singleScreenWidth = sectionWidth;
				singleScreenHeight = singleScreenWidth * (192 / 256);
			}
		}

		// Apply --screen-scale as a user multiplier
		const scale = EngineConfig.get('layout.screenScale') || 1;
		singleScreenHeight *= scale;
		singleScreenWidth *= scale;

		// Re-cap after user scale in case it pushed width over
		if (singleScreenWidth > sectionWidth) {
			const ratio = sectionWidth / singleScreenWidth;
			singleScreenWidth *= ratio;
			singleScreenHeight *= ratio;
		}

		// Compute content scale factor (how much to zoom 256x192 content)
		const contentScale = singleScreenWidth / 256;

		const root = document.documentElement;
		root.style.setProperty('--screen-auto-width', singleScreenWidth + 'px');
		root.style.setProperty('--screen-auto-height', singleScreenHeight + 'px');
		root.style.setProperty('--screen-content-scale', String(contentScale));

		// Margin compensation for transform: scale() (replaces zoom flow behavior).
		// margin = base_dimension × (scale - 1)
		var marginRight = 256 * (contentScale - 1);
		var marginBottom = totalPreZoomH * (contentScale - 1);
		root.style.setProperty('--screen-margin-right', marginRight + 'px');
		root.style.setProperty('--screen-margin-bottom', marginBottom + 'px');
	}

	function applyBodyWidth() {
		var scale = EngineConfig.get('layout.bodyWidth') || 1;
		// Default 1.0 = 85vw. Slider adjusts the viewport percentage.
		var vw = Math.round(85 * scale);
		if (vw > 100) vw = 100;
		document.documentElement.style.setProperty('--body-max-width', vw + 'vw');
	}

	function applyScale() {
		const screenScale = EngineConfig.get('layout.screenScale');
		const mobileScale = EngineConfig.get('layout.mobileScreenScale');
		if (screenScale !== undefined) {
			document.documentElement.style.setProperty('--screen-scale', String(screenScale));
		}
		if (mobileScale !== undefined) {
			document.documentElement.style.setProperty('--mobile-screen-scale', String(mobileScale));
		}
		capMobileScale();
		computeAutoFitScreenSize();
	}

	/** Cap --mobile-screen-scale so the scaled #screens (incl. box-shadow)
	 *  never exceeds the viewport width. Sets --mobile-screen-scale-capped
	 *  and --screen-margin-bottom-mobile accordingly. */
	function capMobileScale() {
		var mobileScale = parseFloat(
			getComputedStyle(document.documentElement).getPropertyValue('--mobile-screen-scale')
		) || 1.4;
		var screenWidth = parseFloat(
			getComputedStyle(document.documentElement).getPropertyValue('--screen-width')
		) || 256;
		// 6px accounts for 3px box-shadow on each side of the screen children
		var maxScale = window.innerWidth / (screenWidth + 6);
		var capped = Math.min(mobileScale, maxScale);
		document.documentElement.style.setProperty('--mobile-screen-scale-capped', String(capped));

		// Compute mobile margin-bottom using the capped scale
		var metaHeight = 18;
		var gapPx = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.7;
		var totalH = metaHeight + (2 * gapPx) + (2 * 192);
		var screenBottom = document.getElementById('screen-bottom');
		if (screenBottom && screenBottom.classList.contains('examination')) {
			totalH += 32;
		}
		document.documentElement.style.setProperty(
			'--screen-margin-bottom-mobile',
			(totalH * (capped - 1)) + 'px'
		);
	}

	function applyNightMode() {
		const enabled = EngineConfig.get('display.nightMode');
		if (enabled) {
			document.body.classList.add('night-mode');
		} else {
			document.body.classList.remove('night-mode');
		}
	}

	function applyPixelated() {
		const enabled = EngineConfig.get('display.pixelated');
		const screens = document.getElementById('screens');
		if (screens) {
			if (enabled) {
				screens.classList.add('pixelated');
			} else {
				screens.classList.remove('pixelated');
			}
		}
	}

	function applyCustomCSS() {
		const css = EngineConfig.get('theme.customCSS');
		if (css) {
			if (!customStyleEl) {
				customStyleEl = document.createElement('style');
				customStyleEl.id = 'aao-custom-theme';
				document.head.appendChild(customStyleEl);
			}
			customStyleEl.textContent = css;
		} else if (customStyleEl) {
			customStyleEl.textContent = '';
		}
	}

	function applyMute() {
		const muted = EngineConfig.get('display.mute');
		if (typeof Howler !== 'undefined') {
			Howler.mute(!!muted);
		}
	}

	function applyInstantText() {
		const enabled = EngineConfig.get('display.instantText');
		if (typeof top_screen !== 'undefined' && top_screen && top_screen.setInstantMode) {
			top_screen.setInstantMode(!!enabled);
		}
	}

	function applyExpandDescriptions() {
		const enabled = EngineConfig.get('display.expandEvidenceDescriptions');
		const cr = document.getElementById('courtrecord');
		if (cr) {
			if (enabled) {
				cr.classList.add('expand-descriptions');
			} else {
				cr.classList.remove('expand-descriptions');
			}
		}
	}

	function applyBlipVolume() {
		if (typeof SoundHowler === 'undefined') return;
		const volume = EngineConfig.get('display.blipVolume');
		if (volume === undefined) return;
		for (let i = 1; i <= 3; i++) {
			try {
				SoundHowler.setSoundVolume('voice_-' + i, volume);
			} catch (e) {
				// Voice may not be registered if case doesn't use it
			}
		}
	}

	function applyHideHeader() {
		var hidden = EngineConfig.get('display.hideHeader');
		var header = document.querySelector('header.compact');
		if (!header) return;
		if (hidden) {
			header.style.display = 'none';
		} else {
			header.style.display = '';
		}
		// Notify parent frame to update toolbar styling
		try {
			if (window.parent && window.parent !== window) {
				window.parent.postMessage({
					type: 'aao-header-visibility',
					hidden: !!hidden,
					title: document.getElementById('title') ? document.getElementById('title').textContent : '',
					author: document.getElementById('author') ? document.getElementById('author').textContent.replace(/^--\s*/, '') : ''
				}, '*');
			}
		} catch (e) { /* cross-origin restriction */ }
	}

	function applyFullscreen() {
		var enabled = EngineConfig.get('display.fullscreen');
		// Notify parent frame (launcher) to toggle Tauri window fullscreen
		try {
			if (window.parent && window.parent !== window) {
				window.parent.postMessage({
					type: 'aao-fullscreen',
					fullscreen: !!enabled
				}, '*');
			}
		} catch (e) { /* cross-origin restriction */ }
	}

	function applyNarrowMode() {
		currentLayoutTier = ''; // force re-evaluation
		updateLayoutMode();
	}

	function toggleBodyClass(configPath, className) {
		if (EngineConfig.get(configPath)) {
			document.body.classList.add(className);
		} else {
			document.body.classList.remove(className);
		}
	}

	function applyAccessibility() {
		toggleBodyClass('accessibility.reduceMotion', 'reduce-motion');
		toggleBodyClass('accessibility.disableScreenShake', 'no-shake');
		toggleBodyClass('accessibility.disableFlash', 'no-flash');
		// fontSize and lineSpacing use CSS custom properties (different pattern)
		var root = document.documentElement;
		var fontSize = EngineConfig.get('accessibility.fontSize');
		if (fontSize !== undefined && fontSize !== 1) {
			root.style.setProperty('--font-scale', String(fontSize));
		} else {
			root.style.removeProperty('--font-scale');
		}
		var lineSpacing = EngineConfig.get('accessibility.lineSpacing');
		if (lineSpacing !== undefined && lineSpacing !== 1) {
			root.style.setProperty('--line-spacing', String(lineSpacing));
		} else {
			root.style.removeProperty('--line-spacing');
		}
	}

	// ============================================================
	// SECTION: Layout Tier Detection & Flex Management
	// ============================================================

	/**
	 * Compute layout tier based on scaled screens width and viewport.
	 * Uses viewport-based projections to prevent oscillation.
	 * @param {number} scaledScreensWidth - Pre-computed scaled screens width
	 * @returns {'wide'|'medium'|'narrow'}
	 */
	function computeTier(scaledScreensWidth) {
		var viewportWidth = document.documentElement.clientWidth;
		var userBodyScale = EngineConfig.get('layout.bodyWidth') || 1;
		var wideVw = Math.round(85 * userBodyScale);
		if (wideVw > 100) wideVw = 100;
		var expectedWideWidth = viewportWidth * (wideVw / 100);
		var expectedMediumWidth = viewportWidth; // non-wide forces 100vw

		if (scaledScreensWidth + 250 + 280 <= expectedWideWidth) return 'wide';
		if (scaledScreensWidth + 250 <= expectedMediumWidth) return 'medium';
		return 'narrow';
	}

	/** Save current flex/scale values and set to defaults (entering non-wide mode). */
	function saveFlex() {
		if (flexOverridden) return;
		savedEvidenceScale = EngineConfig.get('layout.evidenceWidth') || 1;
		savedSettingsScale = EngineConfig.get('layout.settingsWidth') || 1;
		savedScreenScale = EngineConfig.get('layout.screenScale') || 1;
		flexOverridden = true;
		var root = document.documentElement;
		root.style.setProperty('--evidence-flex', String(EVIDENCE_BASE_FLEX));
		root.style.setProperty('--settings-flex', String(SETTINGS_BASE_FLEX));
		root.style.setProperty('--screen-scale', '1');
		root.style.setProperty('--body-max-width', '100vw');
		computeAutoFitScreenSize();
	}

	/** Restore saved flex/scale values (returning to wide mode). */
	function restoreFlex() {
		if (!flexOverridden) return;
		flexOverridden = false;
		var root = document.documentElement;
		root.style.setProperty('--evidence-flex', String(EVIDENCE_BASE_FLEX * ((savedEvidenceScale !== null) ? savedEvidenceScale : 1)));
		root.style.setProperty('--settings-flex', String(SETTINGS_BASE_FLEX * ((savedSettingsScale !== null) ? savedSettingsScale : 1)));
		if (savedScreenScale !== null) {
			root.style.setProperty('--screen-scale', String(savedScreenScale));
			computeAutoFitScreenSize();
		}
		applyBodyWidth();
		savedEvidenceScale = null;
		savedSettingsScale = null;
		savedScreenScale = null;
	}

	function updateLayoutMode() {
		var section = document.querySelector('#content > section');
		var screens = document.getElementById('screens');
		var courtrecord = document.getElementById('courtrecord');
		var settings = document.getElementById('player-parametres');
		if (!section || !screens) return;

		var content = section.parentElement;

		var newTier = computeTier(computeScaledScreensWidth());

		if (newTier === currentLayoutTier) return;
		currentLayoutTier = newTier;

		var narrowMode;
		if (userOverrodeNarrowMode) {
			narrowMode = EngineConfig.get('layout.narrowMode') || 'tabs';
		} else {
			narrowMode = (newTier === 'medium') ? 'tabs' : 'stack';
		}

		if (newTier === 'wide') {
			// Check for tab override: user explicitly chose tabs in wide mode
			if (userOverrodeNarrowMode && narrowMode === 'tabs') {
				removeTabbedZone(section, courtrecord, settings);
				if (settings) { settings.style.display = 'none'; }
				createTabbedZone(courtrecord, settings);
				if (courtrecord) { courtrecord.style.flexGrow = '1'; }
			} else {
				removeTabbedZone(section, courtrecord, settings);
				if (settings) { settings.style.display = ''; }
				if (courtrecord) { courtrecord.style.flexGrow = ''; }
			}
			section.classList.remove('layout-stack');
			if (content) content.classList.remove('layout-stack');
		} else if (newTier === 'medium' && narrowMode === 'tabs') {
			removeTabbedZone(section, courtrecord, settings);
			if (settings) { settings.style.display = 'none'; }
			createTabbedZone(courtrecord, settings);
			// In tabs mode, courtrecord is the only growing panel in the row.
			// Force flex-grow:1 so it fills all space regardless of evidence width slider.
			if (courtrecord) { courtrecord.style.flexGrow = '1'; }
			section.classList.remove('layout-stack');
			if (content) content.classList.remove('layout-stack');
		} else {
			// medium+stack or narrow
			removeTabbedZone(section, courtrecord, settings);
			if (settings) { settings.style.display = ''; }
			if (courtrecord) { courtrecord.style.flexGrow = ''; }
			section.classList.add('layout-stack');
			if (content) content.classList.add('layout-stack');
		}

		// Determine if tabs are active (medium/tabs or wide+tabs override)
		var tabsActive = (newTier === 'medium' && narrowMode === 'tabs') ||
			(newTier === 'wide' && userOverrodeNarrowMode && narrowMode === 'tabs');

		// In non-layout modes, override flex to defaults and restore on return
		var isLayoutFree = !tabsActive && newTier === 'wide';
		if (isLayoutFree) { restoreFlex(); } else { saveFlex(); }

		// Notify settings panel — hide layout when tabs active or non-wide
		// Pass whether wide mode is possible (so narrowMode selector can show)
		var wideIsPossible = (newTier === 'wide');
		if (typeof SettingsPanel !== 'undefined' && SettingsPanel.updateLayoutTier) {
			SettingsPanel.updateLayoutTier(tabsActive ? 'tabs' : currentLayoutTier, wideIsPossible);
		}
	}

	// ============================================================
	// SECTION: Tabbed Zone Management
	// ============================================================

	function createTabbedZone(courtrecord, settings) {
		if (tabbedZoneBar) return; // already created

		tabbedZoneBar = document.createElement('div');
		tabbedZoneBar.className = 'tabbed-zone-bar';
		tabbedZoneBar.style.display = 'flex';

		const evidenceBtn = document.createElement('a');
		evidenceBtn.className = 'tab-button active';
		evidenceBtn.setAttribute('data-tab', 'evidence');
		evidenceBtn.textContent = 'Evidence';
		evidenceBtn.addEventListener('click', function() { switchTab('evidence'); });
		tabbedZoneBar.appendChild(evidenceBtn);

		const settingsBtn = document.createElement('a');
		settingsBtn.className = 'tab-button';
		settingsBtn.setAttribute('data-tab', 'settings');
		settingsBtn.textContent = 'Settings';
		settingsBtn.addEventListener('click', function() { switchTab('settings'); });
		tabbedZoneBar.appendChild(settingsBtn);

		// Insert tab bar as first child of courtrecord
		courtrecord.insertBefore(tabbedZoneBar, courtrecord.firstChild);

		// Create settings content container
		tabbedZoneContent = document.createElement('div');
		tabbedZoneContent.className = 'tabbed-zone-content';
		tabbedZoneContent.style.display = 'none';
		courtrecord.appendChild(tabbedZoneContent);

		// Move settings into the content area
		if (settings) {
			tabbedZoneContent.appendChild(settings);
			settings.style.display = '';
			settings.style.width = '100%';
			settings.style.border = 'none';
		}

		activeTab = 'evidence';
	}

	function removeTabbedZone(section, courtrecord, settings) {
		if (!tabbedZoneBar) return;

		// Always clean inline styles — even if settings was already moved out for measurement
		if (settings) {
			settings.style.width = '';
			settings.style.border = '';
		}

		// Move settings back to section only if still inside tabbed zone
		if (settings && tabbedZoneContent && tabbedZoneContent.contains(settings)) {
			section.appendChild(settings);
		}

		// Remove tab bar and content
		if (tabbedZoneBar && tabbedZoneBar.parentNode) {
			tabbedZoneBar.parentNode.removeChild(tabbedZoneBar);
		}
		if (tabbedZoneContent && tabbedZoneContent.parentNode) {
			tabbedZoneContent.parentNode.removeChild(tabbedZoneContent);
		}
		tabbedZoneBar = null;
		tabbedZoneContent = null;

		// Restore evidence sections visibility
		const crEvidence = document.getElementById('cr_evidence');
		const crProfiles = document.getElementById('cr_profiles');
		if (crEvidence) crEvidence.style.display = '';
		if (crProfiles) crProfiles.style.display = '';

		activeTab = 'evidence';
	}

	function switchTab(tab) {
		activeTab = tab;
		var cr = document.getElementById('courtrecord');
		var crEvidence = document.getElementById('cr_evidence');
		var crProfiles = document.getElementById('cr_profiles');

		if (tab === 'evidence') {
			// Show evidence section (CSS controls which is visible via .evidence class)
			if (cr) { cr.classList.remove('profiles'); cr.classList.add('evidence'); }
			if (crEvidence) crEvidence.style.display = '';
			if (crProfiles) crProfiles.style.display = '';
			if (tabbedZoneContent) tabbedZoneContent.style.display = 'none';
		} else if (tab === 'profiles') {
			// Show profiles section
			if (cr) { cr.classList.remove('evidence'); cr.classList.add('profiles'); }
			if (crEvidence) crEvidence.style.display = '';
			if (crProfiles) crProfiles.style.display = '';
			if (tabbedZoneContent) tabbedZoneContent.style.display = 'none';
		} else {
			// Settings tab — hide evidence/profiles, show settings content
			if (crEvidence) crEvidence.style.display = 'none';
			if (crProfiles) crProfiles.style.display = 'none';
			if (tabbedZoneContent) tabbedZoneContent.style.display = '';
		}

		// Update active tab styling — evidence and profiles both highlight the "Evidence" button
		if (tabbedZoneBar) {
			var buttons = tabbedZoneBar.querySelectorAll('.tab-button');
			for (var i = 0; i < buttons.length; i++) {
				var btnTab = buttons[i].getAttribute('data-tab');
				// "Evidence" button is active for both evidence and profiles tabs
				if ((tab === 'evidence' || tab === 'profiles') && btnTab === 'evidence') {
					buttons[i].classList.add('active');
				} else if (tab === 'settings' && btnTab === 'settings') {
					buttons[i].classList.add('active');
				} else {
					buttons[i].classList.remove('active');
				}
			}
		}
	}

	function cycleTab() {
		// Only works when tabbed zone is active
		if (!tabbedZoneBar) return;

		var now = Date.now();
		var isDoublePress = (now - lastTabPressTime) < 300;
		lastTabPressTime = now;

		if (isDoublePress) {
			// Double press: toggle to/from Settings
			if (activeTab === 'settings') {
				// Return to last non-settings tab
				switchTab(lastNonSettingsTab);
			} else {
				// Go to settings, remember where we were
				lastNonSettingsTab = activeTab;
				switchTab('settings');
			}
		} else {
			// Single press: cycle Evidence ↔ Profiles (or exit Settings)
			if (activeTab === 'settings') {
				switchTab(lastNonSettingsTab);
			} else if (activeTab === 'evidence') {
				switchTab('profiles');
			} else {
				switchTab('evidence');
			}
		}
	}

	function applyPanelArrangement() {
		var arrangement = EngineConfig.get('layout.panelArrangement') || '1-2-3';
		var section = document.querySelector('#content > section');
		var content = section ? section.parentElement : null;
		var courtrecord = document.getElementById('courtrecord');
		var settings = document.getElementById('player-parametres');
		if (!section) return;

		// Clean inline styles that may linger from previous arrangement/tabs
		if (courtrecord) {
			courtrecord.style.overflow = '';
			courtrecord.style.width = '';
			courtrecord.style.height = '';
			courtrecord.style.order = '';
			courtrecord.style.flex = '';
		}
		if (settings) {
			settings.style.overflow = '';
			settings.style.width = '';
			settings.style.height = '';
			settings.style.border = '';
			settings.style.display = '';
			settings.style.order = '';
			settings.style.flex = '';
		}

		// Remove all arrangement classes
		for (var i = 0; i < ARRANGEMENT_CLASSES.length; i++) {
			section.classList.remove('arrangement-' + ARRANGEMENT_CLASSES[i]);
		}

		// Apply the new arrangement class (default 1-2-3 uses no class)
		if (arrangement !== '1-2-3') {
			section.classList.add('arrangement-' + arrangement);
		}

		// Set scroll behavior based on arrangement type.
		// Non-row arrangements wrap panels, so section needs height:auto
		// to grow beyond the viewport, and content needs overflow:auto to scroll.
		var type = getArrangementType(arrangement);
		if (type === 'row') {
			section.style.height = '';
			section.style.overflow = '';
			if (content) content.style.overflow = '';
		} else {
			section.style.height = 'auto';
			section.style.overflow = 'visible';
			if (content) content.style.overflow = 'auto';
		}

		// Re-evaluate layout tier since panel composition in the row changed
		currentLayoutTier = ''; // force re-evaluation
		updateLayoutMode();
	}

	// ============================================================
	// SECTION: Config Change Router
	// ============================================================

	// Config path → handler lookup (direct matches)
	var CONFIG_HANDLERS = {
		'layout.bodyWidth': applyBodyWidth,
		'layout.screenScale': applyScale,
		'layout.mobileScreenScale': applyScale,
		'display.nightMode': applyNightMode,
		'display.pixelated': applyPixelated,
		'theme.customCSS': applyCustomCSS,
		'display.mute': applyMute,
		'display.instantText': applyInstantText,
		'display.expandEvidenceDescriptions': applyExpandDescriptions,
		'display.blipVolume': applyBlipVolume,
		'display.hideHeader': applyHideHeader,
		'display.fullscreen': applyFullscreen,
		'layout.panelArrangement': applyPanelArrangement,
		'layout.evidenceWidth': applyPanelWidths,
		'layout.settingsWidth': applyPanelWidths
	};

	function onConfigChanged(data) {
		if (!data.path) {
			applyAll();
			return;
		}
		var handler = CONFIG_HANDLERS[data.path];
		if (handler) {
			handler();
		} else if (data.path === 'layout.narrowMode') {
			userOverrodeNarrowMode = true;
			applyNarrowMode();
		} else if (data.path.indexOf('accessibility') === 0) {
			applyAccessibility();
		}
	}

	// ============================================================
	// SECTION: Public API
	// ============================================================

	return {
		_init: function() {
			// Detect if user has explicitly overridden narrowMode
			try {
				const stored = window.localStorage.getItem('aao_engine_config');
				if (stored) {
					const diff = JSON.parse(stored);
					userOverrodeNarrowMode = !!(diff.layout && diff.layout.narrowMode !== undefined);
				}
			} catch (e) { /* ignore parse errors */ }

			applyAll();
			EngineEvents.on('config:changed', onConfigChanged, 0, 'engine');

			// Listen for tab cycling input (no source filter — works for keyboard and gamepad)
			EngineEvents.on('input:action', function(data) {
				if (data.action === 'crSwitchTab') {
					cycleTab();
				}
			}, 0, 'engine');

			InputRegistry.register({ action: 'switchToSettings', label: 'switch to settings (double-press)', keyboard: 'Tab ×2', gamepad: 'Y ×2', source: 'engine', module: 'theme_manager' });

			// Listen for config sync messages from parent frame
			window.addEventListener('message', function(e) {
				if (e.data && e.data.type === 'aao-set-config' && e.data.path) {
					EngineConfig.set(e.data.path, e.data.value);
				}
			});

			// Observe section resizes to recompute layout tier and auto-fit screen size
			const section = document.querySelector('#content > section');
			if (section && typeof ResizeObserver !== 'undefined') {
				new ResizeObserver(function() {
					updateLayoutMode();
					capMobileScale();
					computeAutoFitScreenSize();
				}).observe(section);
			}

			// Watch #screen-bottom for class changes (examination mode toggle)
			// to recalculate screen zoom when the extra bar appears/disappears
			var screenBottom = document.getElementById('screen-bottom');
			if (screenBottom) {
				new MutationObserver(function() {
					capMobileScale();
					computeAutoFitScreenSize();
				}).observe(screenBottom, { attributes: true, attributeFilter: ['class'] });
			}

			// Run initial layout mode evaluation after DOM has rendered
			setTimeout(function() {
				updateLayoutMode();
			}, 100);
		},

		/**
		 * Force re-apply all theme settings from current config.
		 * Useful after EngineEvents.clear() destroys the config:changed listener.
		 */
		reapply: function() {
			applyAll();
		},

		/**
		 * Cycle between Evidence/Profiles/Settings tabs.
		 * Single press: Evidence ↔ Profiles. Double press (~300ms): toggle Settings.
		 * Does nothing in wide mode (no tabbed zone).
		 */
		cycleTab: cycleTab,

		/**
		 * Compute the minimum bodyWidth scale that keeps wide mode.
		 * Returns the scale value (e.g. 0.8) or 0 if wide is impossible at any scale.
		 */
		getMinBodyScale: function() {
			var scaledScreensWidth = computeScaledScreensWidth();
			var wideThreshold = scaledScreensWidth + 250 + 280;
			var viewportWidth = document.documentElement.clientWidth;
			if (viewportWidth <= 0) return 0;
			// wideThreshold <= viewportWidth * (85 * scale / 100)
			// scale >= wideThreshold / (viewportWidth * 0.85)
			var minScale = wideThreshold / (viewportWidth * 0.85);
			// Round up to nearest step (0.01)
			minScale = Math.ceil(minScale * 100) / 100;
			return minScale > 2.0 ? 0 : minScale;
		},

		/**
		 * Compute the maximum useful bodyWidth scale (where body reaches 100vw).
		 * Beyond this, the body is already full viewport width and more has no effect.
		 */
		getMaxBodyScale: function() {
			// body max-width = 85 * scale vw, capped at 100vw
			// 85 * scale = 100 → scale = 100/85 ≈ 1.176
			var maxScale = Math.ceil((100 / 85) * 100) / 100;
			return maxScale;
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('theme_manager');
