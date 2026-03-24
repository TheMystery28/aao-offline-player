/**
 * Auto Night Mode Plugin
 * Automatically enables night mode based on system prefers-color-scheme.
 */
EnginePlugins.register({
	name: 'night_mode_auto',
	version: '1.0',
	init: function(config, events, api) {
		if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
			config.set('display.nightMode', true);
		}
		window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function(e) {
			config.set('display.nightMode', e.matches);
		});
	}
});
