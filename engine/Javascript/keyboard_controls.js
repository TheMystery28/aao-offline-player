"use strict";
/*
Ace Attorney Online - Keyboard Controls

Enter/Space: click the visible proceed/skip/forward button
Shift: same as Enter (for fast-forwarding through text)
Left Arrow: click statement-backwards
Right Arrow: click statement-forwards or statement-skip-forwards
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'keyboard_controls',
	dependencies : ['events', 'page_loaded'],
	init : function()
	{
		var KEY_ENTER = 13;
		var KEY_SPACE = 32;
		var KEY_SHIFT = 16;
		var KEY_LEFT  = 37;
		var KEY_RIGHT = 39;

		// Buttons in priority order for Enter/Space/Shift
		var proceedIds = ['proceed', 'skip', 'statement-forwards', 'statement-skip-forwards'];
		var backId = 'statement-backwards';
		var forwardIds = ['statement-forwards', 'statement-skip-forwards'];

		var pressed = {};

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

		document.addEventListener('keydown', function(e) {
			var k = e.keyCode;

			if ((k === KEY_ENTER || k === KEY_SPACE) && !pressed[k]) {
				pressed[k] = true;
				clickFirstVisible(proceedIds);
				e.preventDefault();
			}

			if (k === KEY_SHIFT) {
				clickFirstVisible(proceedIds);
				e.preventDefault();
			}

			if (k === KEY_RIGHT && !pressed[k]) {
				pressed[k] = true;
				clickFirstVisible(forwardIds);
				e.preventDefault();
			}

			if (k === KEY_LEFT && !pressed[k]) {
				pressed[k] = true;
				var el = document.getElementById(backId);
				if (el && isVisible(el)) el.click();
				e.preventDefault();
			}
		});

		document.addEventListener('keyup', function(e) {
			delete pressed[e.keyCode];
		});
	}
}));

//END OF MODULE
Modules.complete('keyboard_controls');
