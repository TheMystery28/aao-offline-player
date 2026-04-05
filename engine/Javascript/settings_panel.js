"use strict";
/*
Ace Attorney Online - Settings Panel

Auto-generates config-driven settings controls in #player_settings.
Each control reads from EngineConfig.get(), writes via EngineConfig.set(),
and syncs on config:changed events.
ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'settings_panel',
	dependencies : ['engine_config', 'engine_events', 'input_registry', 'nodes', 'events', 'form_elements', 'language', 'page_loaded'],
	init : function()
	{
		SettingsPanel._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var SettingsPanel = (function() {
	const controls = [];
	let pickerContainer = null;
	let rowGroup = null;
	let mixedGroup = null;
	let narrowModeWrapper = null;
	let layoutDetailsRef = null;
	let bodyWidthWrapper = null;
	let bodyWidthSlider = null;
	let bodyWidthDisplay = null;
	let evidenceWidthWrapper = null;
	let settingsWidthWrapper = null;

	// Layout arrangement definitions: value → [type, blocks]
	// type: 'row' | 'mixed' | 'top'
	// For row: array of 3 block classes (left to right)
	// For mixed: array of 2 top blocks + 1 bottom block class
	// For top: top block class + array of 2 bottom blocks
	var ROW_LAYOUTS = [
		{ value: '1-2-3', blocks: ['s', 'e', 'p'] },
		{ value: '1-3-2', blocks: ['s', 'p', 'e'] },
		{ value: '2-1-3', blocks: ['e', 's', 'p'] },
		{ value: '2-3-1', blocks: ['e', 'p', 's'] },
		{ value: '3-1-2', blocks: ['p', 's', 'e'] },
		{ value: '3-2-1', blocks: ['p', 'e', 's'] }
	];
	var MIXED_LAYOUTS = [];
	var TOP_LAYOUTS = [];

	/** Show or hide an element via display style. */
	function showHide(el, visible) {
		if (el) el.style.display = visible ? '' : 'none';
	}

	function addCheckbox(container, configPath, labelKey) {
		const checkbox = createFormElement('checkbox');
		checkbox.checked = !!EngineConfig.get(configPath);
		registerEventHandler(checkbox, 'change', function() {
			EngineConfig.set(configPath, checkbox.getValue());
		}, false);
		container.appendChild(createLabel(checkbox, labelKey));

		controls.push({
			element: checkbox,
			path: configPath,
			sync: function() { checkbox.checked = !!EngineConfig.get(configPath); }
		});
	}

	// Ghost preview: freeze all 3 panels, show semi-transparent outlines at
	// the positions they would have with the new flex values.
	var ghostOverlay = null;

	function freezePanels() {
		var section = document.querySelector('#content > section');
		var screens = document.getElementById('screens');
		var panels = [
			document.getElementById('courtrecord'),
			document.getElementById('player-parametres')
		];
		// Read widths BEFORE writing styles (avoid reflow cascade).
		// For panels with scrollbar (overflow-y:auto), getComputedStyle().width
		// excludes the scrollbar (~10px). Use offsetWidth - padding - border instead.
		// For panels without scrollbar, getComputedStyle().width is exact (sub-pixel).
		var widths = [];
		for (var i = 0; i < panels.length; i++) {
			if (!panels[i]) { widths[i] = '0px'; continue; }
			var cs = getComputedStyle(panels[i]);
			var hasScrollbar = (panels[i].offsetWidth - panels[i].clientWidth) > 2;
			if (hasScrollbar) {
				var contentW = panels[i].offsetWidth
					- parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight)
					- parseFloat(cs.borderLeftWidth) - parseFloat(cs.borderRightWidth);
				widths[i] = contentW + 'px';
			} else {
				widths[i] = cs.width;
			}
		}
		// Freeze screens: use offsetWidth (pre-transform base) as flex-basis.
		// Screens has transform: scale() applied on top, and flex-basis is pre-transform,
		// so offsetWidth gives the correct value without double-scaling.
		var screensW = screens ? screens.offsetWidth + 'px' : '256px';

		// Freeze screens: lock flex-basis, transform, AND margins so nothing shifts during drag
		if (screens && !screens.hasAttribute('data-frozen')) {
			var currentTransform = screens.style.transform || getComputedStyle(screens).transform || 'none';
			var cs = getComputedStyle(screens);
			screens.setAttribute('data-frozen', '1');
			screens.style.flex = '0 0 ' + screensW;
			screens.style.minWidth = screensW;
			screens.style.maxWidth = screensW;
			screens.style.transform = currentTransform;
			screens.style.marginRight = cs.marginRight;
			screens.style.marginBottom = cs.marginBottom;
			screens.style.contain = 'size layout';
		}
		// Freeze cr + settings
		for (var j = 0; j < panels.length; j++) {
			if (panels[j] && !panels[j].hasAttribute('data-frozen')) {
				panels[j].setAttribute('data-frozen', '1');
				panels[j].style.flex = '0 0 ' + widths[j];
				panels[j].style.minWidth = widths[j];
				panels[j].style.maxWidth = widths[j];
				panels[j].style.contain = 'size layout';
			}
		}
		if (section) {
			section.style.contain = 'layout';
			section.style.userSelect = 'none';
			section.style.flexWrap = 'nowrap';
		}
		// Freeze body max-width so page width slider doesn't resize during drag
		document.body.style.maxWidth = document.body.offsetWidth + 'px';
	}

	function unfreezePanels() {
		var section = document.querySelector('#content > section');
		var allPanels = [
			document.getElementById('screens'),
			document.getElementById('courtrecord'),
			document.getElementById('player-parametres')
		];
		for (var i = 0; i < allPanels.length; i++) {
			if (allPanels[i] && allPanels[i].hasAttribute('data-frozen')) {
				allPanels[i].removeAttribute('data-frozen');
				allPanels[i].style.flex = '';
				allPanels[i].style.minWidth = '';
				allPanels[i].style.maxWidth = '';
				allPanels[i].style.transform = '';
				allPanels[i].style.marginRight = '';
				allPanels[i].style.marginBottom = '';
				allPanels[i].style.contain = '';
			}
		}
		if (section) {
			section.style.contain = '';
			section.style.userSelect = '';
			section.style.flexWrap = '';
		}
		document.body.style.maxWidth = '';
		removeGhosts();
	}

	function createGhosts() {
		if (ghostOverlay) return;
		ghostOverlay = document.createElement('div');
		ghostOverlay.className = 'ghost-overlay';
		// Attach to document.body with position:fixed so ghosts can extend
		// beyond the frozen body boundary when page width slider grows.
		ghostOverlay.style.cssText = 'position:fixed;top:0;left:0;width:100vw;height:100vh;pointer-events:none;z-index:50;';
		document.body.appendChild(ghostOverlay);
	}

	function updateGhosts() {
		if (!ghostOverlay) return;
		var section = document.querySelector('#content > section');
		var screens = document.getElementById('screens');
		if (!section || !screens) return;

		// Compute ghost positions MATHEMATICALLY — no DOM unfreeze.
		var sectionRect = section.getBoundingClientRect();
		var rootStyles = getComputedStyle(document.documentElement);
		var liveScale = parseFloat(rootStyles.getPropertyValue('--screen-content-scale')) || 1;
		var screensBaseW = screens.offsetWidth;
		var screensW = screensBaseW * liveScale;

		// Compute the FUTURE section width from live --body-max-width (vw units)
		var liveBodyMaxStr = rootStyles.getPropertyValue('--body-max-width').trim();
		var futureSectionW;
		if (liveBodyMaxStr.indexOf('vw') !== -1) {
			futureSectionW = Math.min(parseFloat(liveBodyMaxStr) / 100 * window.innerWidth, window.innerWidth);
		} else {
			futureSectionW = Math.min(parseFloat(liveBodyMaxStr) || 1090, window.innerWidth);
		}
		// Center offset: the future body is centered via margin:auto
		var futureLeft = Math.max(0, (window.innerWidth - futureSectionW) / 2);
		// Use the frozen section's top for vertical position
		var sectionTop = sectionRect.top;
		var sectionH = sectionRect.height;

		var availableW = futureSectionW - screensW;
		var eFlex = parseFloat(rootStyles.getPropertyValue('--evidence-flex')) || 0.7;
		var sFlex = parseFloat(rootStyles.getPropertyValue('--settings-flex')) || 0.4;
		var totalFlex = eFlex + sFlex;
		var evidenceW = (totalFlex > 0) ? (availableW * eFlex / totalFlex) : (availableW / 2);
		var settingsW = availableW - evidenceW;

		var colors = ['rgba(80,80,80,0.3)', 'rgba(100,170,100,0.3)', 'rgba(180,130,70,0.3)'];
		ghostOverlay.innerHTML =
			buildGhostDiv(futureLeft, sectionTop, screensW, sectionH, colors[0]) +
			buildGhostDiv(futureLeft + screensW, sectionTop, evidenceW, sectionH, colors[1]) +
			buildGhostDiv(futureLeft + screensW + evidenceW, sectionTop, settingsW, sectionH, colors[2]);
	}

	function buildGhostDiv(left, top, width, height, color) {
		return '<div style="position:absolute;left:' + left + 'px;top:' + top + 'px;' +
			'width:' + width + 'px;height:' + height + 'px;' +
			'background:' + color + ';border:2px dashed rgba(255,255,255,0.4);' +
			'border-radius:4px;box-sizing:border-box;"></div>';
	}

	function removeGhosts() {
		if (ghostOverlay && ghostOverlay.parentNode) {
			ghostOverlay.parentNode.removeChild(ghostOverlay);
		}
		ghostOverlay = null;
	}

	function addSlider(container, configPath, labelKey, min, max, step, options) {
		var isLayout = options && options.layout;
		const wrapper = document.createElement('label');
		addClass(wrapper, 'regular_label');

		const title = document.createElement('span');
		title.setAttribute('data-locale-content', labelKey);
		wrapper.appendChild(title);

		const slider = document.createElement('input');
		slider.type = 'range';
		slider.min = String(min);
		slider.max = String(max);
		slider.step = String(step);
		var configVal = EngineConfig.get(configPath);
		slider.value = String(configVal !== undefined ? configVal : min);
		wrapper.appendChild(slider);

		const valueDisplay = document.createElement('span');
		valueDisplay.textContent = slider.value;
		addClass(valueDisplay, 'slider-value');
		wrapper.appendChild(valueDisplay);

		// Prevent native gestures from stealing pointer capture
		slider.style.touchAction = 'none';

		if (isLayout) {
			// Freeze panels + show ghosts during drag to prevent layout flickering
			slider.addEventListener('pointerdown', function(e) {
				freezePanels();
				createGhosts();
				if (slider.setPointerCapture) {
					slider.setPointerCapture(e.pointerId);
				}
			});
		}
		slider.addEventListener('input', function() {
			valueDisplay.textContent = slider.value;
			EngineConfig.set(configPath, parseFloat(slider.value));
			if (isLayout) updateGhosts();
		});
		if (isLayout) {
			slider.addEventListener('pointerup', unfreezePanels);
			slider.addEventListener('pointercancel', unfreezePanels);
		}

		container.appendChild(wrapper);
		translateNode(wrapper);

		controls.push({
			element: slider,
			path: configPath,
			sync: function() {
				const val = EngineConfig.get(configPath);
				slider.value = String(val);
				valueDisplay.textContent = String(val);
			}
		});
	}

	function addSelect(container, configPath, labelKey, options) {
		const wrapper = document.createElement('label');
		addClass(wrapper, 'regular_label');

		const title = document.createElement('span');
		title.setAttribute('data-locale-content', labelKey);
		wrapper.appendChild(title);

		const select = document.createElement('select');
		for (let i = 0; i < options.length; i++) {
			const opt = document.createElement('option');
			opt.value = options[i].value;
			opt.textContent = options[i].label;
			select.appendChild(opt);
		}
		select.value = String(EngineConfig.get(configPath));
		wrapper.appendChild(select);

		select.addEventListener('change', function() {
			EngineConfig.set(configPath, select.value);
		});

		container.appendChild(wrapper);
		translateNode(wrapper);

		controls.push({
			element: select,
			path: configPath,
			sync: function() { select.value = String(EngineConfig.get(configPath)); }
		});
	}

	function addResetButton(container) {
		const btn = document.createElement('button');
		btn.textContent = 'Reset to defaults';
		btn.addEventListener('click', function() {
			EngineConfig.reset();
			syncAll();
		});
		container.appendChild(btn);
	}

	var bindingsContainer = null;

	function buildBindingsTable() {
		var entries = InputRegistry.getAll();

		var table = document.createElement('table');
		addClass(table, 'bindings-display');

		// Header row
		var header = document.createElement('tr');
		var h1 = document.createElement('th'); h1.textContent = 'Action'; header.appendChild(h1);
		var h2 = document.createElement('th'); h2.textContent = 'Keyboard'; header.appendChild(h2);
		var h3 = document.createElement('th'); h3.textContent = 'Gamepad'; header.appendChild(h3);
		table.appendChild(header);

		for (var i = 0; i < entries.length; i++) {
			var e = entries[i];
			var row = document.createElement('tr');
			// Plugin entries get a subtle style
			if (e.source.indexOf('plugin:') === 0) {
				row.style.fontStyle = 'italic';
				row.style.opacity = '0.8';
			}
			var ac = document.createElement('td'); ac.textContent = e.label; row.appendChild(ac);
			var kc = document.createElement('td'); kc.textContent = e.keyboard || ''; row.appendChild(kc);
			var gc = document.createElement('td'); gc.textContent = e.gamepad || ''; row.appendChild(gc);
			// Action-level grayout (settings toggles)
			if (InputManager.isActionDisabled(e.action)) {
				row.classList.add('control-disabled');
			}
			// Per-source module grayout (plugin disable): full row or single cell
			else if (e.module) {
				var kbOff = InputManager.isModuleDisabled(e.module, 'keyboard');
				var gpOff = InputManager.isModuleDisabled(e.module, 'gamepad');
				if (kbOff && gpOff) {
					row.classList.add('control-disabled');
				} else {
					if (kbOff) kc.classList.add('control-disabled');
					if (gpOff) gc.classList.add('control-disabled');
				}
			}
			table.appendChild(row);
		}

		return table;
	}

	function refreshBindingsDisplay() {
		if (!bindingsContainer) return;
		emptyNode(bindingsContainer);
		bindingsContainer.appendChild(buildBindingsTable());
	}

	function addBindingsDisplay(container) {
		bindingsContainer = container;
		refreshBindingsDisplay();

		// Live-update when registry changes (plugin load/unload, config change)
		EngineEvents.on('controls:registry:changed', refreshBindingsDisplay, 0, 'engine');
		// Live-update when a module is disabled/enabled (per-source grayout)
		EngineEvents.on('controls:module:changed', refreshBindingsDisplay, 0, 'engine');
		// Live-update when an action is disabled/enabled (settings toggles)
		EngineEvents.on('controls:action:changed', refreshBindingsDisplay, 0, 'engine');
	}

	function buildLayoutPicker(container, configPath) {
		var wrapper = document.createElement('div');
		wrapper.className = 'layout-picker-wrapper';

		// Legend
		var legend = document.createElement('div');
		legend.className = 'layout-picker-legend';
		var legendItems = [
			{ cls: 'legend-screen', text: 'Screen' },
			{ cls: 'legend-evidence', text: 'Evidence' },
			{ cls: 'legend-settings', text: 'Settings' }
		];
		for (var li = 0; li < legendItems.length; li++) {
			var span = document.createElement('span');
			span.className = legendItems[li].cls;
			span.appendChild(document.createTextNode(legendItems[li].text));
			legend.appendChild(span);
		}
		wrapper.appendChild(legend);

		pickerContainer = document.createElement('div');
		pickerContainer.className = 'layout-picker';

		// Row group
		rowGroup = document.createElement('div');
		rowGroup.className = 'layout-thumb-group';
		for (var r = 0; r < ROW_LAYOUTS.length; r++) {
			rowGroup.appendChild(createRowThumb(ROW_LAYOUTS[r], configPath));
		}
		pickerContainer.appendChild(rowGroup);

		// Mixed + Top group (shown together)
		mixedGroup = document.createElement('div');
		mixedGroup.className = 'layout-thumb-group';
		for (var m = 0; m < MIXED_LAYOUTS.length; m++) {
			mixedGroup.appendChild(createMixedThumb(MIXED_LAYOUTS[m], configPath));
		}
		for (var t = 0; t < TOP_LAYOUTS.length; t++) {
			mixedGroup.appendChild(createTopThumb(TOP_LAYOUTS[t], configPath));
		}
		pickerContainer.appendChild(mixedGroup);

		wrapper.appendChild(pickerContainer);
		container.appendChild(wrapper);

		// Initial selection highlight
		updatePickerSelection(configPath);

		// Sync on config changes
		controls.push({
			element: pickerContainer,
			path: configPath,
			sync: function() { updatePickerSelection(configPath); }
		});
	}

	function createRowThumb(def, configPath) {
		var thumb = document.createElement('div');
		thumb.className = 'layout-thumb row-layout';
		thumb.setAttribute('data-value', def.value);
		for (var i = 0; i < def.blocks.length; i++) {
			var block = document.createElement('span');
			block.className = 'block block-' + def.blocks[i];
			thumb.appendChild(block);
		}
		thumb.addEventListener('click', function() {
			EngineConfig.set(configPath, def.value);
		});
		return thumb;
	}

	function createMixedThumb(def, configPath) {
		var thumb = document.createElement('div');
		thumb.className = 'layout-thumb mixed-layout';
		thumb.setAttribute('data-value', def.value);
		for (var i = 0; i < def.top.length; i++) {
			var block = document.createElement('span');
			block.className = 'block block-top block-' + def.top[i];
			thumb.appendChild(block);
		}
		var bottom = document.createElement('span');
		bottom.className = 'block block-bottom block-' + def.bottom;
		thumb.appendChild(bottom);
		thumb.addEventListener('click', function() {
			EngineConfig.set(configPath, def.value);
		});
		return thumb;
	}

	function createTopThumb(def, configPath) {
		var thumb = document.createElement('div');
		thumb.className = 'layout-thumb top-layout';
		thumb.setAttribute('data-value', def.value);
		var topBlock = document.createElement('span');
		topBlock.className = 'block block-top block-' + def.top;
		thumb.appendChild(topBlock);
		var bottomRow = document.createElement('div');
		bottomRow.className = 'block-bottom-row';
		for (var i = 0; i < def.bottom.length; i++) {
			var b = document.createElement('span');
			b.className = 'block block-' + def.bottom[i];
			bottomRow.appendChild(b);
		}
		thumb.appendChild(bottomRow);
		thumb.addEventListener('click', function() {
			EngineConfig.set(configPath, def.value);
		});
		return thumb;
	}

	function updatePickerSelection(configPath) {
		if (!pickerContainer) return;
		var current = String(EngineConfig.get(configPath));
		var thumbs = pickerContainer.querySelectorAll('.layout-thumb');
		for (var i = 0; i < thumbs.length; i++) {
			if (thumbs[i].getAttribute('data-value') === current) {
				thumbs[i].classList.add('selected');
			} else {
				thumbs[i].classList.remove('selected');
			}
		}
	}

	function isRowLayout(value) {
		return value && value.length === 5 && value.charAt(1) === '-' && value.charAt(3) === '-';
	}

	function updateBodyWidthBounds() {
		if (!bodyWidthSlider || typeof ThemeManager === 'undefined' || !ThemeManager.getMinBodyScale) return;
		var minScale = ThemeManager.getMinBodyScale();
		if (minScale > 0) {
			bodyWidthSlider.min = String(minScale);
			if (parseFloat(bodyWidthSlider.value) < minScale) {
				bodyWidthSlider.value = String(minScale);
				if (bodyWidthDisplay) bodyWidthDisplay.textContent = String(minScale);
				EngineConfig.set('layout.bodyWidth', minScale);
			}
		} else {
			bodyWidthSlider.min = '0.5';
		}
		// Cap max to where body reaches 100vw (beyond that has no effect)
		if (ThemeManager.getMaxBodyScale) {
			var maxScale = ThemeManager.getMaxBodyScale();
			bodyWidthSlider.max = String(maxScale);
			if (parseFloat(bodyWidthSlider.value) > maxScale) {
				bodyWidthSlider.value = String(maxScale);
				if (bodyWidthDisplay) bodyWidthDisplay.textContent = String(maxScale);
				EngineConfig.set('layout.bodyWidth', maxScale);
			}
		}
	}

	function syncAll() {
		for (let i = 0; i < controls.length; i++) {
			controls[i].sync();
		}
	}

	function buildPanel(container) {
		emptyNode(container);

		// --- Reset defaults button (top of settings) ---
		var resetBtn = document.createElement('button');
		resetBtn.textContent = 'Reset defaults';
		resetBtn.style.width = '100%';
		resetBtn.style.marginBottom = '5px';
		resetBtn.addEventListener('click', function() {
			EngineConfig.reset();
			syncAll();
		});
		container.appendChild(resetBtn);

		// --- Display section ---
		const displayDetails = document.createElement('details');
		displayDetails.open = true;
		const displaySummary = document.createElement('summary');
		displaySummary.textContent = 'Display';
		displayDetails.appendChild(displaySummary);

		const displayContent = document.createElement('div');
		addClass(displayContent, 'settings-section-content');

		addCheckbox(displayContent, 'display.mute', 'mute');
		addCheckbox(displayContent, 'display.instantText', 'instant_text_typing');
		addCheckbox(displayContent, 'display.nightMode', 'night_mode');
		addCheckbox(displayContent, 'display.pixelated', 'pixelated');
		addSlider(displayContent, 'display.textSpeed', 'text_speed', 0.1, 3.0, 0.1);
		addSlider(displayContent, 'display.blipVolume', 'blip_volume', 0, 100, 5);
		addCheckbox(displayContent, 'display.expandEvidenceDescriptions', 'expand_descriptions');
		addCheckbox(displayContent, 'display.hideHeader', 'hide_header');
		addCheckbox(displayContent, 'display.fullscreen', 'fullscreen');

		var actionSep = document.createElement('hr');
		actionSep.setAttribute('aria-hidden', 'true');
		actionSep.className = 'settings-separator';
		displayContent.appendChild(actionSep);

		addCheckbox(displayContent, 'features.actionProceed', 'action_proceed');
		addCheckbox(displayContent, 'features.actionBack', 'action_back');
		addCheckbox(displayContent, 'features.actionPressPresent', 'action_press_present');
		addCheckbox(displayContent, 'features.actionStatements', 'action_statements');
		addCheckbox(displayContent, 'features.optionNavigation', 'option_navigation');

		displayDetails.appendChild(displayContent);
		container.appendChild(displayDetails);

		// --- Layout section ---
		const layoutDetails = document.createElement('details');
		layoutDetails.open = true;
		const layoutSummary = document.createElement('summary');
		layoutSummary.textContent = 'Layout';
		layoutDetails.appendChild(layoutSummary);

		const layoutContent = document.createElement('div');
		addClass(layoutContent, 'settings-section-content');

		addSlider(layoutContent, 'layout.bodyWidth', 'body_width', 0.5, 2.0, 0.01, { layout: true });
		// Capture body width slider for dynamic min clamping
		var bwLabel = layoutContent.lastElementChild;
		bodyWidthSlider = bwLabel ? bwLabel.querySelector('input[type="range"]') : null;
		bodyWidthDisplay = bwLabel ? bwLabel.querySelector('.slider-value') : null;
		addSlider(layoutContent, 'layout.screenScale', 'screen_scale', 0.5, 2.0, 0.1, { layout: true });
		addSlider(layoutContent, 'layout.evidenceWidth', 'evidence_width', 0.3, 2.0, 0.1, { layout: true });
		addSlider(layoutContent, 'layout.settingsWidth', 'settings_width', 0.3, 2.0, 0.1, { layout: true });
		buildLayoutPicker(layoutContent, 'layout.panelArrangement');
		addSelect(layoutContent, 'layout.narrowMode', 'narrow_mode', [
			{ value: 'tabs', label: 'Tabs' },
			{ value: 'stack', label: 'Stack' }
		]);
		layoutDetailsRef = layoutDetails;
		// Store references to width sliders and narrowMode for dynamic visibility
		var labels = layoutContent.querySelectorAll('.regular_label');
		for (var i = 0; i < labels.length; i++) {
			var span = labels[i].querySelector('[data-locale-content]');
			if (!span) continue;
			var key = span.getAttribute('data-locale-content');
			if (key === 'narrow_mode') narrowModeWrapper = labels[i];
			if (key === 'body_width') bodyWidthWrapper = labels[i];
			if (key === 'evidence_width') evidenceWidthWrapper = labels[i];
			if (key === 'settings_width') settingsWidthWrapper = labels[i];
		}

		layoutDetails.appendChild(layoutContent);
		container.appendChild(layoutDetails);

		// --- Controls section ---
		const controlsDetails = document.createElement('details');
		controlsDetails.open = true;
		const controlsSummary = document.createElement('summary');
		controlsSummary.textContent = 'Controls';
		controlsDetails.appendChild(controlsSummary);

		const controlsContent = document.createElement('div');
		addClass(controlsContent, 'settings-section-content');

		addBindingsDisplay(controlsContent);

		controlsDetails.appendChild(controlsContent);
		container.appendChild(controlsDetails);

		// --- Plugins section (inserted before Controls) ---
		if (typeof EnginePlugins !== 'undefined' && EnginePlugins.buildSettingsPanel) {
			EnginePlugins.buildSettingsPanel(container, controlsDetails);
		}
	}

	return {
		_init: function() {
			const container = document.getElementById('player_settings');
			if (container) {
				buildPanel(container);
			}

			// Sync controls when config changes externally
			EngineEvents.on('config:changed', function() {
				syncAll();
			}, 0, 'engine');

			// Update body width slider min on resize (wide threshold depends on viewport)
			window.addEventListener('resize', function() {
				updateBodyWidthBounds();
			});
		},

		/**
		 * Update the layout picker and narrowMode visibility based on current tier.
		 * Called by ThemeManager after tier detection.
		 * @param {string} tier - 'wide', 'tabs', 'medium', or 'narrow'
		 * @param {boolean} wideIsPossible - true if the window is wide enough for wide mode
		 */
		updateLayoutTier: function(tier, wideIsPossible) {
			var isWide = (tier === 'wide');
			var showNarrow = isWide || (tier === 'tabs' && wideIsPossible);
			showHide(pickerContainer, isWide);
			showHide(rowGroup, isWide);
			showHide(mixedGroup, isWide);
			showHide(narrowModeWrapper, showNarrow);
			showHide(bodyWidthWrapper, isWide);
			showHide(evidenceWidthWrapper, isWide);
			showHide(settingsWidthWrapper, isWide);
			showHide(layoutDetailsRef, isWide || showNarrow);
			// Clamp body width slider min to the threshold that keeps wide mode
			updateBodyWidthBounds();
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('settings_panel');
