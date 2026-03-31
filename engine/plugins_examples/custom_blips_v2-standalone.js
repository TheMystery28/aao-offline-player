/**
 * Custom Voice Blips v2 (Standalone)
 * Replaces default voice blip sounds with custom audio files.
 * Assets are downloaded automatically when this plugin is attached to a case.
 *
 * @assets
 * voice_blip1.opus = https://BeyondTimeAxis.github.io/aao/contest/5/voice_blip1.opus
 * voice_blip2.opus = https://BeyondTimeAxis.github.io/aao/contest/5/voice_blip2.opus
 * voice_blip3.opus = https://BeyondTimeAxis.github.io/aao/contest/5/voice_blip3.opus
 */
EnginePlugins.register({
	name: 'custom_blips_v2',
	version: '2.0',
	init: function(config, events, api) {
		var trialInfo = api.player.getTrialInfo();
		if (!trialInfo) return;
		var baseUrl = 'case/' + trialInfo.id + '/plugins/assets/';
		for (var i = 1; i <= 3; i++) {
			var voiceId = 'voice_-' + i;
			var existing = api.sound.getSoundById(voiceId);
			if (existing) { api.sound.unloadSound(voiceId); }
			api.sound.registerSound(voiceId, {
				urls: [baseUrl + 'voice_blip' + i + '.opus'],
				loop: false,
				volume: 100
			});
		}
	}
});
