"use strict";
/*
Ace Attorney Online - Keyboard Controls

Listens to input:action events (emitted by InputManager) and maps
action names to DOM button clicks.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'keyboard_controls',
	dependencies : ['engine_events', 'input_registry', 'events', 'page_loaded'],
	init : function()
	{
		InputRegistry.register({ action: 'back_escape', label: 'back', keyboard: 'Escape', gamepad: 'B', source: 'engine' });

		var proceedIds = ['proceed', 'present-center', 'statement-forwards', 'statement-skip-forwards'];
		var backId = 'statement-backwards';
		var forwardIds = ['statement-forwards', 'statement-skip-forwards'];
		var backButtonIds = ['cr-item-check-back', 'back', 'examination-back'];
		var pressIds = ['press'];
		var presentIds = ['present-center', 'present-topright'];

		function isVisible(el) {
			if (!el) return false;
			// Must have layout (not display:none or parent hidden)
			// AND not visibility:hidden (which keeps layout but is invisible/unclickable)
			if (el.offsetWidth === 0 && el.offsetHeight === 0) return false;
			return getComputedStyle(el).visibility !== 'hidden';
		}

		function clickFirstVisible(ids) {
			for (var i = 0; i < ids.length; i++) {
				var el = document.getElementById(ids[i]);
				if (el && isVisible(el)) {
					el.click();
					return true;
				}
			}
			return false;
		}

		// Escape key → click visible back button (#back or #examination-back)
		document.addEventListener('keydown', function(e) {
			if (InputManager.isModuleDisabled('keyboard_controls')) return;
			if (e.code === 'Escape' || e.key === 'Escape') {
				if (clickFirstVisible(backButtonIds)) {
					e.preventDefault();
				}
			}
		});

		// Listen to input:action events from InputManager
		EngineEvents.on('input:action', function(data) {
			if (InputManager.isModuleDisabled('keyboard_controls')) return;
			if (data.source !== 'keyboard') return;

			switch (data.action) {
				case 'proceed':
					clickFirstVisible(proceedIds);
					break;
				case 'back':
					var el = document.getElementById(backId);
					if (el && isVisible(el)) el.click();
					break;
				case 'forward':
					clickFirstVisible(forwardIds);
					break;
				case 'press':
					clickFirstVisible(pressIds);
					break;
				case 'present':
					clickFirstVisible(presentIds);
					break;
			}
		}, 0, 'engine');
	}
}));

//END OF MODULE
Modules.complete('keyboard_controls');
