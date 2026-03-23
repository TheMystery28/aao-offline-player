"use strict";
/**
 * Event system regression tests (EXHAUSTIVE).
 */
function testEvents() {
	TestHarness.suite('Events');

	// Function existence
	TestHarness.assertType(registerEventHandler, 'function', 'registerEventHandler is a function');
	TestHarness.assertType(unregisterEventHandler, 'function', 'unregisterEventHandler is a function');
	TestHarness.assertType(unregisterEvent, 'function', 'unregisterEvent is a function');
	TestHarness.assertType(unregisterAllEvents, 'function', 'unregisterAllEvents is a function');
	TestHarness.assertType(triggerEvent, 'function', 'triggerEvent is a function');
	TestHarness.assertType(hasDefaultFocusEvent, 'function', 'hasDefaultFocusEvent is a function');
	TestHarness.assertType(getFocusParent, 'function', 'getFocusParent is a function');

	// Register handler on element → trigger fires it with correct event object
	var testDiv = document.createElement('div');
	var handlerCalled = false;
	var receivedEvent = null;
	registerEventHandler(testDiv, 'testclick', function(e) {
		handlerCalled = true;
		receivedEvent = e;
	}, false);
	triggerEvent(testDiv, 'testclick');
	TestHarness.assert(handlerCalled, 'Register handler → trigger fires it');
	TestHarness.assert(receivedEvent instanceof Event, 'Trigger passes correct event object');

	// Register handler → unregister by index → trigger does NOT fire it
	var testDiv2 = document.createElement('div');
	var callCount = 0;
	var idx = registerEventHandler(testDiv2, 'testev', function() { callCount++; }, false);
	unregisterEventHandler(testDiv2, 'testev', idx);
	triggerEvent(testDiv2, 'testev');
	TestHarness.assertEqual(callCount, 0, 'Unregistered handler does not fire on trigger');

	// Register multiple handlers on same event → trigger fires all in order
	var testDiv3 = document.createElement('div');
	var order = [];
	registerEventHandler(testDiv3, 'multi', function() { order.push(1); }, false);
	registerEventHandler(testDiv3, 'multi', function() { order.push(2); }, false);
	triggerEvent(testDiv3, 'multi');
	TestHarness.assert(order.length === 2 && order[0] === 1 && order[1] === 2, 'Multiple handlers fire in registration order');

	// unregisterEvent removes ALL handlers for that event name
	var testDiv4 = document.createElement('div');
	var multiCallCount = 0;
	registerEventHandler(testDiv4, 'bulkev', function() { multiCallCount++; }, false);
	registerEventHandler(testDiv4, 'bulkev', function() { multiCallCount++; }, false);
	unregisterEvent(testDiv4, 'bulkev');
	triggerEvent(testDiv4, 'bulkev');
	TestHarness.assertEqual(multiCallCount, 0, 'unregisterEvent removes ALL handlers for event');

	// unregisterAllEvents removes handlers for ALL events on element
	var testDiv5 = document.createElement('div');
	var allCount = 0;
	registerEventHandler(testDiv5, 'ev1', function() { allCount++; }, false);
	registerEventHandler(testDiv5, 'ev2', function() { allCount++; }, false);
	unregisterAllEvents(testDiv5);
	triggerEvent(testDiv5, 'ev1');
	triggerEvent(testDiv5, 'ev2');
	TestHarness.assertEqual(allCount, 0, 'unregisterAllEvents removes handlers for ALL events');

	// triggerEvent on element with no handlers does not crash
	var testDiv6 = document.createElement('div');
	var noCrash = true;
	try {
		triggerEvent(testDiv6, 'nonexistent');
	} catch (e) {
		noCrash = false;
	}
	TestHarness.assert(noCrash, 'triggerEvent on element with no handlers does not crash');

	// triggerEvent skips null entries (left by unregister)
	var testDiv7 = document.createElement('div');
	var skipCount = 0;
	var idx0 = registerEventHandler(testDiv7, 'skiptest', function() { skipCount++; }, false);
	registerEventHandler(testDiv7, 'skiptest', function() { skipCount++; }, false);
	unregisterEventHandler(testDiv7, 'skiptest', idx0);
	triggerEvent(testDiv7, 'skiptest');
	TestHarness.assertEqual(skipCount, 1, 'triggerEvent skips null entries left by unregister');

	// hasDefaultFocusEvent tests
	var selectEl = document.createElement('select');
	var inputEl = document.createElement('input');
	var buttonEl = document.createElement('button');
	var optionEl = document.createElement('option');
	TestHarness.assert(hasDefaultFocusEvent(selectEl), 'hasDefaultFocusEvent returns true for <select>');
	TestHarness.assert(hasDefaultFocusEvent(inputEl), 'hasDefaultFocusEvent returns true for <input>');
	TestHarness.assert(hasDefaultFocusEvent(buttonEl), 'hasDefaultFocusEvent returns true for <button>');
	TestHarness.assert(hasDefaultFocusEvent(optionEl), 'hasDefaultFocusEvent returns true for <option>');

	var selectClassDiv = document.createElement('div');
	selectClassDiv.className = 'select';
	TestHarness.assert(hasDefaultFocusEvent(selectClassDiv), 'hasDefaultFocusEvent returns true for elements with class "select"');

	var plainDiv = document.createElement('div');
	var spanEl = document.createElement('span');
	TestHarness.assert(!hasDefaultFocusEvent(plainDiv), 'hasDefaultFocusEvent returns false for <div>');
	TestHarness.assert(!hasDefaultFocusEvent(spanEl), 'hasDefaultFocusEvent returns false for <span>');

	// Webkit mousedown focus fix is registered on document
	TestHarness.assert(
		document.event_handlers && document.event_handlers['mousedown'],
		'Webkit mousedown focus fix is registered on document'
	);
}
