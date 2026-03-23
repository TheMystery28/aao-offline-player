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

	// Final cleanup
	EngineEvents.clear();
}
