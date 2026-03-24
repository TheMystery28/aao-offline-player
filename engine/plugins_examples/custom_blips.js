/**
 * Custom Voice Blips Plugin
 * Replaces default voice blip sounds with case-bundled audio files.
 * Expects assets in case/{id}/plugins/assets/voice_blip1.opus, etc.
 */
EnginePlugins.register({
	name: 'custom_blips',
	version: '1.0',
	init: function(config, events, api) {
		var trialInfo = api.player.getTrialInfo();
		if (!trialInfo) return;
		var baseUrl = 'case/' + trialInfo.id + '/plugins/assets/';

		for (var i = 1; i <= 3; i++) {
			var voiceId = 'voice_-' + i;
			var existing = api.sound.getSoundById(voiceId);
			if (existing) {
				api.sound.unloadSound(voiceId);
			}
			api.sound.registerSound(voiceId, {
				urls: [baseUrl + 'voice_blip' + i + '.opus', baseUrl + 'voice_blip' + i + '.wav'],
				loop: false,
				volume: 100
			});
		}
	}
});
