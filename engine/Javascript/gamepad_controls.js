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

		function clickById(id) {
			var el = document.getElementById(id);
			if (el && isVisible(el)) {
				el.click();
				return true;
			}
			return false;
		}

		function toggleCourtRecord() {
			var cr = document.getElementById('courtrecord');
			if (!cr) return;
			var header = cr.querySelector('.courtrecord-header, #cr-header');
			if (header) {
				header.click();
				return;
			}
			if (cr.classList.contains('open')) {
				cr.classList.remove('open');
			} else {
				cr.classList.add('open');
			}
		}

		// Listen to input:action events from InputManager
		EngineEvents.on('input:action', function(data) {
			if (data.source !== 'gamepad') return;

			switch (data.action) {
				case 'proceed':
					clickFirstVisible(proceedIds);
					break;
				case 'back':
				case 'backStatement':
					clickById(backId);
					break;
				case 'forward':
					clickFirstVisible(forwardIds);
					break;
				case 'courtRecordToggle':
					toggleCourtRecord();
					break;
			}
		});
	}
}));

//END OF MODULE
Modules.complete('gamepad_controls');
