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
	dependencies : ['engine_config', 'engine_events', 'nodes', 'events', 'form_elements', 'language', 'page_loaded'],
	init : function()
	{
		SettingsPanel._init();
	}
}));

//INDEPENDENT INSTRUCTIONS

var SettingsPanel = (function() {
	const controls = [];

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

	function addSlider(container, configPath, labelKey, min, max, step) {
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
		slider.value = String(EngineConfig.get(configPath));
		wrapper.appendChild(slider);

		const valueDisplay = document.createElement('span');
		valueDisplay.textContent = slider.value;
		addClass(valueDisplay, 'slider-value');
		wrapper.appendChild(valueDisplay);

		slider.addEventListener('input', function() {
			valueDisplay.textContent = slider.value;
			EngineConfig.set(configPath, parseFloat(slider.value));
		});

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

	function addBindingsDisplay(container) {
		const kbConfig = EngineConfig.get('controls.keyboard');
		if (!kbConfig) return;

		const table = document.createElement('table');
		addClass(table, 'bindings-display');
		const actions = Object.keys(kbConfig);
		for (let i = 0; i < actions.length; i++) {
			const keys = kbConfig[actions[i]];
			if (!Array.isArray(keys) || keys.length === 0) continue;
			const row = document.createElement('tr');
			const actionCell = document.createElement('td');
			actionCell.textContent = actions[i];
			row.appendChild(actionCell);
			const keyCell = document.createElement('td');
			keyCell.textContent = keys.join(', ');
			row.appendChild(keyCell);
			table.appendChild(row);
		}
		container.appendChild(table);
	}

	function syncAll() {
		for (let i = 0; i < controls.length; i++) {
			controls[i].sync();
		}
	}

	function buildPanel(container) {
		emptyNode(container);

		// --- Display section ---
		const displaySection = document.createElement('div');
		addClass(displaySection, 'settings-section');
		const displayTitle = document.createElement('h3');
		displayTitle.textContent = 'Display';
		displaySection.appendChild(displayTitle);

		addCheckbox(displaySection, 'display.mute', 'mute');
		addCheckbox(displaySection, 'display.instantText', 'instant_text_typing');
		addCheckbox(displaySection, 'display.nightMode', 'night_mode');
		addCheckbox(displaySection, 'display.pixelated', 'pixelated');
		addSlider(displaySection, 'display.textSpeed', 'text_speed', 0.1, 3.0, 0.1);
		addSlider(displaySection, 'display.blipVolume', 'blip_volume', 0, 100, 5);
		addCheckbox(displaySection, 'display.expandEvidenceDescriptions', 'expand_descriptions');

		container.appendChild(displaySection);

		// --- Layout section ---
		const layoutSection = document.createElement('div');
		addClass(layoutSection, 'settings-section');
		const layoutTitle = document.createElement('h3');
		layoutTitle.textContent = 'Layout';
		layoutSection.appendChild(layoutTitle);

		addSlider(layoutSection, 'layout.screenScale', 'screen_scale', 0.5, 2.0, 0.1);
		addSelect(layoutSection, 'layout.courtRecordPosition', 'cr_position', [
			{ value: 'right', label: 'Right' },
			{ value: 'left', label: 'Left' },
			{ value: 'bottom', label: 'Bottom' },
			{ value: 'hidden', label: 'Hidden' }
		]);

		container.appendChild(layoutSection);

		// --- Controls section ---
		const controlsSection = document.createElement('div');
		addClass(controlsSection, 'settings-section');
		const controlsTitle = document.createElement('h3');
		controlsTitle.textContent = 'Controls';
		controlsSection.appendChild(controlsTitle);

		addBindingsDisplay(controlsSection);
		addResetButton(controlsSection);

		container.appendChild(controlsSection);
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
			});
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('settings_panel');
