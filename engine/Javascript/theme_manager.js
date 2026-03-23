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

	function applyAll() {
		applyScale();
		applyNightMode();
		applyPixelated();
		applyCustomCSS();
		applyMute();
		applyInstantText();
		applyExpandDescriptions();
		applyBlipVolume();
		applyCourtRecordPosition();
		applyNarrowMode();
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

	function applyNarrowMode() {
		currentLayoutTier = ''; // force re-evaluation
		updateLayoutMode();
	}

	function updateLayoutMode() {
		const section = document.querySelector('#content > section');
		const screens = document.getElementById('screens');
		const courtrecord = document.getElementById('courtrecord');
		const settings = document.getElementById('player-parametres');
		if (!section || !screens) return;

		const content = section.parentElement;
		const sectionWidth = section.clientWidth;

		// Calculate what the zoomed screens width WOULD be in wide mode (height-based).
		// Use bounded content height (stable regardless of layout-stack).
		var boundedHeight = content ? content.clientHeight : section.clientHeight;
		var metaH = 18; // --meta-height
		var gapPx = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.7;
		var usableH = boundedHeight - metaH - (2 * gapPx);
		var wideScreenHeight = Math.max(usableH / 2, 50);
		var wideScreenWidth = wideScreenHeight * (256 / 192);
		var userScale = EngineConfig.get('layout.screenScale') || 1;
		var scaledScreensWidth = wideScreenWidth * userScale;

		// Flex item minimum widths (from CSS)
		var crMinWidth = 250;   // #courtrecord min-width
		var settingsWidth = 280; // #player-parametres width

		// Determine tier: would all 3 fit? Would 2 fit?
		var newTier;
		if (scaledScreensWidth + crMinWidth + settingsWidth <= sectionWidth) {
			newTier = 'wide';
		} else if (scaledScreensWidth + crMinWidth <= sectionWidth) {
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
			removeTabbedZone(section, courtrecord, settings);
			if (settings) { settings.style.display = ''; }
			section.classList.remove('layout-stack');
			if (content) content.classList.remove('layout-stack');
		} else if (newTier === 'medium' && narrowMode === 'tabs') {
			removeTabbedZone(section, courtrecord, settings);
			if (settings) { settings.style.display = 'none'; }
			createTabbedZone(courtrecord, settings);
			section.classList.remove('layout-stack');
			if (content) content.classList.remove('layout-stack');
		} else {
			// medium+stack or narrow
			removeTabbedZone(section, courtrecord, settings);
			if (settings) { settings.style.display = ''; }
			section.classList.add('layout-stack');
			if (content) content.classList.add('layout-stack');
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

	function applyCourtRecordPosition() {
		const position = EngineConfig.get('layout.courtRecordPosition');
		const section = document.querySelector('#content > section');
		if (!section) return;
		section.classList.remove('cr-right', 'cr-left', 'cr-bottom', 'cr-hidden');
		if (position && position !== 'right') {
			section.classList.add('cr-' + position);
		}
	}

	function onConfigChanged(data) {
		if (!data.path) {
			// Full config reload (e.g. reset or loadCaseConfig)
			applyAll();
			return;
		}
		// Apply only relevant section
		if (data.path.indexOf('layout.screenScale') === 0 || data.path.indexOf('layout.mobileScreenScale') === 0) {
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
		} else if (data.path === 'layout.courtRecordPosition') {
			applyCourtRecordPosition();
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

			// Observe section resizes to recompute layout tier and auto-fit screen size
			const section = document.querySelector('#content > section');
			if (section && typeof ResizeObserver !== 'undefined') {
				new ResizeObserver(function() {
					updateLayoutMode();
					computeAutoFitScreenSize();
				}).observe(section);
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
