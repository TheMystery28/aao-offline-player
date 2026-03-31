/**
 * Auto Night Mode Plugin
 * Automatically enables night mode based on system prefers-color-scheme.
 */
EnginePlugins.register({
	name: 'night_mode_auto',
	version: '1.1',
	init: function(config, events, api) {
		var mq = api.dom.onMediaQuery('(prefers-color-scheme: dark)', function(e) {
			config.set('display.nightMode', e.matches);
		});
		if (mq.matches) {
			config.set('display.nightMode', true);
		}
	}
});
