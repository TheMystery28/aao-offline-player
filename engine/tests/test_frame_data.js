"use strict";
/**
 * Frame data regression tests (EXHAUSTIVE).
 */
function testFrameData() {
	TestHarness.suite('Frame Data');

	// Function existence
	TestHarness.assertType(getObjectDescriptor, 'function', 'getObjectDescriptor is a function');
	TestHarness.assertType(getDefaultSpriteUrl, 'function', 'getDefaultSpriteUrl is a function');
	TestHarness.assertType(getPoseDesc, 'function', 'getPoseDesc is a function');
	TestHarness.assertType(getCharacterDescriptor, 'function', 'getCharacterDescriptor is a function');
	TestHarness.assertType(getPopupDescriptor, 'function', 'getPopupDescriptor is a function');
	TestHarness.assertType(getCharacterIndexById, 'function', 'getCharacterIndexById is a function');
	TestHarness.assertType(getSpeakerName, 'function', 'getSpeakerName is a function');
	TestHarness.assertType(getPlace, 'function', 'getPlace is a function');
	TestHarness.assertType(getPosition, 'function', 'getPosition is a function');
	TestHarness.assertType(getProfile, 'function', 'getProfile is a function');
	TestHarness.assertType(getProfileIconUrl, 'function', 'getProfileIconUrl is a function');
	TestHarness.assertType(getEvidenceIconUrl, 'function', 'getEvidenceIconUrl is a function');
	TestHarness.assertType(getPopupUrl, 'function', 'getPopupUrl is a function');
	TestHarness.assertType(getMusicUrl, 'function', 'getMusicUrl is a function');
	TestHarness.assertType(getSoundUrl, 'function', 'getSoundUrl is a function');
	TestHarness.assertType(getVoiceId, 'function', 'getVoiceId is a function');
	TestHarness.assertType(getVoiceUrl, 'function', 'getVoiceUrl is a function');
	TestHarness.assertType(getVoiceUrls, 'function', 'getVoiceUrls is a function');
	TestHarness.assertType(getVoiceDelay, 'function', 'getVoiceDelay is a function');

	// getObjectDescriptor with image field returns {uri: ...} with correct path
	var result1 = getObjectDescriptor({ image: 'test_img' }, 'bg_subdir');
	TestHarness.assert(
		result1.uri === cfg.picture_dir + cfg.bg_subdir + 'test_img.jpg',
		'getObjectDescriptor with image field returns {uri} with correct path'
	);

	// getObjectDescriptor with external image returns raw path (no prefix)
	var result2 = getObjectDescriptor({ image: 'http://example.com/img.png', external: true }, 'bg_subdir');
	TestHarness.assertEqual(
		result2.uri, 'http://example.com/img.png',
		'getObjectDescriptor with external image returns raw path'
	);

	// getObjectDescriptor with .gif image does not append .jpg
	var result3 = getObjectDescriptor({ image: 'anim.gif' }, 'bg_subdir');
	TestHarness.assert(
		result3.uri.indexOf('.jpg') === -1,
		'getObjectDescriptor with .gif image does not append .jpg'
	);

	// getObjectDescriptor with non-gif image appends .jpg
	var result4 = getObjectDescriptor({ image: 'photo' }, 'bg_subdir');
	TestHarness.assert(
		result4.uri.indexOf('.jpg') > -1,
		'getObjectDescriptor with non-gif image appends .jpg'
	);

	// getObjectDescriptor with subdir uses cfg[subdir]
	var result5 = getObjectDescriptor({ image: 'bg1' }, 'bg_subdir');
	TestHarness.assert(
		result5.uri.indexOf(cfg.bg_subdir) > -1,
		'getObjectDescriptor with subdir uses cfg[subdir]'
	);

	// getDefaultSpriteUrl builds correct path
	var spriteUrl = getDefaultSpriteUrl('Phoenix', 1, 'talking');
	TestHarness.assertEqual(
		spriteUrl,
		cfg.picture_dir + cfg.talking_subdir + 'Phoenix/1.gif',
		'getDefaultSpriteUrl builds cfg.picture_dir + cfg[status_subdir] + base + / + id + .gif'
	);

	// getDefaultSpriteUrl for still status
	var stillUrl = getDefaultSpriteUrl('Phoenix', 1, 'still');
	TestHarness.assert(
		stillUrl.indexOf(cfg.still_subdir) > -1,
		'getDefaultSpriteUrl for still status uses still_subdir'
	);

	// getDefaultSpriteUrl for startup status
	var startupUrl = getDefaultSpriteUrl('Phoenix', 1, 'startup');
	TestHarness.assert(
		startupUrl.indexOf(cfg.startup_subdir) > -1,
		'getDefaultSpriteUrl for startup status uses startup_subdir'
	);

	// getPoseDesc with sprite_id 0 returns empty strings
	var poseZero = getPoseDesc({ sprite_id: 0, profile_id: 1 });
	TestHarness.assertEqual(poseZero.talking, '', 'getPoseDesc with sprite_id 0 returns empty talking');
	TestHarness.assertEqual(poseZero.still, '', 'getPoseDesc with sprite_id 0 returns empty still');
	TestHarness.assertEqual(poseZero.startup, '', 'getPoseDesc with sprite_id 0 returns empty startup');

	// getPlace with negative id returns default_places entry
	if (typeof default_places !== 'undefined') {
		var defaultPlace = getPlace(-1);
		TestHarness.assertDefined(defaultPlace, 'getPlace with negative id returns default_places entry');
	}

	// getPosition with id <= 0 returns default_positions entry
	if (typeof default_positions !== 'undefined') {
		var defaultPos = getPosition(POSITION_CENTER);
		TestHarness.assertDefined(defaultPos, 'getPosition with POSITION_CENTER returns default_positions entry');
	}

	// getSpeakerName with speaker_use_name returns speaker_name field
	var nameFrame1 = { speaker_use_name: true, speaker_name: 'Custom Name', speaker_id: 1 };
	TestHarness.assertEqual(getSpeakerName(nameFrame1), 'Custom Name', 'getSpeakerName with speaker_use_name returns speaker_name field');

	// getSpeakerName with PROFILE_JUDGE returns localized judge name
	if (typeof l === 'function') {
		var judgeFrame = { speaker_id: PROFILE_JUDGE };
		var judgeName = getSpeakerName(judgeFrame, true);
		TestHarness.assertEqual(judgeName, l('profile_judge'), 'getSpeakerName with PROFILE_JUDGE returns localized judge name');
	}

	// getSpeakerName with PROFILE_UNKNOWN returns '???'
	var unknownFrame = { speaker_id: PROFILE_UNKNOWN };
	TestHarness.assertEqual(getSpeakerName(unknownFrame, true), '???', 'getSpeakerName with PROFILE_UNKNOWN returns ???');

	// getSpeakerName with 0 returns empty string (default case in switch)
	var zeroFrame = { speaker_id: 0 };
	// speaker_id 0 is PROFILE_JUDGE, which returns l('profile_judge'), not empty string
	// Actually PROFILE_JUDGE = 0, so this returns the judge name
	if (typeof l === 'function') {
		TestHarness.assertEqual(
			getSpeakerName(zeroFrame, true), l('profile_judge'),
			'getSpeakerName with speaker_id 0 (PROFILE_JUDGE) returns judge name'
		);
	}

	// getVoiceUrls returns array of exactly 3 URLs (opus, wav, mp3)
	var voiceUrls = getVoiceUrls(VOICE_MALE);
	TestHarness.assert(Array.isArray(voiceUrls), 'getVoiceUrls returns an array');
	TestHarness.assertEqual(voiceUrls.length, 3, 'getVoiceUrls returns exactly 3 URLs');
	TestHarness.assert(voiceUrls[0].indexOf('.opus') > -1, 'getVoiceUrls first URL is .opus');
	TestHarness.assert(voiceUrls[1].indexOf('.wav') > -1, 'getVoiceUrls second URL is .wav');
	TestHarness.assert(voiceUrls[2].indexOf('.mp3') > -1, 'getVoiceUrls third URL is .mp3');

	// getVoiceDelay returns correct delay for voice types
	TestHarness.assertEqual(getVoiceDelay(VOICE_MALE), VOICE_MALE_DELAY, 'getVoiceDelay returns correct delay for VOICE_MALE');
	TestHarness.assertEqual(getVoiceDelay(VOICE_FEMALE), VOICE_FEMALE_DELAY, 'getVoiceDelay returns correct delay for VOICE_FEMALE');
	TestHarness.assertEqual(getVoiceDelay(VOICE_TYPEWRITER), VOICE_TYPEWRITER_DELAY, 'getVoiceDelay returns correct delay for VOICE_TYPEWRITER');

	// getMusicUrl with external returns raw path
	TestHarness.assertEqual(
		getMusicUrl({ external: true, path: 'http://ext.com/song.mp3' }),
		'http://ext.com/song.mp3',
		'getMusicUrl with external returns raw path'
	);

	// getMusicUrl with non-external prepends cfg.music_dir and appends .mp3
	var musicResult = getMusicUrl({ external: false, path: 'bgm01' });
	TestHarness.assertEqual(musicResult, cfg.music_dir + 'bgm01.mp3', 'getMusicUrl with non-external prepends cfg.music_dir and appends .mp3');

	// getSoundUrl with external returns raw path
	TestHarness.assertEqual(
		getSoundUrl({ external: true, path: 'http://ext.com/sfx.mp3' }),
		'http://ext.com/sfx.mp3',
		'getSoundUrl with external returns raw path'
	);

	// getSoundUrl with non-external prepends cfg.sounds_dir and appends .mp3
	var soundResult = getSoundUrl({ external: false, path: 'sfx01' });
	TestHarness.assertEqual(soundResult, cfg.sounds_dir + 'sfx01.mp3', 'getSoundUrl with non-external prepends cfg.sounds_dir and appends .mp3');
}
