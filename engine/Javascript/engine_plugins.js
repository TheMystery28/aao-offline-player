"use strict";
/*
Ace Attorney Online - Plugin System

Two-phase plugin lifecycle:
1. Plugins call EnginePlugins.register() on load — stores the descriptor
2. After player:init fires, all registered plugins get init(config, events, api) called

Plugins use the 'plugins.' namespace for config and their plugin name for events.
ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'engine_plugins',
	dependencies : ['engine_events', 'engine_config'],
	init : function()
	{
		EnginePlugins._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var EnginePlugins = (function() {
	var registry = [];
	var isReady = false;
	var frozenApi = null;

	// ============================================================
	// SECTION: Plugin API Builder
	// ============================================================

	function buildApi() {
		return {
			// --- DOM manipulation ---
			dom: {
				query: function(sel) { return document.querySelector(sel); },
				queryAll: function(sel) { return document.querySelectorAll(sel); },
				create: function(tag) { return document.createElement(tag); },
				addClass: typeof addClass === 'function' ? addClass : function() {},
				removeClass: typeof removeClass === 'function' ? removeClass : function() {},
				hasClass: typeof hasClass === 'function' ? hasClass : function() { return false; },
				toggleClass: typeof toggleClass === 'function' ? toggleClass : function() {},
				setClass: typeof setClass === 'function' ? setClass : function() {},
				emptyNode: typeof emptyNode === 'function' ? emptyNode : function() {},
				setNodeTextContents: typeof setNodeTextContents === 'function' ? setNodeTextContents : function() {},
				injectCSS: function(cssText) {
					var style = document.createElement('style');
					style.textContent = cssText;
					document.head.appendChild(style);
					return style;
				},
				injectStylesheet: function(href) {
					var link = document.createElement('link');
					link.rel = 'stylesheet';
					link.href = href;
					document.head.appendChild(link);
					return link;
				}
			},

			// --- Player state & control ---
			player: {
				readFrame: function(idx) { if (typeof readFrame === 'function') readFrame(idx); },
				proceed: function() { if (typeof proceed === 'function') proceed('click'); },
				getCurrentFrameId: function() { return typeof player_status !== 'undefined' ? player_status.current_frame_id : 0; },
				getCurrentFrameIndex: function() { return typeof player_status !== 'undefined' ? player_status.current_frame_index : 0; },
				getNextFrameIndex: function() { return typeof player_status !== 'undefined' ? player_status.next_frame_index : 0; },
				getStatus: function() { return typeof player_status !== 'undefined' ? player_status : null; },
				getTrialData: function() { return typeof trial_data !== 'undefined' ? trial_data : null; },
				getTrialInfo: function() { return typeof trial_information !== 'undefined' ? trial_information : null; }
			},

			// --- Sound (transparent passthrough to SoundHowler) ---
			sound: {
				playMusic: typeof playMusic === 'function' ? playMusic : function() {},
				stopMusic: typeof stopMusic === 'function' ? stopMusic : function() {},
				playSound: typeof playSound === 'function' ? playSound : function() {},
				fadeMusic: typeof fadeMusic === 'function' ? fadeMusic : function() {},
				crossfadeMusic: typeof crossfadeMusic === 'function' ? crossfadeMusic : function() {},
				registerSound: function(id, options) {
					return typeof SoundHowler !== 'undefined' ? SoundHowler.registerSound(id, options) : null;
				},
				unloadSound: function(id) {
					if (typeof SoundHowler !== 'undefined') SoundHowler.unloadSound(id);
				},
				getSoundById: function(id) {
					return typeof SoundHowler !== 'undefined' ? SoundHowler.getSoundById(id) : null;
				},
				setSoundVolume: function(id, vol) {
					if (typeof SoundHowler !== 'undefined') SoundHowler.setSoundVolume(id, vol);
				},
				mute: function(muted) {
					if (typeof Howler !== 'undefined') Howler.mute(muted);
				},
				isMuted: function() {
					return typeof Howler !== 'undefined' ? Howler._muted : false;
				}
			},

			// --- Court record ---
			courtRecord: {
				setHidden: typeof setCrElementHidden === 'function' ? setCrElementHidden : function() {},
				refresh: typeof refreshCrElements === 'function' ? refreshCrElements : function() {},
				getElement: function(type, id) {
					return document.getElementById('cr_' + type + '_' + id);
				}
			},

			// --- Custom input actions ---
			input: {
				registerAction: function(actionName, handler) {
					EngineEvents.on('input:action', function(data) {
						if (data.action === actionName) handler(data);
					});
				},
				onKeyDown: function(handler) {
					document.addEventListener('keydown', handler);
				},
				onKeyUp: function(handler) {
					document.addEventListener('keyup', handler);
				}
			},

			// --- Plugin settings ---
			settings: {
				addSection: function(title, controls) {
					var container = document.getElementById('player-parametres');
					if (!container) return null;

					var detailsEl = document.createElement('details');
					detailsEl.setAttribute('data-plugin-section', title);
					var summaryEl = document.createElement('summary');
					summaryEl.textContent = title;
					detailsEl.appendChild(summaryEl);

					var contentDiv = document.createElement('div');
					contentDiv.className = 'settings-section-content';

					if (Array.isArray(controls)) {
						for (var i = 0; i < controls.length; i++) {
							var ctrl = controls[i];
							if (ctrl.type === 'checkbox') {
								var label = document.createElement('label');
								label.className = 'regular_label';
								var cb = document.createElement('input');
								cb.type = 'checkbox';
								cb.checked = !!EngineConfig.get(ctrl.key);
								cb.addEventListener('change', (function(key) {
									return function() { EngineConfig.set(key, this.checked); };
								})(ctrl.key));
								label.appendChild(cb);
								label.appendChild(document.createTextNode(' ' + ctrl.label));
								contentDiv.appendChild(label);
							} else if (ctrl.type === 'slider') {
								var sliderLabel = document.createElement('label');
								sliderLabel.className = 'regular_label';
								var span = document.createElement('span');
								span.textContent = ctrl.label;
								sliderLabel.appendChild(span);
								var slider = document.createElement('input');
								slider.type = 'range';
								slider.min = String(ctrl.min || 0);
								slider.max = String(ctrl.max || 100);
								slider.step = String(ctrl.step || 1);
								var val = EngineConfig.get(ctrl.key);
								slider.value = String(val !== undefined ? val : ctrl.min || 0);
								slider.addEventListener('input', (function(key) {
									return function() { EngineConfig.set(key, parseFloat(this.value)); };
								})(ctrl.key));
								sliderLabel.appendChild(slider);
								contentDiv.appendChild(sliderLabel);
							} else if (ctrl.type === 'select') {
								var selectLabel = document.createElement('label');
								selectLabel.className = 'regular_label';
								var selectSpan = document.createElement('span');
								selectSpan.textContent = ctrl.label;
								selectLabel.appendChild(selectSpan);
								var select = document.createElement('select');
								var opts = ctrl.options || [];
								for (var oi = 0; oi < opts.length; oi++) {
									var opt = document.createElement('option');
									if (typeof opts[oi] === 'object') {
										opt.value = opts[oi].value;
										opt.textContent = opts[oi].label;
									} else {
										opt.value = String(opts[oi]);
										opt.textContent = String(opts[oi]);
									}
									select.appendChild(opt);
								}
								select.value = String(EngineConfig.get(ctrl.key) || '');
								select.addEventListener('change', (function(key) {
									return function() { EngineConfig.set(key, this.value); };
								})(ctrl.key));
								selectLabel.appendChild(select);
								contentDiv.appendChild(selectLabel);
							}
						}
					}

					detailsEl.appendChild(contentDiv);
					container.appendChild(detailsEl);
					return contentDiv;
				},

				removeSection: function(title) {
					var container = document.getElementById('player-parametres');
					if (!container) return;
					var sections = container.querySelectorAll('details[data-plugin-section]');
					for (var i = 0; i < sections.length; i++) {
						if (sections[i].getAttribute('data-plugin-section') === title) {
							sections[i].parentNode.removeChild(sections[i]);
							break;
						}
					}
				}
			},

			// --- Display engine access ---
			display: {
				getTopScreen: function() { return typeof top_screen !== 'undefined' ? top_screen : null; },
				getBottomScreen: function() { return typeof bottom_screen !== 'undefined' ? bottom_screen : null; },
				getScreenDisplay: function() { return typeof top_screen !== 'undefined' ? top_screen : null; }
			}
		};
	}

	// ============================================================
	// SECTION: Plugin Initialization
	// ============================================================

	function initPlugin(descriptor) {
		if (!descriptor || typeof descriptor.init !== 'function') return;
		try {
			descriptor.init(EngineConfig, EngineEvents, frozenApi);
		} catch (e) {
			console.error('[EnginePlugins] Plugin "' + (descriptor.name || 'unknown') + '" crashed during init:', e);
		}
	}

	function initAllPending() {
		if (!frozenApi) {
			frozenApi = Object.freeze(buildApi());
		}
		for (var i = 0; i < registry.length; i++) {
			if (!registry[i]._initialized) {
				registry[i]._initialized = true;
				initPlugin(registry[i]);
			}
		}
	}

	// ============================================================
	// SECTION: Case Plugin Loading
	// ============================================================

	function loadCasePlugins() {
		if (typeof trial_information === 'undefined' || !trial_information) return;
		var caseBase = 'case/' + trial_information.id + '/';
		var manifestUrl = caseBase + 'plugins/manifest.json';

		try {
			var xhr = new XMLHttpRequest();
			xhr.open('GET', manifestUrl, false); // synchronous
			xhr.send();
			if (xhr.status !== 200) return; // no plugins

			var manifest = JSON.parse(xhr.responseText);
			if (!manifest || !Array.isArray(manifest.scripts)) return;

			// Load case config if declared
			if (manifest.config) {
				var configUrl = caseBase + 'case_config.json';
				try {
					var cxhr = new XMLHttpRequest();
					cxhr.open('GET', configUrl, false);
					cxhr.send();
					if (cxhr.status === 200) {
						var caseConfig = JSON.parse(cxhr.responseText);
						EngineConfig.loadCaseConfig(caseConfig);
					}
				} catch (ce) {
					console.warn('[EnginePlugins] Failed to load case_config.json:', ce.message);
				}
			}

			// Inject plugin script tags
			for (var i = 0; i < manifest.scripts.length; i++) {
				var scriptUrl = caseBase + 'plugins/' + manifest.scripts[i];
				var script = document.createElement('script');
				script.src = scriptUrl;
				script.async = false; // preserve order
				document.head.appendChild(script);
			}
		} catch (e) {
			console.warn('[EnginePlugins] Failed to load plugins manifest:', e.message);
		}
	}

	// ============================================================
	// SECTION: Public API
	// ============================================================

	return {
		_init: function() {
			// Listen for player:init to trigger plugin initialization
			EngineEvents.on('player:init', function() {
				isReady = true;
				loadCasePlugins();
				initAllPending();
			}, 0, 'engine');
		},

		/**
		 * Register a plugin. Call this from your plugin script.
		 * The init function will be called after player:init fires.
		 */
		register: function(descriptor) {
			if (!descriptor || typeof descriptor !== 'object') return;
			if (typeof descriptor.init !== 'function') {
				console.warn('[EnginePlugins] Plugin "' + (descriptor.name || 'unknown') + '" has no init function');
			}
			registry.push(descriptor);

			if (isReady) {
				descriptor._initialized = true;
				if (!frozenApi) frozenApi = Object.freeze(buildApi());
				initPlugin(descriptor);
			}
		},

		getLoaded: function() {
			var names = [];
			for (var i = 0; i < registry.length; i++) {
				if (registry[i].name) names.push(registry[i].name);
			}
			return names;
		},

		isLoaded: function(name) {
			for (var i = 0; i < registry.length; i++) {
				if (registry[i].name === name) return true;
			}
			return false;
		},

		/** Build the API object (exposed for testing). */
		_buildApi: buildApi
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('engine_plugins');
