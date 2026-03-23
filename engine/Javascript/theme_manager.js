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
	}

	function computeAutoFitScreenSize() {
		const section = document.querySelector('#content > section');
		if (!section) return;

		// In portrait orientation, skip height-based auto-fit (zoom handles it)
		if (window.matchMedia && window.matchMedia('(orientation: portrait)').matches) return;

		const sectionHeight = section.clientHeight;
		if (sectionHeight <= 0) return;

		const metaHeight = 18; // --meta-height
		const gapPx = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.7; // 0.7em in px

		let usableHeight = sectionHeight - metaHeight - (2 * gapPx);
		let singleScreenHeight = usableHeight / 2;
		let singleScreenWidth = singleScreenHeight * (256 / 192); // maintain 4:3

		// Apply --screen-scale as a multiplier
		const scale = EngineConfig.get('layout.screenScale') || 1;
		singleScreenHeight *= scale;
		singleScreenWidth *= scale;

		// Compute content scale factor (how much to scale 256x192 internal content)
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
		}
	}

	return {
		_init: function() {
			applyAll();
			EngineEvents.on('config:changed', onConfigChanged);

			// Observe section resizes to recompute auto-fit screen size
			const section = document.querySelector('#content > section');
			if (section && typeof ResizeObserver !== 'undefined') {
				new ResizeObserver(function() {
					computeAutoFitScreenSize();
				}).observe(section);
			}
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
