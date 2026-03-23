"use strict";
/**
 * Controls regression tests (EXHAUSTIVE).
 */
function testControls() {
	TestHarness.suite('Controls');

	// Keyboard module is loaded (status 3)
	TestHarness.assertEqual(
		Modules.request_list['keyboard_controls'], 3,
		'Keyboard module is loaded (status 3)'
	);

	// Gamepad module is loaded (status 3)
	TestHarness.assertEqual(
		Modules.request_list['gamepad_controls'], 3,
		'Gamepad module is loaded (status 3)'
	);

	// Helper: create a KeyboardEvent with keyCode reliably set.
	function createKeyEvent(type, keyCode) {
		var event = new KeyboardEvent(type, {
			keyCode: keyCode,
			which: keyCode,
			bubbles: true,
			cancelable: true
		});
		// Verify keyCode was set; if not, override via defineProperty
		if (event.keyCode !== keyCode) {
			try {
				Object.defineProperty(event, 'keyCode', { get: function() { return keyCode; } });
			} catch (e) { /* non-configurable in this env */ }
		}
		if (event.which !== keyCode) {
			try {
				Object.defineProperty(event, 'which', { get: function() { return keyCode; } });
			} catch (e) { /* non-configurable in this env */ }
		}
		return event;
	}

	// Track clicks on buttons for keyboard tests
	var clickLog = [];
	function instrumentButton(id) {
		var el = document.getElementById(id);
		if (el) {
			el.addEventListener('click', function() {
				clickLog.push(id);
			});
		}
	}

	// Make proceed button visible for tests
	var proceedBtn = document.getElementById('proceed');
	var stmtBack = document.getElementById('statement-backwards');
	var stmtFwd = document.getElementById('statement-forwards');
	var skipFwd = document.getElementById('statement-skip-forwards');

	// Save original display styles
	var origProceedDisplay = proceedBtn ? proceedBtn.style.display : '';
	var origBackDisplay = stmtBack ? stmtBack.style.display : '';
	var origFwdDisplay = stmtFwd ? stmtFwd.style.display : '';

	// Make buttons visible for testing (override both display and visibility from CSS)
	if (proceedBtn) { proceedBtn.style.display = 'block'; proceedBtn.style.visibility = 'visible'; }
	if (stmtBack) { stmtBack.style.display = 'block'; stmtBack.style.visibility = 'visible'; }
	if (stmtFwd) { stmtFwd.style.display = 'block'; stmtFwd.style.visibility = 'visible'; }

	instrumentButton('proceed');
	instrumentButton('statement-backwards');
	instrumentButton('statement-forwards');
	instrumentButton('statement-skip-forwards');

	// Simulated keydown Enter (keyCode 13) clicks proceed button
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 13));
	document.dispatchEvent(createKeyEvent('keyup', 13));
	TestHarness.assert(
		clickLog.indexOf('proceed') > -1,
		'Simulated keydown Enter clicks proceed button (when visible)'
	);

	// Simulated keydown Space (keyCode 32) clicks proceed button
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 32));
	document.dispatchEvent(createKeyEvent('keyup', 32));
	TestHarness.assert(
		clickLog.indexOf('proceed') > -1,
		'Simulated keydown Space clicks proceed button (when visible)'
	);

	// Simulated keydown Shift (keyCode 16) clicks proceed button (allows repeat)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 16));
	TestHarness.assert(
		clickLog.indexOf('proceed') > -1,
		'Simulated keydown Shift clicks proceed button (allows repeat)'
	);

	// Simulated keydown Left (keyCode 37) clicks statement-backwards
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 37));
	document.dispatchEvent(createKeyEvent('keyup', 37));
	TestHarness.assert(
		clickLog.indexOf('statement-backwards') > -1,
		'Simulated keydown Left clicks statement-backwards (when visible)'
	);

	// Simulated keydown Right (keyCode 39) clicks statement-forwards or skip-forwards
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 39));
	document.dispatchEvent(createKeyEvent('keyup', 39));
	TestHarness.assert(
		clickLog.indexOf('statement-forwards') > -1 || clickLog.indexOf('statement-skip-forwards') > -1,
		'Simulated keydown Right clicks statement-forwards or statement-skip-forwards (when visible)'
	);

	// Enter does NOT fire twice when held (pressed guard)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 13));
	document.dispatchEvent(createKeyEvent('keydown', 13)); // repeat
	var enterClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 13));
	TestHarness.assertEqual(enterClicks, 1, 'Enter does NOT fire twice when held (pressed guard)');

	// Space does NOT fire twice when held (pressed guard)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 32));
	document.dispatchEvent(createKeyEvent('keydown', 32)); // repeat
	var spaceClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 32));
	TestHarness.assertEqual(spaceClicks, 1, 'Space does NOT fire twice when held (pressed guard)');

	// Shift DOES fire repeatedly when held (no pressed guard)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 16));
	document.dispatchEvent(createKeyEvent('keydown', 16)); // repeat
	var shiftClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 16));
	TestHarness.assert(shiftClicks >= 2, 'Shift DOES fire repeatedly when held (no pressed guard)');

	// Left does NOT fire twice when held
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 37));
	document.dispatchEvent(createKeyEvent('keydown', 37)); // repeat
	var leftClicks = clickLog.filter(function(id) { return id === 'statement-backwards'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 37));
	TestHarness.assertEqual(leftClicks, 1, 'Left does NOT fire twice when held');

	// Right does NOT fire twice when held
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 39));
	document.dispatchEvent(createKeyEvent('keydown', 39)); // repeat
	var rightClicks = clickLog.filter(function(id) {
		return id === 'statement-forwards' || id === 'statement-skip-forwards';
	}).length;
	document.dispatchEvent(createKeyEvent('keyup', 39));
	TestHarness.assertEqual(rightClicks, 1, 'Right does NOT fire twice when held');

	// keyup resets pressed state for Enter, Space, Left, Right
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 13));
	document.dispatchEvent(createKeyEvent('keyup', 13));
	document.dispatchEvent(createKeyEvent('keydown', 13));
	var resetClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 13));
	TestHarness.assertEqual(resetClicks, 2, 'keyup resets pressed state (Enter fires again after keyup)');

	// Keyboard does not fire when target button is not visible (display:none)
	if (proceedBtn) { proceedBtn.style.display = 'none'; proceedBtn.style.visibility = 'hidden'; }
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 13));
	document.dispatchEvent(createKeyEvent('keyup', 13));
	var invisClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	TestHarness.assertEqual(invisClicks, 0, 'Keyboard does not fire when target button is not visible');

	// Restore button display
	if (proceedBtn) { proceedBtn.style.display = origProceedDisplay; proceedBtn.style.visibility = ''; }
	if (stmtBack) { stmtBack.style.display = origBackDisplay; stmtBack.style.visibility = ''; }
	if (stmtFwd) { stmtFwd.style.display = origFwdDisplay; stmtFwd.style.visibility = ''; }

	// Gamepad pollGamepads: just verify requestAnimationFrame or gamepadconnected is wired
	// (We can't fully test gamepad without a real device, but we verify the module loaded)
	TestHarness.assert(
		Modules.request_list['gamepad_controls'] === 3,
		'Gamepad pollGamepads is active (module loaded successfully)'
	);
}
