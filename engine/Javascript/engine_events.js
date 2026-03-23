"use strict";
/*
Ace Attorney Online - Engine Event Bus

Pub/sub event bus for decoupling engine modules.
ES2017 max — no import/export, no ES2018+ features.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'engine_events',
	dependencies : [],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

var EngineEvents = (function() {
	// Map<string, Array<{handler: Function, priority: number}>>
	const listeners = new Map();

	function getList(event) {
		if (!listeners.has(event)) {
			listeners.set(event, []);
		}
		return listeners.get(event);
	}

	return {
		/**
		 * Register a listener for an event.
		 * @param {string} event - Event name (e.g. 'frame:before')
		 * @param {Function} handler - Callback receiving (data)
		 * @param {number} [priority=0] - Lower runs first
		 */
		on: function(event, handler, priority) {
			if (typeof priority === 'undefined') {
				priority = 0;
			}
			const list = getList(event);
			list.push({ handler: handler, priority: priority });
			// Sort ascending by priority so lower numbers fire first
			list.sort(function(a, b) { return a.priority - b.priority; });
		},

		/**
		 * Remove a previously registered listener.
		 * @param {string} event - Event name
		 * @param {Function} handler - The exact function reference passed to on()
		 */
		off: function(event, handler) {
			if (!listeners.has(event)) return;
			const list = listeners.get(event);
			for (let i = list.length - 1; i >= 0; i--) {
				if (list[i].handler === handler) {
					list.splice(i, 1);
					break;
				}
			}
		},

		/**
		 * Emit an event, calling all registered listeners in priority order.
		 * @param {string} event - Event name
		 * @param {*} [data] - Payload passed to each handler
		 */
		emit: function(event, data) {
			if (!listeners.has(event)) return;
			const list = listeners.get(event);
			for (let i = 0; i < list.length; i++) {
				list[i].handler(data);
			}
		},

		/**
		 * Emit a cancellable event. Handlers can call data.preventDefault()
		 * to signal cancellation.
		 * @param {string} event - Event name
		 * @param {*} [data] - Payload; a preventDefault() method is added
		 * @returns {{cancelled: boolean}}
		 */
		emitCancellable: function(event, data) {
			let cancelled = false;
			if (typeof data === 'undefined' || data === null) {
				data = {};
			}
			data.preventDefault = function() { cancelled = true; };

			if (listeners.has(event)) {
				const list = listeners.get(event);
				for (let i = 0; i < list.length; i++) {
					list[i].handler(data);
					if (cancelled) break;
				}
			}

			return { cancelled: cancelled };
		},

		/**
		 * Remove all listeners (useful for testing teardown).
		 */
		clear: function() {
			listeners.clear();
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('engine_events');
