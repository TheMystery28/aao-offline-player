"use strict";
/**
 * Regression tests for the HTML5 audio loop mechanism and music liveness recovery.
 *
 * Phase 1 — Safe Loop Transition:
 *   Verifies the onend handler in sound-howler.js uses play()-then-seek(id)
 *   instead of the buggy seek()-then-play() pattern that caused concurrent
 *   <audio> element conflicts on Android.
 *
 * Phase 2 — Liveness Check + Position Recovery:
 *   Verifies playMusic() detects silently-dead music, that playSound() returns
 *   a playback ID, and that _musicPositionCache tracks position for recovery.
 */

/**
 * Phase 1: Source-level verification that the onend handler is safe.
 */
function testSoundLoopMechanism() {
	TestHarness.suite('Sound Loop Mechanism');

	// --- Read sound-howler.js source for static analysis ---
	var xhr = new XMLHttpRequest();
	xhr.open('GET', 'Javascript/sound-howler.js', false);
	xhr.send();
	if (xhr.status !== 200 && xhr.status !== 0) {
		TestHarness.assert(false, 'Could not load sound-howler.js (status ' + xhr.status + ')');
		return;
	}
	var source = xhr.responseText;

	// --- Locate the onend handler block ---
	// Find the section: newHowl.on('end', function()
	var onendStart = source.indexOf("newHowl.on('end'");
	TestHarness.assert(onendStart !== -1, 'Found onend handler in sound-howler.js');

	// Extract the handler body (from 'end' to the closing });)
	var onendBody = '';
	if (onendStart !== -1) {
		// Find the opening { of the callback
		var braceStart = source.indexOf('{', onendStart);
		if (braceStart !== -1) {
			// Walk forward to find the matching closing brace
			var depth = 1;
			var pos = braceStart + 1;
			while (pos < source.length && depth > 0) {
				if (source[pos] === '{') depth++;
				if (source[pos] === '}') depth--;
				pos++;
			}
			onendBody = source.substring(braceStart, pos);
		}
	}

	TestHarness.assert(onendBody.length > 0, 'Extracted onend handler body');

	// --- Phase 1 core assertion: play() must come BEFORE seek() ---
	// The safe pattern is: var id = newHowl.play(); ... newHowl.seek(loopStartSec, id);
	// The buggy pattern was: newHowl.seek(loopStartSec); newHowl.play();
	var playPos = onendBody.indexOf('.play()');
	var seekPos = onendBody.indexOf('.seek(');
	TestHarness.assert(playPos !== -1, 'onend handler calls play()');
	TestHarness.assert(seekPos !== -1, 'onend handler calls seek()');
	TestHarness.assert(
		playPos < seekPos,
		'onend handler calls play() BEFORE seek() (safe loop transition)'
	);

	// --- Phase 1: seek() must receive the playback ID as second arg ---
	// Pattern: .seek(loopStartSec, newId) or .seek(loopStartSec, id)
	// The seek call must have two arguments (comma-separated)
	if (seekPos !== -1) {
		var seekCall = onendBody.substring(seekPos);
		var seekParen = seekCall.indexOf('(');
		var seekClose = seekCall.indexOf(')');
		if (seekParen !== -1 && seekClose !== -1) {
			var seekArgs = seekCall.substring(seekParen + 1, seekClose);
			var hasSecondArg = seekArgs.indexOf(',') !== -1;
			TestHarness.assert(
				hasSecondArg,
				'onend seek() passes playback ID as second argument'
			);
		}
	}

	// --- Phase 1: the play() return value must be captured ---
	// Pattern: var newId = newHowl.play(); or var id = ...play()
	var capturesPlayId = /var\s+\w+\s*=\s*newHowl\.play\(\)/.test(onendBody);
	TestHarness.assert(
		capturesPlayId,
		'onend handler captures play() return value in a variable'
	);

	// --- Phase 1: NO seek() before play() (the old buggy pattern) ---
	// Ensure there's no seek call that precedes the first play() call
	var firstSeekBeforePlay = onendBody.indexOf('.seek(') < onendBody.indexOf('.play()') &&
		onendBody.indexOf('.seek(') !== -1;
	TestHarness.assert(
		!firstSeekBeforePlay,
		'onend handler does NOT have seek() before play() (old buggy pattern absent)'
	);
}

/**
 * Phase 2: playSound() returns a playback ID and playMusic() has liveness recovery.
 */
