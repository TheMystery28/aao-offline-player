/**
 * Number Key Options Plugin
 * Press 1-9 to select matching option links during dialogue choices.
 */
EnginePlugins.register({
	name: 'number_key_options',
	version: '1.0',
	init: function(config, events, api) {
		api.input.onKeyDown(function(e) {
			var digit = null;
			if (e.keyCode >= 49 && e.keyCode <= 57) digit = e.keyCode - 48;
			else if (e.keyCode >= 97 && e.keyCode <= 105) digit = e.keyCode - 96;
			if (digit === null) return;

			var options = api.dom.queryAll('#options a');
			for (var i = 0; i < options.length; i++) {
				if (options[i].textContent.indexOf(digit + '.') === 0) {
					var style = getComputedStyle(options[i]);
					if (style.display !== 'none' && style.visibility !== 'hidden') {
						options[i].click();
						break;
					}
				}
			}
		});
	}
});
