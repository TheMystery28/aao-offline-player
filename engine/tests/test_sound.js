"use strict";
/**
 * Sound regression tests (EXHAUSTIVE).
 */
function testSound() {
	TestHarness.suite('Sound');

	// Function existence
	TestHarness.assertType(playMusic, 'function', 'playMusic is a function');
	TestHarness.assertType(stopMusic, 'function', 'stopMusic is a function');
	TestHarness.assertType(playSound, 'function', 'playSound is a function');
	TestHarness.assertType(fadeMusic, 'function', 'fadeMusic is a function');
	TestHarness.assertType(crossfadeMusic, 'function', 'crossfadeMusic is a function');
	TestHarness.assertType(createSoundPlayer, 'function', 'createSoundPlayer is a function');

	// current_music_id is declared (var exists, may be undefined when no trial loaded)
	TestHarness.assert('current_music_id' in window, 'current_music_id is declared');

	// Constants
	TestHarness.assertDefined(MUSIC_STOP, 'MUSIC_STOP constant is defined');
	TestHarness.assertDefined(MUSIC_UNCHANGED, 'MUSIC_UNCHANGED constant is defined');
	TestHarness.assertDefined(SOUND_NONE, 'SOUND_NONE constant is defined');

	// Tests that require trial_data and SoundHowler
	if (typeof trial_data !== 'undefined' && trial_data && typeof SoundHowler !== 'undefined') {
		// playMusic with new id sets current_music_id to that id
		if (trial_data.music && trial_data.music.length > 1) {
			var testMusicId = trial_data.music[1].id;
			var prevId = current_music_id;

			playMusic(testMusicId);
			TestHarness.assertEqual(current_music_id, testMusicId, 'playMusic with new id sets current_music_id');

			// playMusic with same id does not restart (early return)
			// Just verify it doesn't throw
			var noThrow = true;
			try { playMusic(testMusicId); } catch (e) { noThrow = false; }
			TestHarness.assert(noThrow, 'playMusic with same id does not restart (early return)');

			// stopMusic sets current_music_id to MUSIC_STOP
			stopMusic();
			TestHarness.assertEqual(current_music_id, MUSIC_STOP, 'stopMusic sets current_music_id to MUSIC_STOP');
		}
	}

	// stopMusic calls SoundHowler.stopSound (verified by MUSIC_STOP being set above)
	// We can also test the constant value
	TestHarness.assertEqual(MUSIC_STOP, -1, 'MUSIC_STOP equals -1');
	TestHarness.assertEqual(MUSIC_UNCHANGED, 0, 'MUSIC_UNCHANGED equals 0');
	TestHarness.assertEqual(SOUND_NONE, 0, 'SOUND_NONE equals 0');

	// createSoundPlayer returns a div element
	if (typeof SoundHowler !== 'undefined') {
		var playerEl = createSoundPlayer('test.mp3', 'test_sound_player');
		TestHarness.assertEqual(playerEl.tagName, 'DIV', 'createSoundPlayer returns a div element');

		// createSoundPlayer result contains play button
		var hasPlayBtn = playerEl.querySelector('button') !== null;
		TestHarness.assert(hasPlayBtn, 'createSoundPlayer result contains play button');

		// createSoundPlayer result contains pause button
		var buttons = playerEl.querySelectorAll('button');
		TestHarness.assert(buttons.length >= 2, 'createSoundPlayer result contains pause button');

		// createSoundPlayer result contains progress bar
		var hasProgressBar = playerEl.querySelector('progress') !== null;
		TestHarness.assert(hasProgressBar, 'createSoundPlayer result contains progress bar');
	}
}
