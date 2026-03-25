/**
 * Alt Nametags Plugin
 * Custom nametag font via CSS injection.
 * Supports configurable font and size via params.
 */
EnginePlugins.register({
	name: 'alt_nametags',
	version: '1.1',
	params: {
		font: { type: 'text', default: 'Courier New, monospace', label: 'Font Family' },
		size: { type: 'number', default: 11, label: 'Font Size (px)', min: 6, max: 30 }
	},
	init: function(config, events, api) {
		var params = config.getPluginParams('alt_nametags');
		var style = api.dom.injectCSS(
			'div.textbox .name { font-family: ' + params.font + ' !important; font-size: ' + params.size + 'px !important; }'
		);
		return {
			destroy: function() { if (style && style.parentNode) style.parentNode.removeChild(style); }
		};
	}
});
