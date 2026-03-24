"use strict";
/*
Ace Attorney Online - Keyboard Controls

Listens to input:action events (emitted by InputManager) and maps
action names to DOM button clicks.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'keyboard_controls',
	dependencies : ['engine_events', 'events', 'page_loaded'],
	init : function()
	{
		var proceedIds = ['proceed', 'skip', 'statement-forwards', 'statement-skip-forwards'];
		var backId = 'statement-backwards';
		var forwardIds = ['statement-forwards', 'statement-skip-forwards'];

		function isVisible(el) {
			if (!el) return false;
			var s = getComputedStyle(el);
			return s.display !== 'none' && s.visibility !== 'hidden';
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

		// Listen to input:action events from InputManager
		EngineEvents.on('input:action', function(data) {
			if (data.source !== 'keyboard') return;

			switch (data.action) {
				case 'proceed':
				case 'skip':
					clickFirstVisible(proceedIds);
					break;
				case 'back':
					var el = document.getElementById(backId);
					if (el && isVisible(el)) el.click();
					break;
				case 'forward':
					clickFirstVisible(forwardIds);
					break;
			}
		}, 0, 'engine');
	}
}));

//END OF MODULE
Modules.complete('keyboard_controls');
