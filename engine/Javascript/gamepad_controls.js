"use strict";
/*
Ace Attorney Online - Gamepad Controls

Listens to input:action events (emitted by InputManager) and maps
action names to DOM button clicks and court record toggling.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'gamepad_controls',
	dependencies : ['engine_events', 'events', 'page_loaded'],
	init : function()
	{
		var proceedIds = ['skip', 'proceed', 'present-center', 'statement-forwards', 'statement-skip-forwards'];
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

		function clickById(id) {
			var el = document.getElementById(id);
			if (el && isVisible(el)) {
				el.click();
				return true;
			}
			return false;
		}

		// Listen to input:action events from InputManager
		EngineEvents.on('input:action', function(data) {
			if (InputManager.isModuleDisabled('gamepad_controls', 'gamepad')) return;
			if (data.source !== 'gamepad') return;

			if (data._consumed) return;

			switch (data.action) {
				case 'proceed':
					clickFirstVisible(proceedIds);
					break;
				case 'back':
				case 'backStatement':
					// Try statement back first, then back buttons (#back, #examination-back)
					if (!clickById(backId)) {
						clickFirstVisible(backButtonIds);
					}
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
Modules.complete('gamepad_controls');
