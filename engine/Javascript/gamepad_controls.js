"use strict";
/*
Ace Attorney Online - Gamepad Controls

A / Cross: proceed / skip / forward statement
B / Circle: back statement
D-pad Right: forward statement
D-pad Left: back statement
Start: toggle court record
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'gamepad_controls',
	dependencies : ['events', 'page_loaded'],
	init : function()
	{
		var proceedIds = ['proceed', 'skip', 'statement-forwards', 'statement-skip-forwards'];
		var backId = 'statement-backwards';
		var forwardIds = ['statement-forwards', 'statement-skip-forwards'];

		// Standard gamepad button indices (https://w3c.github.io/gamepad/#remapping)
		var BTN_A      = 0;  // A / Cross
		var BTN_B      = 1;  // B / Circle
		var BTN_START  = 9;  // Start / Options
		var DPAD_UP    = 12;
		var DPAD_DOWN  = 13;
		var DPAD_LEFT  = 14;
		var DPAD_RIGHT = 15;

		// Track pressed state to avoid repeat-firing
		var wasPressed = {};

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
					return;
				}
			}
		}

		function clickById(id) {
			var el = document.getElementById(id);
			if (el && isVisible(el)) el.click();
		}

		function toggleCourtRecord() {
			// The court record toggle is the header bar — clicking it opens/closes
			var cr = document.getElementById('courtrecord');
			if (!cr) return;
			var header = cr.querySelector('.courtrecord-header, #cr-header');
			if (header) {
				header.click();
				return;
			}
			// Fallback: toggle the 'open' class directly
			if (cr.classList.contains('open')) {
				cr.classList.remove('open');
			} else {
				cr.classList.add('open');
			}
		}

		function pollGamepads() {
			var gamepads = navigator.getGamepads ? navigator.getGamepads() : [];

			for (var g = 0; g < gamepads.length; g++) {
				var gp = gamepads[g];
				if (!gp) continue;

				var buttons = gp.buttons;

				// A / Cross → proceed
				if (buttons[BTN_A] && buttons[BTN_A].pressed) {
					if (!wasPressed[g + '_' + BTN_A]) {
						wasPressed[g + '_' + BTN_A] = true;
						clickFirstVisible(proceedIds);
					}
				} else {
					wasPressed[g + '_' + BTN_A] = false;
				}

				// B / Circle → back statement
				if (buttons[BTN_B] && buttons[BTN_B].pressed) {
					if (!wasPressed[g + '_' + BTN_B]) {
						wasPressed[g + '_' + BTN_B] = true;
						clickById(backId);
					}
				} else {
					wasPressed[g + '_' + BTN_B] = false;
				}

				// D-pad Right → forward statement
				if (buttons[DPAD_RIGHT] && buttons[DPAD_RIGHT].pressed) {
					if (!wasPressed[g + '_' + DPAD_RIGHT]) {
						wasPressed[g + '_' + DPAD_RIGHT] = true;
						clickFirstVisible(forwardIds);
					}
				} else {
					wasPressed[g + '_' + DPAD_RIGHT] = false;
				}

				// D-pad Left → back statement
				if (buttons[DPAD_LEFT] && buttons[DPAD_LEFT].pressed) {
					if (!wasPressed[g + '_' + DPAD_LEFT]) {
						wasPressed[g + '_' + DPAD_LEFT] = true;
						clickById(backId);
					}
				} else {
					wasPressed[g + '_' + DPAD_LEFT] = false;
				}

				// Start → toggle court record
				if (buttons[BTN_START] && buttons[BTN_START].pressed) {
					if (!wasPressed[g + '_' + BTN_START]) {
						wasPressed[g + '_' + BTN_START] = true;
						toggleCourtRecord();
					}
				} else {
					wasPressed[g + '_' + BTN_START] = false;
				}
			}

			requestAnimationFrame(pollGamepads);
		}

		// Start polling when a gamepad connects
		window.addEventListener('gamepadconnected', function() {
			requestAnimationFrame(pollGamepads);
		});

		// Also start immediately in case gamepad is already connected
		if (navigator.getGamepads) {
			var existing = navigator.getGamepads();
			for (var i = 0; i < existing.length; i++) {
				if (existing[i]) {
					requestAnimationFrame(pollGamepads);
					break;
				}
			}
		}
	}
}));

//END OF MODULE
Modules.complete('gamepad_controls');
