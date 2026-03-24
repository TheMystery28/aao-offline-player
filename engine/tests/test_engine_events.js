"use strict";
/**
 * EngineEvents (event bus) regression tests.
 */
function testEngineEvents() {
	TestHarness.suite('EngineEvents');

	// Module is loaded
	TestHarness.assertEqual(
		Modules.request_list['engine_events'], 3,
		'engine_events module is loaded (status 3)'
	);

	// EngineEvents global exists
	TestHarness.assertDefined(EngineEvents, 'EngineEvents global is defined');
	TestHarness.assertType(EngineEvents.on, 'function', 'EngineEvents.on is a function');
	TestHarness.assertType(EngineEvents.off, 'function', 'EngineEvents.off is a function');
	TestHarness.assertType(EngineEvents.emit, 'function', 'EngineEvents.emit is a function');
	TestHarness.assertType(EngineEvents.emitCancellable, 'function', 'EngineEvents.emitCancellable is a function');
	TestHarness.assertType(EngineEvents.clear, 'function', 'EngineEvents.clear is a function');

	// --- on + emit fires handler with data ---
	(function() {
		EngineEvents.clear();
		var received = null;
		EngineEvents.on('test:basic', function(data) { received = data; });
		EngineEvents.emit('test:basic', { value: 42 });
		TestHarness.assert(received !== null, 'on + emit fires handler');
		TestHarness.assertEqual(received.value, 42, 'on + emit passes data payload');
		EngineEvents.clear();
	})();

	// --- off removes handler ---
	(function() {
		EngineEvents.clear();
		var count = 0;
		var handler = function() { count++; };
		EngineEvents.on('test:off', handler);
		EngineEvents.emit('test:off');
		TestHarness.assertEqual(count, 1, 'handler fires before off()');
		EngineEvents.off('test:off', handler);
		EngineEvents.emit('test:off');
		TestHarness.assertEqual(count, 1, 'off() removes handler — not fired again');
		EngineEvents.clear();
	})();

	// --- Multiple handlers fire in registration order (same priority) ---
	(function() {
		EngineEvents.clear();
		var order = [];
		EngineEvents.on('test:multi', function() { order.push('a'); });
		EngineEvents.on('test:multi', function() { order.push('b'); });
		EngineEvents.on('test:multi', function() { order.push('c'); });
		EngineEvents.emit('test:multi');
		TestHarness.assertEqual(order.join(','), 'a,b,c', 'Multiple handlers fire in registration order');
		EngineEvents.clear();
	})();

	// --- Priority ordering works ---
	(function() {
		EngineEvents.clear();
		var order = [];
		EngineEvents.on('test:priority', function() { order.push('low'); }, 10);
		EngineEvents.on('test:priority', function() { order.push('high'); }, -5);
		EngineEvents.on('test:priority', function() { order.push('mid'); }, 0);
		EngineEvents.emit('test:priority');
		TestHarness.assertEqual(order.join(','), 'high,mid,low', 'Priority ordering: lower number fires first');
		EngineEvents.clear();
	})();

	// --- emitCancellable + preventDefault stops propagation ---
	(function() {
		EngineEvents.clear();
		var reached = [];
		EngineEvents.on('test:cancel', function(data) {
			reached.push('first');
			data.preventDefault();
		});
		EngineEvents.on('test:cancel', function() {
			reached.push('second');
		});
		var result = EngineEvents.emitCancellable('test:cancel', {});
		TestHarness.assert(result.cancelled, 'emitCancellable returns cancelled: true after preventDefault');
		TestHarness.assertEqual(reached.join(','), 'first', 'preventDefault stops propagation to second handler');
		EngineEvents.clear();
	})();

	// --- emitCancellable without preventDefault returns cancelled: false ---
	(function() {
		EngineEvents.clear();
		EngineEvents.on('test:nocancel', function() {});
		var result = EngineEvents.emitCancellable('test:nocancel', {});
		TestHarness.assert(!result.cancelled, 'emitCancellable returns cancelled: false when not prevented');
		EngineEvents.clear();
	})();

	// --- No listeners = no crash ---
	(function() {
		EngineEvents.clear();
		var noCrash = true;
		try {
			EngineEvents.emit('test:nonexistent', { foo: 1 });
		} catch (e) {
			noCrash = false;
		}
		TestHarness.assert(noCrash, 'emit with no listeners does not crash');
	})();

	(function() {
		var noCrash = true;
		try {
			EngineEvents.emitCancellable('test:nonexistent2');
		} catch (e) {
			noCrash = false;
		}
		TestHarness.assert(noCrash, 'emitCancellable with no listeners does not crash');
		EngineEvents.clear();
	})();

	// --- off on non-existent event does not crash ---
	(function() {
		var noCrash = true;
		try {
			EngineEvents.off('test:never_registered', function() {});
		} catch (e) {
			noCrash = false;
		}
		TestHarness.assert(noCrash, 'off() on non-existent event does not crash');
	})();

	// --- clear removes all listeners ---
	(function() {
		var count = 0;
		EngineEvents.on('test:clear', function() { count++; });
		EngineEvents.clear();
		EngineEvents.emit('test:clear');
		TestHarness.assertEqual(count, 0, 'clear() removes all listeners');
	})();

	// --- Namespace tests ---

	// on() with no namespace has no namespace tag (not 'engine')
	(function() {
		EngineEvents.clear();
		var fired = false;
		EngineEvents.on('test:ns1', function() { fired = true; });
		EngineEvents.emit('test:ns1');
		TestHarness.assert(fired, 'Namespace: handler with no explicit namespace fires');
		EngineEvents.clear();
	})();

	// on() with 'engine' namespace — protected from clear()
	(function() {
		EngineEvents.clear();
		var engineFired = false;
		var regularFired = false;
		EngineEvents.on('test:ns2', function() { engineFired = true; }, 0, 'engine');
		EngineEvents.on('test:ns2', function() { regularFired = true; });
		EngineEvents.clear(); // Should remove regular, keep engine
		engineFired = false;
		regularFired = false;
		EngineEvents.emit('test:ns2');
		TestHarness.assert(engineFired, 'Namespace: engine listener survives clear()');
		TestHarness.assert(!regularFired, 'Namespace: regular listener removed by clear()');
		// Cleanup engine listeners manually
		EngineEvents.clear(); // This won't remove engine — use a temp approach
	})();

	// clearNamespace removes only that namespace
	(function() {
		EngineEvents.clear();
		var aFired = false, bFired = false;
		EngineEvents.on('test:ns3', function() { aFired = true; }, 0, 'pluginA');
		EngineEvents.on('test:ns3', function() { bFired = true; }, 0, 'pluginB');
		if (typeof EngineEvents.clearNamespace === 'function') {
			EngineEvents.clearNamespace('pluginA');
			EngineEvents.emit('test:ns3');
			TestHarness.assert(!aFired, 'clearNamespace: pluginA listener removed');
			TestHarness.assert(bFired, 'clearNamespace: pluginB listener preserved');
		} else {
			TestHarness.assert(false, 'clearNamespace: method not yet implemented');
		}
		EngineEvents.clear();
	})();

	// clearNamespace('engine') is a no-op
	(function() {
		EngineEvents.clear();
		var fired = false;
		EngineEvents.on('test:ns4', function() { fired = true; }, 0, 'engine');
		if (typeof EngineEvents.clearNamespace === 'function') {
			EngineEvents.clearNamespace('engine');
			EngineEvents.emit('test:ns4');
			TestHarness.assert(fired, 'clearNamespace(engine) is a no-op — listener still fires');
		}
		EngineEvents.clear();
	})();

	// emit() with throwing handler doesn't prevent other handlers
	(function() {
		EngineEvents.clear();
		var secondFired = false;
		EngineEvents.on('test:throw', function() { throw new Error('intentional'); });
		EngineEvents.on('test:throw', function() { secondFired = true; });
		EngineEvents.emit('test:throw');
		TestHarness.assert(secondFired, 'emit: throwing handler does not prevent subsequent handlers');
		EngineEvents.clear();
	})();

	// emitCancellable() with throwing handler doesn't crash
	(function() {
		EngineEvents.clear();
		var noCrash = true;
		EngineEvents.on('test:throwCancel', function() { throw new Error('intentional'); });
		try {
			EngineEvents.emitCancellable('test:throwCancel', {});
		} catch (e) {
			noCrash = false;
		}
		TestHarness.assert(noCrash, 'emitCancellable: throwing handler does not crash');
		EngineEvents.clear();
	})();

	// Priority ordering works across namespaces
	(function() {
		EngineEvents.clear();
		var order = [];
		EngineEvents.on('test:nsPriority', function() { order.push('engine'); }, 0, 'engine');
		EngineEvents.on('test:nsPriority', function() { order.push('plugin'); }, 0, 'pluginA');
		EngineEvents.on('test:nsPriority', function() { order.push('early'); }, -5, 'pluginB');
		EngineEvents.emit('test:nsPriority');
		TestHarness.assertEqual(order[0], 'early', 'Priority across namespaces: -5 fires first');
		EngineEvents.clear();
	})();

	// off() works by function reference regardless of namespace
	(function() {
		EngineEvents.clear();
		var count = 0;
		var handler = function() { count++; };
		EngineEvents.on('test:nsOff', handler, 0, 'pluginA');
		EngineEvents.emit('test:nsOff');
		TestHarness.assertEqual(count, 1, 'Namespace off: handler fires before off');
		EngineEvents.off('test:nsOff', handler);
		EngineEvents.emit('test:nsOff');
		TestHarness.assertEqual(count, 1, 'Namespace off: handler removed by off()');
		EngineEvents.clear();
	})();

	// Final cleanup
	EngineEvents.clear();
}
