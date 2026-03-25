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
	var resolvedPluginData = null; // { active: [...], available: [...] } from resolved_plugins.json

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
			var handle = descriptor.init(EngineConfig, EngineEvents, frozenApi);
			if (handle && typeof handle.destroy === 'function') {
				descriptor._handle = handle;
			}
		} catch (e) {
			console.error('[EnginePlugins] Plugin "' + (descriptor.name || 'unknown') + '" crashed during init:', e);
		}
	}

	function getResolvedParamsForPlugin(name) {
		if (resolvedPluginData && Array.isArray(resolvedPluginData.active)) {
			for (var i = 0; i < resolvedPluginData.active.length; i++) {
				if (resolvedPluginData.active[i].script && resolvedPluginData.active[i].script.replace('.js', '') === name) {
					return resolvedPluginData.active[i].params || {};
				}
				// Also match by filename directly
				if (resolvedPluginData.active[i].script === name + '.js') {
					return resolvedPluginData.active[i].params || {};
				}
			}
		}
		return {};
	}

	function getPluginParams(name) {
		var result = {};
		// 1. Plugin declared defaults
		for (var i = 0; i < registry.length; i++) {
			if (registry[i].name === name && registry[i].params) {
				var keys = Object.keys(registry[i].params);
				for (var k = 0; k < keys.length; k++) {
					if (registry[i].params[keys[k]].default !== undefined) {
						result[keys[k]] = registry[i].params[keys[k]].default;
					}
				}
				break;
			}
		}
		// 2. Resolved params from resolved_plugins.json (overrides defaults)
		var resolved = getResolvedParamsForPlugin(name);
		var rKeys = Object.keys(resolved);
		for (var r = 0; r < rKeys.length; r++) {
			result[rKeys[r]] = resolved[rKeys[r]];
		}
		// 3. Session overrides from EngineConfig (most specific)
		for (var sk in result) {
			var sessionVal = EngineConfig.get('plugins.' + name + '.params.' + sk);
			if (sessionVal !== undefined && sessionVal !== null) {
				result[sk] = sessionVal;
			}
		}
		return result;
	}

	function reapplyPlugin(desc) {
		if (desc._handle && typeof desc._handle.destroy === 'function') {
			desc._handle.destroy();
			initPlugin(desc);
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
	// SECTION: Plugin Settings Panel
	// ============================================================

	function buildSettingsPanel(container, beforeElement) {
		if (!container) return null;

		// Remove existing plugin settings panel if any
		var existing = container.querySelector('details[data-plugin-section="__plugins__"]');
		if (existing) existing.parentNode.removeChild(existing);

		var details = document.createElement('details');
		details.setAttribute('data-plugin-section', '__plugins__');
		var summary = document.createElement('summary');
		summary.textContent = 'Plugins';
		details.appendChild(summary);

		var content = document.createElement('div');
		content.className = 'settings-section-content';

		if (registry.length === 0) {
			var emptyMsg = document.createElement('div');
			emptyMsg.style.color = '#888';
			emptyMsg.style.fontSize = '11px';
			emptyMsg.style.padding = '4px 0';
			emptyMsg.textContent = 'No plugins loaded.';
			content.appendChild(emptyMsg);
		} else {
			for (var i = 0; i < registry.length; i++) {
				(function(desc) {
					var label = document.createElement('label');
					label.className = 'regular_label';
					var cb = document.createElement('input');
					cb.type = 'checkbox';
					var configKey = 'plugins.' + desc.name + '.enabled';
					var enabled = EngineConfig.get(configKey);
					cb.checked = (enabled === undefined || enabled === null) ? !desc._disabled : !!enabled;

					cb.addEventListener('change', function() {
						if (cb.checked) {
							EngineConfig.set(configKey, true);
							if (desc._disabled && desc._handle && typeof desc._handle.destroy === 'function') {
								desc._disabled = false;
								initPlugin(desc);
							} else if (desc._disabled) {
								desc._disabled = false;
							}
						} else {
							EngineConfig.set(configKey, false);
							if (desc._handle && typeof desc._handle.destroy === 'function') {
								desc._handle.destroy();
								desc._disabled = true;
							} else {
								desc._disabled = true;
							}
						}
					});

					label.appendChild(cb);
					var text = ' ' + (desc.name || 'unnamed');
					if (desc.version) text += ' v' + desc.version;
					if (!desc._handle || typeof desc._handle.destroy !== 'function') {
						text += ' (reload to apply)';
					}
					label.appendChild(document.createTextNode(text));
					content.appendChild(label);

					// --- Param editors ---
					if (desc.params && typeof desc.params === 'object') {
						var paramKeys = Object.keys(desc.params);
						for (var pi = 0; pi < paramKeys.length; pi++) {
							(function(paramKey, paramDef) {
								var resolvedParams = getResolvedParamsForPlugin(desc.name);
								var currentVal = resolvedParams[paramKey];
								if (currentVal === undefined) currentVal = paramDef.default;
								// Check session override
								var sessionVal = EngineConfig.get('plugins.' + desc.name + '.params.' + paramKey);
								if (sessionVal !== undefined && sessionVal !== null) currentVal = sessionVal;

								var paramLabel = document.createElement('label');
								paramLabel.className = 'regular_label';
								paramLabel.style.paddingLeft = '20px';
								paramLabel.style.fontSize = '11px';

								var paramSpan = document.createElement('span');
								paramSpan.textContent = (paramDef.label || paramKey) + ': ';
								paramLabel.appendChild(paramSpan);

								var input;
								if (paramDef.type === 'checkbox') {
									input = document.createElement('input');
									input.type = 'checkbox';
									input.checked = !!currentVal;
									input.addEventListener('change', function() {
										EngineConfig.set('plugins.' + desc.name + '.params.' + paramKey, input.checked);
										reapplyPlugin(desc);
									});
								} else if (paramDef.type === 'number') {
									input = document.createElement('input');
									input.type = 'range';
									input.min = String(paramDef.min || 0);
									input.max = String(paramDef.max || 100);
									input.step = String(paramDef.step || 1);
									input.value = String(currentVal);
									var valDisplay = document.createElement('span');
									valDisplay.textContent = ' ' + currentVal;
									valDisplay.style.fontSize = '10px';
									input.addEventListener('input', function() {
										valDisplay.textContent = ' ' + input.value;
										EngineConfig.set('plugins.' + desc.name + '.params.' + paramKey, parseFloat(input.value));
										reapplyPlugin(desc);
									});
									paramLabel.appendChild(input);
									paramLabel.appendChild(valDisplay);
									content.appendChild(paramLabel);
									return; // already appended
								} else if (paramDef.type === 'select') {
									input = document.createElement('select');
									var opts = paramDef.options || [];
									for (var oi = 0; oi < opts.length; oi++) {
										var opt = document.createElement('option');
										if (typeof opts[oi] === 'object') {
											opt.value = opts[oi].value;
											opt.textContent = opts[oi].label;
										} else {
											opt.value = String(opts[oi]);
											opt.textContent = String(opts[oi]);
										}
										input.appendChild(opt);
									}
									input.value = String(currentVal);
									input.addEventListener('change', function() {
										EngineConfig.set('plugins.' + desc.name + '.params.' + paramKey, input.value);
										reapplyPlugin(desc);
									});
								} else {
									// text
									input = document.createElement('input');
									input.type = 'text';
									input.value = String(currentVal || '');
									input.style.cssText = 'width:120px;font-size:11px;padding:1px 4px;background:rgba(0,0,0,0.3);color:#ddd;border:1px solid rgba(255,255,255,0.15);border-radius:2px;';
									input.addEventListener('change', function() {
										EngineConfig.set('plugins.' + desc.name + '.params.' + paramKey, input.value);
										reapplyPlugin(desc);
									});
								}
								paramLabel.appendChild(input);
								content.appendChild(paramLabel);
							})(paramKeys[pi], desc.params[paramKeys[pi]]);
						}
					}
				})(registry[i]);
			}
		}

		// --- Available (disabled) plugins from resolved_plugins.json ---
		if (resolvedPluginData && Array.isArray(resolvedPluginData.available) && resolvedPluginData.available.length > 0) {
			var availHeader = document.createElement('div');
			availHeader.style.cssText = 'font-size:10px;color:#666;margin-top:8px;text-transform:uppercase;letter-spacing:0.04em;';
			availHeader.textContent = 'Available (disabled)';
			content.appendChild(availHeader);

			for (var ai = 0; ai < resolvedPluginData.available.length; ai++) {
				var avail = resolvedPluginData.available[ai];
				var availDiv = document.createElement('div');
				availDiv.style.cssText = 'font-size:11px;color:#555;opacity:0.6;padding:2px 0;';
				availDiv.setAttribute('data-available-plugin', avail.script);
				availDiv.textContent = avail.script + ' — ' + (avail.reason || 'not active for this case');
				content.appendChild(availDiv);
			}
		}

		// --- Attach Code UI ---
		var attachToggle = document.createElement('button');
		attachToggle.textContent = 'Attach Code...';
		attachToggle.style.cssText = 'margin-top:8px;padding:3px 10px;font-size:11px;cursor:pointer;background:rgba(255,255,255,0.08);color:#ccc;border:1px solid rgba(255,255,255,0.15);border-radius:3px;';

		var attachArea = document.createElement('div');
		attachArea.style.display = 'none';

		var detectedName = document.createElement('div');
		detectedName.style.cssText = 'font-size:10px;color:#888;margin-top:4px;min-height:14px;';

		var textarea = document.createElement('textarea');
		textarea.className = 'plugin-attach-textarea';
		textarea.rows = 6;
		textarea.placeholder = '// Paste plugin JS code here...\n// e.g. EnginePlugins.register({ name: "my_plugin", ... })';

		var loadBtn = document.createElement('button');
		loadBtn.textContent = 'Load Plugin';
		loadBtn.style.cssText = 'margin-top:4px;padding:3px 12px;font-size:11px;cursor:pointer;background:rgba(80,140,200,0.3);color:#adf;border:1px solid rgba(80,140,200,0.4);border-radius:3px;';

		attachToggle.addEventListener('click', function() {
			var isOpen = attachArea.style.display !== 'none';
			attachArea.style.display = isOpen ? 'none' : 'block';
			attachToggle.textContent = isOpen ? 'Attach Code...' : 'Hide';
		});

		textarea.addEventListener('input', function() {
			var match = textarea.value.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
			if (match) {
				detectedName.textContent = 'Detected: ' + match[1] + '.js';
			} else {
				detectedName.textContent = '';
			}
		});

		loadBtn.addEventListener('click', function() {
			var code = textarea.value.trim();
			if (!code) return;
			try {
				var beforeCount = registry.length;
				(new Function(code))();
				var afterCount = registry.length;
				if (afterCount > beforeCount) {
					var newPlugin = registry[afterCount - 1];
					console.log('[EnginePlugins] Loaded plugin from paste: ' + (newPlugin.name || 'unnamed'));
				}
				textarea.value = '';
				detectedName.textContent = '';
				attachArea.style.display = 'none';
				attachToggle.textContent = 'Attach Code...';
				buildSettingsPanel(container, beforeElement);
			} catch (e) {
				detectedName.textContent = 'Error: ' + e.message;
				detectedName.style.color = '#f88';
				setTimeout(function() { detectedName.style.color = '#888'; }, 3000);
			}
		});

		attachArea.appendChild(detectedName);
		attachArea.appendChild(textarea);
		attachArea.appendChild(loadBtn);

		content.appendChild(attachToggle);
		content.appendChild(attachArea);

		details.appendChild(content);

		if (beforeElement) {
			container.insertBefore(details, beforeElement);
		} else {
			container.appendChild(details);
		}

		return details;
	}

	// ============================================================
	// SECTION: Plugin Loading (global + case)
	// ============================================================

	function loadGlobalPlugins() {
		if (typeof trial_information === 'undefined' || !trial_information) return;

		// Try resolved_plugins.json first (written by Rust at play time)
		var resolvedUrl = 'case/' + trial_information.id + '/resolved_plugins.json';
		try {
			var rxhr = new XMLHttpRequest();
			rxhr.open('GET', resolvedUrl, false);
			rxhr.send();
			if (rxhr.status === 200) {
				var resolved = JSON.parse(rxhr.responseText);
				resolvedPluginData = resolved;
				var active = resolved.active || [];
				for (var i = 0; i < active.length; i++) {
					var scriptUrl = active[i].source;
					if (!scriptUrl) continue;
					var script = document.createElement('script');
					script.src = scriptUrl;
					script.async = false;
					script.setAttribute('data-plugin-scope', 'global');
					document.head.appendChild(script);
				}
				return; // resolved_plugins.json found — skip old fallback
			}
		} catch (e) {
			// resolved_plugins.json not available — fall back
		}

		// Fallback: old plugins/manifest.json behavior
		var manifestUrl = 'plugins/manifest.json';
		try {
			var xhr = new XMLHttpRequest();
			xhr.open('GET', manifestUrl, false);
			xhr.send();
			if (xhr.status !== 200) return;

			var manifest = JSON.parse(xhr.responseText);
			if (!manifest || !Array.isArray(manifest.scripts)) return;

			var disabledList = Array.isArray(manifest.disabled) ? manifest.disabled : [];
			for (var j = 0; j < manifest.scripts.length; j++) {
				if (disabledList.indexOf(manifest.scripts[j]) !== -1) continue;
				var sUrl = 'plugins/' + manifest.scripts[j];
				var s = document.createElement('script');
				s.src = sUrl;
				s.async = false;
				s.setAttribute('data-plugin-scope', 'global');
				document.head.appendChild(s);
			}
		} catch (e) {
			// No global plugins — that's fine
		}
	}

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

			// Inject plugin script tags (skip disabled ones)
			var disabledList = Array.isArray(manifest.disabled) ? manifest.disabled : [];
			for (var i = 0; i < manifest.scripts.length; i++) {
				if (disabledList.indexOf(manifest.scripts[i]) !== -1) {
					console.log('[EnginePlugins] Skipping disabled plugin: ' + manifest.scripts[i]);
					continue;
				}
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
				loadGlobalPlugins();
				loadCasePlugins();
				initAllPending();
				// Rebuild the settings panel after plugins load (it was created empty by settings_panel.js)
				setTimeout(function() {
					var container = document.getElementById('player_settings');
					if (!container) return;
					// Find the Controls section as the insertBefore reference
					var allDetails = container.querySelectorAll('details');
					var controlsRef = null;
					for (var k = 0; k < allDetails.length; k++) {
						var s = allDetails[k].querySelector('summary');
						if (s && s.textContent.indexOf('Controls') !== -1) {
							controlsRef = allDetails[k];
							break;
						}
					}
					buildSettingsPanel(container, controlsRef);
				}, 200);
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
		_buildApi: buildApi,

		/** Build the plugin settings panel inside a container, before a reference element. */
		buildSettingsPanel: buildSettingsPanel,

		/** Get resolved params for a plugin (cascade + session overrides). */
		getPluginParams: getPluginParams,

		/** Get the resolved plugin data (active + available). */
		getResolvedData: function() { return resolvedPluginData; }
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('engine_plugins');
