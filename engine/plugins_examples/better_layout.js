/**
 * Better Layout Plugin
 * Adds custom settings section with layout options.
 * Demonstrates api.settings.addSection with config persistence.
 */
EnginePlugins.register({
	name: 'better_layout',
	version: '1.0',
	init: function(config, events, api) {
		api.settings.addSection('Custom Layout', [
			{ type: 'checkbox', key: 'plugins.betterLayout.zoom2x', label: '2x Screen Size' },
			{ type: 'checkbox', key: 'plugins.betterLayout.hideLifebar', label: 'Hide Life Bar' },
			{ type: 'slider', key: 'plugins.betterLayout.dialogueOpacity', label: 'Dialogue Opacity', min: 0.3, max: 1.0, step: 0.1 }
		]);

		var styleEl = null;
		function updateStyles() {
			var css = '';
			if (config.get('plugins.betterLayout.zoom2x')) {
				css += '#screens { zoom: 2 !important; }\n';
			}
			if (config.get('plugins.betterLayout.hideLifebar')) {
				css += '#lifebar { display: none !important; }\n';
			}
			var opacity = config.get('plugins.betterLayout.dialogueOpacity');
			if (opacity !== undefined && opacity < 1) {
				css += '.textbox { opacity: ' + opacity + ' !important; }\n';
			}
			if (styleEl) styleEl.parentNode.removeChild(styleEl);
			styleEl = api.dom.injectCSS(css);
		}

		events.on('config:changed', function(data) {
			if (data.path && data.path.indexOf('plugins.betterLayout.') === 0) {
				updateStyles();
			}
		}, 0, 'better_layout');
		updateStyles();

		return {
			destroy: function() {
				if (styleEl && styleEl.parentNode) styleEl.parentNode.removeChild(styleEl);
				events.clearNamespace('better_layout');
				api.settings.removeSection('Custom Layout');
			}
		};
	}
});
