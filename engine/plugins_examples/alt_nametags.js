/**
 * Alt Nametags Plugin
 * Custom nametag font via CSS injection.
 */
EnginePlugins.register({
	name: 'alt_nametags',
	version: '1.0',
	init: function(config, events, api) {
		api.dom.injectCSS('div.textbox .name { font: 9px aaDialogueText, sans-serif; line-height: 1; }');
	}
});
