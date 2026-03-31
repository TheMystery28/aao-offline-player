/**
 * Backlog Plugin
 * Text history panel that captures dialogue via frame:willLeave event.
 * No monkey-patching of proceed() needed — uses the event bus.
 */
EnginePlugins.register({
	name: 'backlog',
	version: '1.0',
	init: function(config, events, api) {
		var log = [];
		var MAX_ENTRIES = 100;

		events.on('frame:willLeave', function(data) {
			if (data.dialogueText && data.dialogueText.trim() !== '') {
				if (log.length >= MAX_ENTRIES) log.shift();
				log.push({ name: data.speakerName, html: data.dialogueHTML });
				refreshLog();
			}
		});

		var contentEl = null;

		function refreshLog() {
			if (!contentEl) return;
			contentEl.innerHTML = '';
			for (var i = 0; i < log.length; i++) {
				var entry = log[i];
				if (entry.name) {
					contentEl.innerHTML += '<span class="backlog-name">' + entry.name + '</span><br/><div class="backlog-text">' + entry.html + '</div>';
				} else {
					contentEl.innerHTML += '<br/><div class="backlog-text">' + entry.html + '</div>';
				}
			}
			contentEl.scrollTop = contentEl.scrollHeight;
		}

		var meta = api.dom.query('#screen-meta');
		if (!meta) return;

		var btn = api.dom.create('button');
		btn.id = 'backlog-button';
		btn.textContent = 'Backlog';
		btn.style.cssText = 'position:absolute;top:-2px;left:-2px;z-index:5;font-size:9px;padding:1px 3px;';
		meta.appendChild(btn);

		var panel = api.dom.create('div');
		panel.id = 'backlog';
		panel.style.display = 'none';
		contentEl = api.dom.create('div');
		contentEl.id = 'backlog_content';
		panel.appendChild(contentEl);
		var screens = api.dom.query('#screens');
		if (screens) {
			screens.insertBefore(panel, api.dom.query('#screen-top'));
		}

		api.dom.injectCSS(
			'#backlog { position:absolute; z-index:999; width:256px; height:185px;' +
			'background:rgba(0,0,0,0.85); color:white; font:12px sans-serif;' +
			'border:2px ridge rgba(136,136,136,0.75); border-radius:3px; padding:2px; }' +
			'#backlog_content { overflow-y:auto; height:165px; padding:5px; }' +
			'.backlog-name { background:rgba(27,34,108,0.75); font-size:10px; padding:0 2px;' +
			'border:2px ridge rgba(136,136,136,0.75); border-radius:3px; }' +
			'.backlog-text { margin-bottom:4px; }'
		);

		btn.onclick = function() {
			panel.style.display = panel.style.display === 'none' ? 'block' : 'none';
			if (panel.style.display === 'block') {
				contentEl.scrollTop = contentEl.scrollHeight;
			}
		};

		// Manual destroy for raw DOM elements (CSS + events are auto-cleaned)
		return {
			destroy: function() {
				if (btn.parentNode) btn.parentNode.removeChild(btn);
				if (panel.parentNode) panel.parentNode.removeChild(panel);
			}
		};
	}
});
