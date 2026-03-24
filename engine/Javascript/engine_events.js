"use strict";
/*
Ace Attorney Online - Engine Event Bus

Pub/sub event bus for decoupling engine modules.
Supports namespaced listeners — 'engine' namespace is protected from clear().
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
	// Map<string, Array<{handler: Function, priority: number, namespace: string|undefined}>>
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
		 * @param {string} [namespace] - Listener namespace. 'engine' is protected from clear().
		 */
		on: function(event, handler, priority, namespace) {
			if (typeof priority === 'undefined') {
				priority = 0;
			}
			const list = getList(event);
			list.push({ handler: handler, priority: priority, namespace: namespace });
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
		 * Errors in individual handlers are caught and logged — one handler
		 * crashing does not prevent others from firing.
		 * @param {string} event - Event name
		 * @param {*} [data] - Payload passed to each handler
		 */
		emit: function(event, data) {
			if (!listeners.has(event)) return;
			const list = listeners.get(event);
			for (let i = 0; i < list.length; i++) {
				try {
					list[i].handler(data);
				} catch (e) {
					console.error('[EngineEvents] Handler error in ' + event +
						(list[i].namespace ? ' (' + list[i].namespace + ')' : '') + ':', e);
				}
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
					try {
						list[i].handler(data);
					} catch (e) {
						console.error('[EngineEvents] Handler error in ' + event +
							(list[i].namespace ? ' (' + list[i].namespace + ')' : '') + ':', e);
					}
					if (cancelled) break;
				}
			}

			return { cancelled: cancelled };
		},

		/**
		 * Remove all listeners EXCEPT those registered with the 'engine' namespace.
		 * Engine-core listeners are protected and survive clear().
		 */
		clear: function() {
			const keys = Array.from(listeners.keys());
			for (let i = 0; i < keys.length; i++) {
				const list = listeners.get(keys[i]);
				const kept = list.filter(function(entry) {
					return entry.namespace === 'engine';
				});
				if (kept.length > 0) {
					listeners.set(keys[i], kept);
				} else {
					listeners.delete(keys[i]);
				}
			}
		},

		/**
		 * Remove all listeners for a specific namespace.
		 * Cannot clear the 'engine' namespace (no-op).
		 * @param {string} namespace - The namespace to clear
		 */
		clearNamespace: function(namespace) {
			if (!namespace || namespace === 'engine') return;
			const keys = Array.from(listeners.keys());
			for (let i = 0; i < keys.length; i++) {
				const list = listeners.get(keys[i]);
				const kept = list.filter(function(entry) {
					return entry.namespace !== namespace;
				});
				if (kept.length > 0) {
					listeners.set(keys[i], kept);
				} else {
					listeners.delete(keys[i]);
				}
			}
		}
	};
})();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('engine_events');
