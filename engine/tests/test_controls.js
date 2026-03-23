"use strict";
/**
 * Controls regression tests (EXHAUSTIVE).
 * Tests the full input pipeline: keydown → InputManager → input:action → keyboard_controls → DOM click.
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

	// InputManager module is loaded (status 3)
	TestHarness.assertEqual(
		Modules.request_list['input_manager'], 3,
		'InputManager module is loaded (status 3)'
	);

	// Helper: create a KeyboardEvent with code, key, and keyCode set.
	// InputManager uses event.code (primary) and event.key (fallback).
	function createKeyEvent(type, code, key, keyCode) {
		var event = new KeyboardEvent(type, {
			code: code,
			key: key,
			keyCode: keyCode,
			which: keyCode,
			bubbles: true,
			cancelable: true
		});
		// Ensure code/key are set (some browsers may not honor KeyboardEvent init dict)
		if (event.code !== code) {
			try { Object.defineProperty(event, 'code', { get: function() { return code; } }); } catch (e) {}
		}
		if (event.key !== key) {
			try { Object.defineProperty(event, 'key', { get: function() { return key; } }); } catch (e) {}
		}
		if (event.keyCode !== keyCode) {
			try { Object.defineProperty(event, 'keyCode', { get: function() { return keyCode; } }); } catch (e) {}
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

	// Simulated keydown Enter clicks proceed button
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'Enter', 'Enter', 13));
	document.dispatchEvent(createKeyEvent('keyup', 'Enter', 'Enter', 13));
	TestHarness.assert(
		clickLog.indexOf('proceed') > -1,
		'Simulated keydown Enter clicks proceed button (when visible)'
	);

	// Simulated keydown Space clicks proceed button
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'Space', ' ', 32));
	document.dispatchEvent(createKeyEvent('keyup', 'Space', ' ', 32));
	TestHarness.assert(
		clickLog.indexOf('proceed') > -1,
		'Simulated keydown Space clicks proceed button (when visible)'
	);

	// Simulated keydown Shift clicks proceed button (allows repeat via skip action)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'ShiftLeft', 'Shift', 16));
	TestHarness.assert(
		clickLog.indexOf('proceed') > -1,
		'Simulated keydown Shift clicks proceed button (allows repeat)'
	);
	document.dispatchEvent(createKeyEvent('keyup', 'ShiftLeft', 'Shift', 16));

	// Simulated keydown Left (keyCode 37) clicks statement-backwards
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'ArrowLeft', 'ArrowLeft', 37));
	document.dispatchEvent(createKeyEvent('keyup', 'ArrowLeft', 'ArrowLeft', 37));
	TestHarness.assert(
		clickLog.indexOf('statement-backwards') > -1,
		'Simulated keydown Left clicks statement-backwards (when visible)'
	);

	// Simulated keydown Right clicks statement-forwards or skip-forwards
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'ArrowRight', 'ArrowRight', 39));
	document.dispatchEvent(createKeyEvent('keyup', 'ArrowRight', 'ArrowRight', 39));
	TestHarness.assert(
		clickLog.indexOf('statement-forwards') > -1 || clickLog.indexOf('statement-skip-forwards') > -1,
		'Simulated keydown Right clicks statement-forwards or statement-skip-forwards (when visible)'
	);

	// Enter does NOT fire twice when held (pressed guard)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'Enter', 'Enter', 13));
	document.dispatchEvent(createKeyEvent('keydown', 'Enter', 'Enter', 13)); // repeat
	var enterClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 'Enter', 'Enter', 13));
	TestHarness.assertEqual(enterClicks, 1, 'Enter does NOT fire twice when held (pressed guard)');

	// Space does NOT fire twice when held (pressed guard)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'Space', ' ', 32));
	document.dispatchEvent(createKeyEvent('keydown', 'Space', ' ', 32)); // repeat
	var spaceClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 'Space', ' ', 32));
	TestHarness.assertEqual(spaceClicks, 1, 'Space does NOT fire twice when held (pressed guard)');

	// Shift DOES fire repeatedly when held (skip action allows repeat)
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'ShiftLeft', 'Shift', 16));
	document.dispatchEvent(createKeyEvent('keydown', 'ShiftLeft', 'Shift', 16)); // repeat
	var shiftClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 'ShiftLeft', 'Shift', 16));
	TestHarness.assert(shiftClicks >= 2, 'Shift DOES fire repeatedly when held (no pressed guard)');

	// Left does NOT fire twice when held
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'ArrowLeft', 'ArrowLeft', 37));
	document.dispatchEvent(createKeyEvent('keydown', 'ArrowLeft', 'ArrowLeft', 37)); // repeat
	var leftClicks = clickLog.filter(function(id) { return id === 'statement-backwards'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 'ArrowLeft', 'ArrowLeft', 37));
	TestHarness.assertEqual(leftClicks, 1, 'Left does NOT fire twice when held');

	// Right does NOT fire twice when held
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'ArrowRight', 'ArrowRight', 39));
	document.dispatchEvent(createKeyEvent('keydown', 'ArrowRight', 'ArrowRight', 39)); // repeat
	var rightClicks = clickLog.filter(function(id) {
		return id === 'statement-forwards' || id === 'statement-skip-forwards';
	}).length;
	document.dispatchEvent(createKeyEvent('keyup', 'ArrowRight', 'ArrowRight', 39));
	TestHarness.assertEqual(rightClicks, 1, 'Right does NOT fire twice when held');

	// keyup resets pressed state for Enter, Space, Left, Right
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'Enter', 'Enter', 13));
	document.dispatchEvent(createKeyEvent('keyup', 'Enter', 'Enter', 13));
	document.dispatchEvent(createKeyEvent('keydown', 'Enter', 'Enter', 13));
	var resetClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	document.dispatchEvent(createKeyEvent('keyup', 'Enter', 'Enter', 13));
	TestHarness.assertEqual(resetClicks, 2, 'keyup resets pressed state (Enter fires again after keyup)');

	// Keyboard does not fire when target button is not visible (display:none)
	if (proceedBtn) { proceedBtn.style.display = 'none'; proceedBtn.style.visibility = 'hidden'; }
	clickLog = [];
	document.dispatchEvent(createKeyEvent('keydown', 'Enter', 'Enter', 13));
	document.dispatchEvent(createKeyEvent('keyup', 'Enter', 'Enter', 13));
	var invisClicks = clickLog.filter(function(id) { return id === 'proceed'; }).length;
	TestHarness.assertEqual(invisClicks, 0, 'Keyboard does not fire when target button is not visible');

	// Restore button display
	if (proceedBtn) { proceedBtn.style.display = origProceedDisplay; proceedBtn.style.visibility = ''; }
	if (stmtBack) { stmtBack.style.display = origBackDisplay; stmtBack.style.visibility = ''; }
	if (stmtFwd) { stmtFwd.style.display = origFwdDisplay; stmtFwd.style.visibility = ''; }

	// Gamepad pollGamepads: verify the module loaded (can't test real gamepad without device)
	TestHarness.assert(
		Modules.request_list['gamepad_controls'] === 3,
		'Gamepad pollGamepads is active (module loaded successfully)'
	);
}
