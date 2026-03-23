"use strict";
/**
 * Module system regression tests (EXHAUSTIVE).
 */
function testModules() {
	TestHarness.suite('Modules');

	// Core object exists
	TestHarness.assertDefined(Modules, 'Modules global object exists');
	TestHarness.assertType(Modules.load, 'function', 'Modules.load is a function');
	TestHarness.assertType(Modules.complete, 'function', 'Modules.complete is a function');
	TestHarness.assertType(Modules.request, 'function', 'Modules.request is a function');
	TestHarness.assertType(Modules.request_list, 'object', 'Modules.request_list is an object');
	TestHarness.assertType(Modules.module_list, 'object', 'Modules.module_list is an object');
	TestHarness.assertType(Modules.depending_on, 'object', 'Modules.depending_on is an object');
	TestHarness.assertType(Modules.callbacks, 'object', 'Modules.callbacks is an object');

	// Every expected module name is in request_list with status 3 (operational)
	var expectedModules = [
		'nodes', 'events', 'objects', 'objects_model', 'objects_diff',
		'trial', 'trial_data', 'trial_object_model',
		'frame_data', 'default_data',
		'player', 'player_sound', 'player_images', 'player_actions',
		'player_courtrecord', 'player_save', 'player_debug',
		'keyboard_controls', 'gamepad_controls',
		'display_engine_screen', 'display_engine_text',
		'display_engine_place', 'display_engine_characters',
		'display_engine_popups', 'display_engine_locks',
		'display_engine_cr_icons', 'display_engine_effects',
		'display_engine_globals',
		'expression_engine', 'var_environments',
		'language', 'style_loader', 'loading_bar',
		'sound-howler', 'base64',
		'form_elements'
	];

	for (var i = 0; i < expectedModules.length; i++) {
		var name = expectedModules[i];
		TestHarness.assertEqual(
			Modules.request_list[name], 3,
			'Module "' + name + '" is status 3 (operational)'
		);
	}

	// Pseudo-modules
	TestHarness.assertEqual(Modules.request_list['dom_loaded'], 3, 'dom_loaded pseudo-module is status 3');
	TestHarness.assertEqual(Modules.request_list['page_loaded'], 3, 'page_loaded pseudo-module is status 3');

	// Modules.request for an already-loaded module returns true
	TestHarness.assertEqual(Modules.request('nodes'), true, 'Modules.request for already-loaded module returns true');

	// Modules.request for an already-loaded module with callback fires callback immediately
	var callbackFired = false;
	Modules.request('nodes', function() { callbackFired = true; });
	TestHarness.assert(callbackFired, 'Modules.request with callback for loaded module fires callback immediately');

	// includeScript is a function
	TestHarness.assertType(includeScript, 'function', 'includeScript is a function');
}
