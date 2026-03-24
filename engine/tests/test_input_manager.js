"use strict";
/**
 * InputManager regression tests.
 * IMPORTANT: Do NOT call EngineEvents.clear() in these tests — it would remove
 * InputManager's internal config:changed listener. Use targeted on/off instead.
 */
function testInputManager() {
	TestHarness.suite('InputManager');

	// Clean up any stale localStorage from previous test runs and rebuild lookups
	window.localStorage.removeItem('aao_engine_config');
	EngineConfig.reset();
	EngineConfig._init();
	InputManager.rebuildLookups();

	// Helper: create a KeyboardEvent with code/key reliably set
	function makeKeyEvent(type, code, key, keyCode) {
		var event = new KeyboardEvent(type, {
			code: code, key: key, keyCode: keyCode, bubbles: true, cancelable: true
		});
		if (event.code !== code) {
			try { Object.defineProperty(event, 'code', { get: function() { return code; } }); } catch (e) {}
		}
		if (event.key !== key) {
			try { Object.defineProperty(event, 'key', { get: function() { return key; } }); } catch (e) {}
		}
		return event;
	}

	// Module is loaded
	TestHarness.assertEqual(
		Modules.request_list['input_manager'], 3,
		'input_manager module is loaded (status 3)'
	);

	// InputManager global exists with expected API
	TestHarness.assertDefined(InputManager, 'InputManager global is defined');
	TestHarness.assertType(InputManager.getKeyboardLookup, 'function', 'InputManager.getKeyboardLookup is a function');
	TestHarness.assertType(InputManager.getGamepadLookup, 'function', 'InputManager.getGamepadLookup is a function');

	// --- Default config maps keys correctly ---
	(function() {
		var lookup = InputManager.getKeyboardLookup();
		TestHarness.assertEqual(lookup['Enter'], 'proceed', 'Default config maps Enter to proceed');
		TestHarness.assertEqual(lookup['Space'], 'proceed', 'Default config maps Space to proceed');
		TestHarness.assertEqual(lookup['Shift'], 'skip', 'Default config maps Shift to skip');
		TestHarness.assertEqual(lookup['ArrowLeft'], 'back', 'Default config maps ArrowLeft to back');
		TestHarness.assertEqual(lookup['ArrowRight'], 'forward', 'Default config maps ArrowRight to forward');
		TestHarness.assertEqual(lookup['Tab'], 'crSwitchTab', 'Default config maps Tab to crSwitchTab');
	})();

	// --- Gamepad buttons map correctly ---
	(function() {
		var lookup = InputManager.getGamepadLookup();
		TestHarness.assertEqual(lookup['0'], 'proceed', 'Default gamepad maps button 0 to proceed');
		TestHarness.assertEqual(lookup['1'], 'back', 'Default gamepad maps button 1 to back');
		TestHarness.assertEqual(lookup['15'], 'forward', 'Default gamepad maps button 15 to forward');
		TestHarness.assertEqual(lookup['3'], 'crSwitchTab', 'Default gamepad maps button 3 (Y) to crSwitchTab');
	})();

	// --- input:action carries correct data ---
	(function() {
		// Verify lookup is correct before testing emission
		var lookup = InputManager.getKeyboardLookup();
		TestHarness.assertEqual(lookup['Enter'], 'proceed', 'Lookup has Enter→proceed before dispatch test');

		// Verify EngineEvents emission works (direct emit test)
		var directReceived = null;
		var directHandler = function(data) { directReceived = data; };
		EngineEvents.on('input:action', directHandler);
		EngineEvents.emit('input:action', { source: 'test', action: 'test' });
		TestHarness.assert(directReceived !== null, 'Direct EngineEvents.emit reaches handler');
		EngineEvents.off('input:action', directHandler);

		// Now test the full pipeline: keydown → InputManager → EngineEvents → handler
		var received = null;
		var handler = function(data) { if (data.source === 'keyboard') received = data; };
		EngineEvents.on('input:action', handler);

		document.dispatchEvent(makeKeyEvent('keydown', 'Enter', 'Enter', 13));

		TestHarness.assert(received !== null, 'input:action emitted on mapped keydown');
		if (received) {
			TestHarness.assertEqual(received.source, 'keyboard', 'input:action source is keyboard');
			TestHarness.assertEqual(received.action, 'proceed', 'input:action action is proceed for Enter');
		}

		document.dispatchEvent(makeKeyEvent('keyup', 'Enter', 'Enter', 13));
		EngineEvents.off('input:action', handler);
	})();

	// --- Unmapped keys don't emit ---
	(function() {
		var received = null;
		var handler = function(data) { received = data; };
		EngineEvents.on('input:action', handler);

		document.dispatchEvent(makeKeyEvent('keydown', 'KeyZ', 'z', 90));
		TestHarness.assert(received === null, 'Unmapped key (Z) does not emit input:action');

		document.dispatchEvent(makeKeyEvent('keyup', 'KeyZ', 'z', 90));
		EngineEvents.off('input:action', handler);
	})();

	// --- Config change remaps keys ---
	(function() {
		// Remap proceed to KeyQ
		EngineConfig.set('controls.keyboard.proceed', ['KeyQ']);
		// Force rebuild since config:changed listener may have been cleared by EngineEvents tests
		InputManager.rebuildLookups();

		var lookup = InputManager.getKeyboardLookup();
		TestHarness.assertEqual(lookup['KeyQ'], 'proceed', 'Config change: KeyQ now maps to proceed');
		TestHarness.assertEqual(lookup['Enter'], undefined, 'Config change: Enter no longer maps to proceed');

		// Verify KeyQ actually emits input:action
		var received = null;
		var handler = function(data) { received = data; };
		EngineEvents.on('input:action', handler);

		document.dispatchEvent(makeKeyEvent('keydown', 'KeyQ', 'q', 81));
		TestHarness.assert(received !== null, 'Remapped KeyQ emits input:action');
		TestHarness.assertEqual(received.action, 'proceed', 'Remapped KeyQ action is proceed');

		document.dispatchEvent(makeKeyEvent('keyup', 'KeyQ', 'q', 81));
		EngineEvents.off('input:action', handler);

		// Restore defaults and rebuild lookups
		EngineConfig.reset();
		InputManager.rebuildLookups();
		window.localStorage.removeItem('aao_engine_config');
	})();

	// --- input:release emitted on keyup ---
	(function() {
		var releaseData = null;
		var handler = function(data) { releaseData = data; };
		EngineEvents.on('input:release', handler);

		document.dispatchEvent(makeKeyEvent('keydown', 'Enter', 'Enter', 13));
		document.dispatchEvent(makeKeyEvent('keyup', 'Enter', 'Enter', 13));

		TestHarness.assert(releaseData !== null, 'input:release emitted on keyup');
		if (releaseData) {
			TestHarness.assertEqual(releaseData.source, 'keyboard', 'input:release source is keyboard');
			TestHarness.assertEqual(releaseData.action, 'proceed', 'input:release action is proceed for Enter');
		}

		EngineEvents.off('input:release', handler);
	})();

	// --- Tab maps to crSwitchTab ---
	(function() {
		var lookup = InputManager.getKeyboardLookup();
		TestHarness.assertEqual(lookup['Tab'], 'crSwitchTab', 'Tab key maps to crSwitchTab action');
	})();

	// --- Gamepad Y maps to crSwitchTab ---
	(function() {
		var lookup = InputManager.getGamepadLookup();
		TestHarness.assertEqual(lookup['3'], 'crSwitchTab', 'Gamepad button 3 (Y) maps to crSwitchTab');
	})();

	// --- Tab preventDefault ---
	(function() {
		var evt = new KeyboardEvent('keydown', { code: 'Tab', key: 'Tab', bubbles: true, cancelable: true });
		document.dispatchEvent(evt);
		TestHarness.assert(evt.defaultPrevented, 'Tab key preventDefault is called');
		// Cleanup keyup
		document.dispatchEvent(new KeyboardEvent('keyup', { code: 'Tab', key: 'Tab', bubbles: true }));
	})();

	// --- Gamepad save/loadLatest/fullscreen mapped ---
	(function() {
		var lookup = InputManager.getGamepadLookup();
		TestHarness.assertEqual(lookup['4'], 'save', 'Gamepad button 4 (LB) maps to save');
		TestHarness.assertEqual(lookup['6'], 'loadLatest', 'Gamepad button 6 (LT) maps to loadLatest');
		TestHarness.assertEqual(lookup['8'], 'fullscreen', 'Gamepad button 8 (View) maps to fullscreen');
	})();

	// --- unmapped key does not emit ---
	(function() {
		var received = false;
		var handler = function() { received = true; };
		EngineEvents.on('input:action', handler);
		document.dispatchEvent(makeKeyEvent('keydown', 'F2', 'F2', 113));
		document.dispatchEvent(makeKeyEvent('keyup', 'F2', 'F2', 113));
		TestHarness.assert(!received, 'Unmapped key (F2) does not emit input:action');
		EngineEvents.off('input:action', handler);
	})();

	// --- repeat guard ---
	(function() {
		var count = 0;
		var handler = function(data) { if (data.action === 'proceed') count++; };
		EngineEvents.on('input:action', handler);
		document.dispatchEvent(makeKeyEvent('keydown', 'Enter', 'Enter', 13));
		document.dispatchEvent(makeKeyEvent('keydown', 'Enter', 'Enter', 13));
		TestHarness.assertEqual(count, 1, 'Repeat guard: Enter held does not emit twice');
		document.dispatchEvent(makeKeyEvent('keyup', 'Enter', 'Enter', 13));
		EngineEvents.off('input:action', handler);
	})();

	// --- Ctrl+non-shortcut key does not trigger hardcoded shortcuts ---
	(function() {
		var configBefore = EngineConfig.get('display.nightMode');
		var ctrlA = makeKeyEvent('keydown', 'KeyA', 'a', 65);
		Object.defineProperty(ctrlA, 'ctrlKey', { value: true });
		document.dispatchEvent(ctrlA);
		var configAfter = EngineConfig.get('display.nightMode');
		TestHarness.assertEqual(configBefore, configAfter, 'Ctrl+A does not trigger any hardcoded shortcut');
		document.dispatchEvent(makeKeyEvent('keyup', 'KeyA', 'a', 65));
	})();

	// --- skip allows repeat ---
	(function() {
		var count = 0;
		var handler = function(data) { if (data.action === 'skip') count++; };
		EngineEvents.on('input:action', handler);
		document.dispatchEvent(makeKeyEvent('keydown', 'Shift', 'Shift', 16));
		document.dispatchEvent(makeKeyEvent('keydown', 'Shift', 'Shift', 16));
		TestHarness.assert(count >= 2, 'Skip action allows repeat (count=' + count + ')');
		document.dispatchEvent(makeKeyEvent('keyup', 'Shift', 'Shift', 16));
		EngineEvents.off('input:action', handler);
	})();
}
