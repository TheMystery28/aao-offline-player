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
	dependencies : ['engine_config', 'engine_events', 'page_loaded'],
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

		if (currentLayoutTier === 'narrow') {
			// In narrow mode, fit screens to section width (not height)
			singleScreenWidth = sectionWidth;
			singleScreenHeight = singleScreenWidth * (192 / 256);
		} else {
			// In wide/medium mode, fit total #screens height to section height.
			// Zoom applies to everything inside #screens (meta + gaps + both screens),
			// so divide sectionHeight by the total pre-zoom height to get the scale.
			let totalPreZoomH = metaHeight + (2 * gapPx) + (2 * 192);
			// Account for examination mode's extra bottom bar (32px padding on #screens)
			var screenBottom = document.getElementById('screen-bottom');
			if (screenBottom && screenBottom.classList.contains('examination')) {
				totalPreZoomH += 32;
			}
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
		computeAutoFitScreenSize();
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

	function updateLayoutMode() {
		var section = document.querySelector('#content > section');
		var screens = document.getElementById('screens');
		var courtrecord = document.getElementById('courtrecord');
		var settings = document.getElementById('player-parametres');
		if (!section || !screens) return;

		var content = section.parentElement;

		// Calculate what the zoomed screens width WOULD be in wide mode (height-based).
		// Use viewport height minus header for a stable reference that doesn't
		// change when settings panel content is shown/hidden.
		var header = document.querySelector('header.compact');
		var headerH = (header && header.style.display !== 'none') ? header.offsetHeight : 0;
		var boundedHeight = window.innerHeight - headerH;
		var metaH = 18; // --meta-height
		var gapPx = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.7;
		var totalPreZoomH = metaH + (2 * gapPx) + (2 * 192);
		var fitScale = boundedHeight / totalPreZoomH;
		var wideScreenWidth = 256 * fitScale;
		var userScale = EngineConfig.get('layout.screenScale') || 1;
		var scaledScreensWidth = wideScreenWidth * userScale;

		// Flex item minimum widths (from CSS)
		var crMinWidth = 250;
		var settingsW = 280;

		// Predict container widths to prevent oscillation.
		// The tier decision changes --body-max-width (85vw for wide, 100vw for non-wide),
		// which changes section.clientWidth, which would re-trigger tier detection.
		// Instead, use viewport-based projections that are stable regardless of current tier.
		var viewportWidth = document.documentElement.clientWidth;
		var userBodyScale = EngineConfig.get('layout.bodyWidth') || 1;
		var wideVw = Math.round(85 * userBodyScale);
		if (wideVw > 100) wideVw = 100;
		var expectedWideWidth = viewportWidth * (wideVw / 100);
		var expectedMediumWidth = viewportWidth; // non-wide forces 100vw

		var newTier;
		if (scaledScreensWidth + crMinWidth + settingsW <= expectedWideWidth) {
			newTier = 'wide';
		} else if (scaledScreensWidth + crMinWidth <= expectedMediumWidth) {
			newTier = 'medium';
		} else {
			newTier = 'narrow';
		}

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

		// In non-layout modes (tabs/medium/narrow/stack), override to defaults and restore on return
		var isLayoutFree = !tabsActive && newTier === 'wide';
		if (isLayoutFree) {
			// Entering pure wide: restore saved values
			if (flexOverridden) {
				flexOverridden = false;
				var root = document.documentElement;
				root.style.setProperty('--evidence-flex', String(EVIDENCE_BASE_FLEX * ((savedEvidenceScale !== null) ? savedEvidenceScale : 1)));
				root.style.setProperty('--settings-flex', String(SETTINGS_BASE_FLEX * ((savedSettingsScale !== null) ? savedSettingsScale : 1)));
				if (savedScreenScale !== null) {
					root.style.setProperty('--screen-scale', String(savedScreenScale));
					computeAutoFitScreenSize();
				}
				// Restore user's body width setting
				applyBodyWidth();
				savedEvidenceScale = null;
				savedSettingsScale = null;
				savedScreenScale = null;
			}
		} else {
			// Entering tabs/medium/narrow/stack: save current and set to defaults
			if (!flexOverridden) {
				savedEvidenceScale = EngineConfig.get('layout.evidenceWidth') || 1;
				savedSettingsScale = EngineConfig.get('layout.settingsWidth') || 1;
				savedScreenScale = EngineConfig.get('layout.screenScale') || 1;
				flexOverridden = true;
				var root = document.documentElement;
				root.style.setProperty('--evidence-flex', String(EVIDENCE_BASE_FLEX));
				root.style.setProperty('--settings-flex', String(SETTINGS_BASE_FLEX));
				root.style.setProperty('--screen-scale', '1');
				// Force full width in non-wide modes (especially mobile)
				root.style.setProperty('--body-max-width', '100vw');
				computeAutoFitScreenSize();
			}
		}

		// Notify settings panel — hide layout when tabs active or non-wide
		// Pass whether wide mode is possible (so narrowMode selector can show)
		var wideIsPossible = (newTier === 'wide');
		if (typeof SettingsPanel !== 'undefined' && SettingsPanel.updateLayoutTier) {
			SettingsPanel.updateLayoutTier(tabsActive ? 'tabs' : currentLayoutTier, wideIsPossible);
		}
	}

	function createTabbedZone(courtrecord, settings) {
		if (tabbedZoneBar) return; // already created

		tabbedZoneBar = document.createElement('div');
		tabbedZoneBar.className = 'tabbed-zone-bar';
		tabbedZoneBar.style.display = 'flex';

		const evidenceBtn = document.createElement('a');
		evidenceBtn.className = 'tab-button active';
		evidenceBtn.setAttribute('data-tab', 'evidence');
		evidenceBtn.textContent = 'Evidence';
		evidenceBtn.addEventListener('click', function() { switchTab('evidence', courtrecord, settings); });
		tabbedZoneBar.appendChild(evidenceBtn);

		const settingsBtn = document.createElement('a');
		settingsBtn.className = 'tab-button';
		settingsBtn.setAttribute('data-tab', 'settings');
		settingsBtn.textContent = 'Settings';
		settingsBtn.addEventListener('click', function() { switchTab('settings', courtrecord, settings); });
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

	function switchTab(tab, courtrecord, settings) {
		activeTab = tab;
		const crEvidence = document.getElementById('cr_evidence');
		const crProfiles = document.getElementById('cr_profiles');

		if (tab === 'evidence') {
			if (crEvidence) crEvidence.style.display = '';
			if (crProfiles) crProfiles.style.display = '';
			if (tabbedZoneContent) tabbedZoneContent.style.display = 'none';
		} else {
			if (crEvidence) crEvidence.style.display = 'none';
			if (crProfiles) crProfiles.style.display = 'none';
			if (tabbedZoneContent) tabbedZoneContent.style.display = '';
		}

		// Update active tab styling
		if (tabbedZoneBar) {
			const buttons = tabbedZoneBar.querySelectorAll('.tab-button');
			for (let i = 0; i < buttons.length; i++) {
				if (buttons[i].getAttribute('data-tab') === tab) {
					buttons[i].classList.add('active');
				} else {
					buttons[i].classList.remove('active');
				}
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

	function onConfigChanged(data) {
		if (!data.path) {
			// Full config reload (e.g. reset or loadCaseConfig)
			applyAll();
			return;
		}
		// Apply only relevant section
		if (data.path === 'layout.bodyWidth') {
			applyBodyWidth();
		} else if (data.path.indexOf('layout.screenScale') === 0 || data.path.indexOf('layout.mobileScreenScale') === 0) {
			applyScale();
		} else if (data.path === 'display.nightMode') {
			applyNightMode();
		} else if (data.path === 'display.pixelated') {
			applyPixelated();
		} else if (data.path === 'theme.customCSS') {
			applyCustomCSS();
		} else if (data.path === 'display.mute') {
			applyMute();
		} else if (data.path === 'display.instantText') {
			applyInstantText();
		} else if (data.path === 'display.expandEvidenceDescriptions') {
			applyExpandDescriptions();
		} else if (data.path === 'display.blipVolume') {
			applyBlipVolume();
		} else if (data.path === 'display.hideHeader') {
			applyHideHeader();
		} else if (data.path === 'display.fullscreen') {
			applyFullscreen();
		} else if (data.path === 'layout.panelArrangement') {
			applyPanelArrangement();
		} else if (data.path === 'layout.evidenceWidth' || data.path === 'layout.settingsWidth') {
			applyPanelWidths();
		} else if (data.path === 'layout.narrowMode') {
			userOverrodeNarrowMode = true;
			applyNarrowMode();
		}
	}

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
			EngineEvents.on('config:changed', onConfigChanged);

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
					computeAutoFitScreenSize();
				}).observe(section);
			}

			// Watch #screen-bottom for class changes (examination mode toggle)
			// to recalculate screen zoom when the extra bar appears/disappears
			var screenBottom = document.getElementById('screen-bottom');
			if (screenBottom) {
				new MutationObserver(function() {
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
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('theme_manager');