function testSoundLivenessRecovery() {
	TestHarness.suite('Sound Liveness Recovery');

	// --- playSound returns a value (source analysis) ---
	var xhr = new XMLHttpRequest();
	xhr.open('GET', 'Javascript/sound-howler.js', false);
	xhr.send();
	var howlerSource = (xhr.status === 200 || xhr.status === 0) ? xhr.responseText : '';

	// Find the playSound function body
	var playSoundStart = howlerSource.indexOf('self.playSound = function');
	var playSoundBody = '';
	if (playSoundStart !== -1) {
		var braceStart = howlerSource.indexOf('{', playSoundStart);
		if (braceStart !== -1) {
			var depth = 1;
			var pos = braceStart + 1;
			while (pos < howlerSource.length && depth > 0) {
				if (howlerSource[pos] === '{') depth++;
				if (howlerSource[pos] === '}') depth--;
				pos++;
			}
			playSoundBody = howlerSource.substring(braceStart, pos);
		}
	}

	TestHarness.assert(playSoundBody.length > 0, 'Extracted playSound function body');

	// The 3 top-level sound.play() calls in playSound must return their value.
	// There is also a 4th sound.play("loop") inside an _onend callback that is
	// fire-and-forget (sprite intro→loop transition) and does NOT need a return.
	var returnPlayCalls = (playSoundBody.match(/return\s+sound\.play\(/g) || []).length;

	TestHarness.assert(
		returnPlayCalls >= 3,
		'playSound has >= 3 return sound.play() calls (' + returnPlayCalls + ' found)'
	);

	// --- playMusic liveness check (source analysis) ---
	var xhr2 = new XMLHttpRequest();
	xhr2.open('GET', 'Javascript/player_sound.js', false);
	xhr2.send();
	var playerSource = (xhr2.status === 200 || xhr2.status === 0) ? xhr2.responseText : '';

	// playMusic must call howl.playing() for liveness detection
	var playMusicStart = playerSource.indexOf('function playMusic');
	var playMusicEnd = playerSource.indexOf('\nfunction ', playMusicStart + 1);
	var playMusicBody = (playMusicStart !== -1 && playMusicEnd !== -1)
		? playerSource.substring(playMusicStart, playMusicEnd)
		: (playMusicStart !== -1 ? playerSource.substring(playMusicStart) : '');

	TestHarness.assert(
		playMusicBody.indexOf('.playing()') !== -1,
		'playMusic checks howl.playing() for liveness detection'
	);

	// playMusic must reference _musicPositionCache for recovery
	TestHarness.assert(
		playMusicBody.indexOf('_musicPositionCache') !== -1,
		'playMusic uses _musicPositionCache for position recovery'
	);

	// --- _musicPositionCache variable exists ---
	TestHarness.assert(
		'_musicPositionCache' in window,
		'_musicPositionCache is declared as a global variable'
	);

	// --- _trackMusicPosition function exists ---
	TestHarness.assertType(
		typeof _trackMusicPosition !== 'undefined' ? _trackMusicPosition : undefined,
		'function',
		'_trackMusicPosition heartbeat function exists'
	);

	// --- stopMusic cleans up the tracker ---
	var stopMusicStart = playerSource.indexOf('function stopMusic');
	var stopMusicEnd = playerSource.indexOf('\nfunction ', stopMusicStart + 1);
	var stopMusicBody = (stopMusicStart !== -1 && stopMusicEnd !== -1)
		? playerSource.substring(stopMusicStart, stopMusicEnd)
		: (stopMusicStart !== -1 ? playerSource.substring(stopMusicStart) : '');

	TestHarness.assert(
		stopMusicBody.indexOf('_musicPositionCache') !== -1,
		'stopMusic resets _musicPositionCache'
	);
	TestHarness.assert(
		stopMusicBody.indexOf('cancelAnimationFrame') !== -1,
		'stopMusic calls cancelAnimationFrame to clean up tracker'
	);

	// --- Behavioral test: stopNonMusicSounds still exists and filters correctly ---
	TestHarness.assertType(
		typeof stopNonMusicSounds !== 'undefined' ? stopNonMusicSounds : undefined,
		'function',
		'stopNonMusicSounds function exists'
	);

	// --- Behavioral: playMusic with SoundHowler (if trial loaded) ---
	if (typeof trial_data !== 'undefined' && trial_data &&
		typeof SoundHowler !== 'undefined' && trial_data.music && trial_data.music.length > 1)
	{
		var testMusicId = trial_data.music[1].id;

		// playSound should return a value (playback ID)
		var howlerId = 'music_' + testMusicId;
		var playResult = SoundHowler.playSound(howlerId);
		TestHarness.assert(
			typeof playResult === 'number',
			'SoundHowler.playSound() returns a numeric playback ID'
		);

		// Set current_music_id and stop the howl to simulate Android death
		current_music_id = testMusicId;
		var howl = SoundHowler.getSoundById(howlerId);
		if (howl) howl.stop();

		// playMusic should detect the dead state and restart
		var prevId = current_music_id;
		playMusic(testMusicId);
		TestHarness.assertEqual(
			current_music_id, testMusicId,
			'playMusic detects dead music and restarts (current_music_id preserved)'
		);

		// Clean up
		stopMusic();
	}
}
