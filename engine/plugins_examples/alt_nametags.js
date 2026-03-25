/**
 * Alt Nametags Plugin
 * Custom nametag font via CSS injection.
 */
EnginePlugins.register({
	name: 'alt_nametags',
	version: '1.0',
	init: function(config, events, api) {
		var style = api.dom.injectCSS(
			'div.textbox .name { font-family: "Courier New", monospace !important; font-size: 11px !important; }'
		);
		return {
			destroy: function() { if (style && style.parentNode) style.parentNode.removeChild(style); }
		};
	}
});
