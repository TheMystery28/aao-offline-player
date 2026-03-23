"use strict";
/**
 * Player core regression tests (EXHAUSTIVE).
 */
function testPlayer() {
	TestHarness.suite('Player');

	// player_status is defined as an object
	TestHarness.assertDefined(player_status, 'player_status is defined');
	TestHarness.assertType(player_status, 'object', 'player_status is an object');

	// player_status has expected fields
	var statusFields = [
		'current_frame_id', 'current_frame_index', 'next_frame_index',
		'last_frame_merged', 'latest_action_frame_index', 'computed_parameters',
		'game_env', 'health', 'health_flash', 'game_over_target'
	];
	for (var i = 0; i < statusFields.length; i++) {
		TestHarness.assert(
			statusFields[i] in player_status,
			'player_status has field: ' + statusFields[i]
		);
	}

	// player_status has proceed fields
	var proceedFields = [
		'proceed_click', 'proceed_click_met',
		'proceed_timer', 'proceed_timer_met',
		'proceed_typing', 'proceed_typing_met'
	];
	for (var j = 0; j < proceedFields.length; j++) {
		TestHarness.assert(
			proceedFields[j] in player_status,
			'player_status has proceed field: ' + proceedFields[j]
		);
	}

	// resetProceedConditions
	TestHarness.assertType(resetProceedConditions, 'function', 'resetProceedConditions is a function');

	// resetProceedConditions() with no args resets all 6 proceed flags to false
	player_status.proceed_click = true;
	player_status.proceed_click_met = true;
	player_status.proceed_timer = true;
	player_status.proceed_timer_met = true;
	player_status.proceed_typing = true;
	player_status.proceed_typing_met = true;
	resetProceedConditions();
	TestHarness.assert(
		!player_status.proceed_click && !player_status.proceed_click_met &&
		!player_status.proceed_timer && !player_status.proceed_timer_met &&
		!player_status.proceed_typing && !player_status.proceed_typing_met,
		'resetProceedConditions() with no args resets all 6 proceed flags to false'
	);

	// resetProceedConditions(['click']) resets only proceed_click and proceed_click_met
	player_status.proceed_click = true;
	player_status.proceed_click_met = true;
	player_status.proceed_timer = true;
	player_status.proceed_timer_met = true;
	resetProceedConditions(['click']);
	TestHarness.assert(
		!player_status.proceed_click && !player_status.proceed_click_met,
		'resetProceedConditions([click]) resets click flags'
	);
	TestHarness.assert(
		player_status.proceed_timer && player_status.proceed_timer_met,
		'resetProceedConditions([click]) leaves timer flags intact'
	);

	// resetProceedConditions(['timer']) resets only proceed_timer and proceed_timer_met
	player_status.proceed_timer = true;
	player_status.proceed_timer_met = true;
	player_status.proceed_click = true;
	resetProceedConditions(['timer']);
	TestHarness.assert(
		!player_status.proceed_timer && !player_status.proceed_timer_met,
		'resetProceedConditions([timer]) resets timer flags'
	);
	TestHarness.assert(player_status.proceed_click, 'resetProceedConditions([timer]) leaves click flag intact');

	// addProceedCondition
	TestHarness.assertType(addProceedCondition, 'function', 'addProceedCondition is a function');

	resetProceedConditions();
	addProceedCondition('click');
	TestHarness.assert(player_status.proceed_click, 'addProceedCondition(click) sets proceed_click to true');

	resetProceedConditions();
	addProceedCondition('timer');
	TestHarness.assert(player_status.proceed_timer, 'addProceedCondition(timer) sets proceed_timer to true');

	resetProceedConditions();
	addProceedCondition('typing');
	TestHarness.assert(player_status.proceed_typing, 'addProceedCondition(typing) sets proceed_typing to true');

	// proceed
	TestHarness.assertType(proceed, 'function', 'proceed is a function');

	// proceed('click') sets proceed_click_met to true
	resetProceedConditions();
	addProceedCondition('click');
	addProceedCondition('timer'); // Add another condition so proceed doesn't advance
	proceed('click');
	TestHarness.assert(player_status.proceed_click_met, 'proceed(click) sets proceed_click_met to true');

	// proceed does NOT advance frame when another condition is unmet
	// (If timer is still unmet, readFrame should NOT have been called —
	//  we test this by checking that the frame index hasn't changed)
	var savedIndex = player_status.next_frame_index;
	resetProceedConditions();
	addProceedCondition('click');
	addProceedCondition('timer');
	proceed('click');
	TestHarness.assertEqual(
		player_status.next_frame_index, savedIndex,
		'proceed does NOT advance frame when another condition is unmet'
	);

	// readFrame is a function
	TestHarness.assertType(readFrame, 'function', 'readFrame is a function');

	// top_screen is defined (ScreenDisplay instance or undefined when no trial)
	if (typeof trial_data !== 'undefined' && trial_data) {
		TestHarness.assertDefined(top_screen, 'top_screen is defined (ScreenDisplay instance)');
	}

	// bottom_screen is defined (DOM element or undefined when no trial)
	if (typeof trial_data !== 'undefined' && trial_data) {
		TestHarness.assertDefined(bottom_screen, 'bottom_screen is defined (DOM element)');
	}

	// player_init is a function
	TestHarness.assertType(player_init, 'function', 'player_init is a function');

	// Start button exists and is clickable
	var startBtn = document.getElementById('start');
	TestHarness.assert(startBtn !== null, 'Start button exists');

	// Proceed button exists and is clickable
	var proceedBtn = document.getElementById('proceed');
	TestHarness.assert(proceedBtn !== null, 'Proceed button exists');

	// Statement-forwards and statement-backwards buttons exist
	TestHarness.assert(document.getElementById('statement-forwards') !== null, 'statement-forwards button exists');
	TestHarness.assert(document.getElementById('statement-backwards') !== null, 'statement-backwards button exists');

	// --- Behavioral regression tests (minimal fake case) ---
	// These tests use a stub trial_data and top_screen so they run even without
	// a real case loaded. All globals are saved and restored afterward.
	(function() {
		var orig = {
			trial_data: typeof trial_data !== 'undefined' ? trial_data : undefined,
			top_screen: typeof top_screen !== 'undefined' ? top_screen : undefined,
			bottom_screen: typeof bottom_screen !== 'undefined' ? bottom_screen : undefined,
			game_env: player_status.game_env,
			last_frame_merged: player_status.last_frame_merged,
			current_frame_id: player_status.current_frame_id,
			current_frame_index: player_status.current_frame_index,
			next_frame_index: player_status.next_frame_index,
			latest_action_frame_index: player_status.latest_action_frame_index,
			computed_parameters: player_status.computed_parameters
		};

		function fakeFrame(id) {
			return {
				action_name: null, action_parameters: {},
				characters: [], characters_erase_previous: true, fade: null,
				hidden: false, id: id, merged_to_next: false,
				music: 0, music_fade: null, place: -1, place_position: -1,
				place_transition: 0, popups: [], sound: 0,
				speaker_id: 0, speaker_name: '', speaker_use_name: false,
				speaker_voice: 0, text_colour: 'white', text_content: '',
				text_speed: 1, wait_time: 0
			};
		}

		// Set up minimal fake environment
		trial_data = { frames: [0, fakeFrame(42), fakeFrame(43), fakeFrame(44)] };
		top_screen = {
			iconsPrepareClear: function() {},
			loadFrame: function() {},  // Don't call callback — avoids runFrameActionAfter issues
			text_display: {
				name_box: { textContent: '' },
				dialogue_box: { innerHTML: '', textContent: '' }
			}
		};
		bottom_screen = document.getElementById('screen-bottom');
		player_status.game_env = new VariableEnvironment();
		player_status.last_frame_merged = false;

		// Test: readFrame(1) sets player_status.current_frame_id
		readFrame(1);
		TestHarness.assertEqual(
			player_status.current_frame_id, 42,
			'readFrame(1) sets player_status.current_frame_id to frame id'
		);
		TestHarness.assertEqual(
			player_status.current_frame_index, 1,
			'readFrame(1) sets player_status.current_frame_index to 1'
		);
		TestHarness.assertEqual(
			player_status.next_frame_index, 2,
			'readFrame(1) sets player_status.next_frame_index to 2'
		);

		// Test: proceed('click') after addProceedCondition('click') advances frame
		// readFrame(1) already set click condition; reset and set up cleanly
		resetProceedConditions();
		addProceedCondition('click');
		proceed('click');
		// proceed should have called readFrame(2), which sets current_frame_id to 43
		TestHarness.assertEqual(
			player_status.current_frame_id, 43,
			'proceed(click) advances frame: current_frame_id updated'
		);
		TestHarness.assertEqual(
			player_status.current_frame_index, 2,
			'proceed(click) advances frame: current_frame_index is now 2'
		);

		// Restore all globals
		trial_data = orig.trial_data;
		top_screen = orig.top_screen;
		bottom_screen = orig.bottom_screen;
		player_status.game_env = orig.game_env;
		player_status.last_frame_merged = orig.last_frame_merged;
		player_status.current_frame_id = orig.current_frame_id;
		player_status.current_frame_index = orig.current_frame_index;
		player_status.next_frame_index = orig.next_frame_index;
		player_status.latest_action_frame_index = orig.latest_action_frame_index;
		player_status.computed_parameters = orig.computed_parameters;
	})();

	// Test: stopMusic sets current_music_id to MUSIC_STOP
	(function() {
		var orig_music_id = current_music_id;
		current_music_id = 999;
		stopMusic();
		TestHarness.assertEqual(
			current_music_id, MUSIC_STOP,
			'stopMusic sets current_music_id to MUSIC_STOP'
		);
		current_music_id = orig_music_id;
	})();

	// Reset proceed state to clean
	resetProceedConditions();
}
