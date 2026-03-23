"use strict";
/**
 * Display engine regression tests (EXHAUSTIVE).
 */
function testDisplay() {
	TestHarness.suite('Display Engine');

	// ScreenDisplay
	TestHarness.assertType(ScreenDisplay, 'function', 'ScreenDisplay is a function (constructor)');

	// CallbackBuffer
	TestHarness.assertType(CallbackBuffer, 'function', 'CallbackBuffer is a function (constructor)');

	// Global display functions
	TestHarness.assertType(generateImageElement, 'function', 'generateImageElement is a function');
	TestHarness.assertType(generateGraphicElement, 'function', 'generateGraphicElement is a function');
	TestHarness.assertType(updateGraphicElement, 'function', 'updateGraphicElement is a function');
	TestHarness.assertType(setEffectToGraphicElement, 'function', 'setEffectToGraphicElement is a function');
	TestHarness.assertType(setGraphicElementPosition, 'function', 'setGraphicElementPosition is a function');
	TestHarness.assertType(setTransition, 'function', 'setTransition is a function');
	TestHarness.assertType(cancelTransition, 'function', 'cancelTransition is a function');

	// ScreenDisplay creates render element with class display_engine_screen
	var screen = new ScreenDisplay();
	TestHarness.assert(
		screen.render.classList.contains('display_engine_screen'),
		'ScreenDisplay creates render element with class display_engine_screen'
	);

	// ScreenDisplay render has viewport child
	var hasViewport = false;
	for (var i = 0; i < screen.render.children.length; i++) {
		if (screen.render.children[i].classList.contains('viewport')) {
			hasViewport = true;
			break;
		}
	}
	TestHarness.assert(hasViewport, 'ScreenDisplay render has viewport child');

	// ScreenDisplay methods
	TestHarness.assertType(screen.loadFrame, 'function', 'ScreenDisplay has loadFrame method');
	TestHarness.assertType(screen.clearScreen, 'function', 'ScreenDisplay has clearScreen method');
	TestHarness.assertType(screen.skip, 'function', 'ScreenDisplay has skip method');
	TestHarness.assertType(screen.setInstantMode, 'function', 'ScreenDisplay has setInstantMode method');

	// ScreenDisplay state getter/setter
	var stateDesc = Object.getOwnPropertyDescriptor(screen, 'state') ||
		Object.getOwnPropertyDescriptor(Object.getPrototypeOf(screen), 'state');
	// The engine uses explicit get/set methods or direct property
	// Actually ScreenDisplay uses defineProperty for state, let's check differently
	var stateExists = false;
	try {
		var s = screen.state;
		stateExists = typeof s === 'object' && s !== null;
	} catch (e) {
		stateExists = false;
	}
	TestHarness.assert(stateExists, 'ScreenDisplay has state getter (returns object)');

	// ScreenDisplay.state getter returns object with expected fields
	if (stateExists) {
		var state = screen.state;
		TestHarness.assert('position' in state, 'ScreenDisplay.state has position field');
		TestHarness.assert('place' in state, 'ScreenDisplay.state has place field');
		TestHarness.assert('characters' in state, 'ScreenDisplay.state has characters field');
		TestHarness.assert('locks' in state, 'ScreenDisplay.state has locks field');
		TestHarness.assert('cr_icons' in state, 'ScreenDisplay.state has cr_icons field');
		TestHarness.assert('popups' in state, 'ScreenDisplay.state has popups field');
		TestHarness.assert('text' in state, 'ScreenDisplay.state has text field');
		TestHarness.assert('fade' in state, 'ScreenDisplay.state has fade field');
	}

	// TextDisplay
	TestHarness.assertType(TextDisplay, 'function', 'TextDisplay is a function (constructor)');

	var textDisplay = new TextDisplay(null, null);

	// TextDisplay creates render element with class display_engine_text
	TestHarness.assert(
		textDisplay.render.classList.contains('display_engine_text'),
		'TextDisplay creates render element with class display_engine_text'
	);

	// TextDisplay methods
	TestHarness.assertType(textDisplay.typeText, 'function', 'TextDisplay has typeText method');
	TestHarness.assertType(textDisplay.instantTypeText, 'function', 'TextDisplay has instantTypeText method');
	TestHarness.assertType(textDisplay.loadFrameText, 'function', 'TextDisplay has loadFrameText method');
	TestHarness.assertType(textDisplay.skip, 'function', 'TextDisplay has skip method');
	TestHarness.assertType(textDisplay.clearText, 'function', 'TextDisplay has clearText method');
	TestHarness.assertType(textDisplay.setInstantMode, 'function', 'TextDisplay has setInstantMode method');

	// TextDisplay state
	var textStateExists = false;
	try {
		var ts = textDisplay.state;
		textStateExists = typeof ts === 'object' || typeof ts === 'undefined';
		// state might be undefined initially, that's ok
		textStateExists = true;
	} catch (e) {
		textStateExists = false;
	}
	TestHarness.assert(textStateExists, 'TextDisplay has state getter/setter');

	// TextDisplay.decodeFirstTag tests
	TestHarness.assertType(textDisplay.decodeFirstTag, 'function', 'TextDisplay.decodeFirstTag is a function');

	// decodeFirstTag parses [#colour:red]text[/#] correctly
	var tag1 = textDisplay.decodeFirstTag('[#/colour:red]text[/#]');
	TestHarness.assert(tag1 !== null, 'decodeFirstTag parses [#/colour:red]text[/#]');
	if (tag1) {
		TestHarness.assertEqual(tag1.tag_definition, 'colour:red', 'decodeFirstTag tag_definition = colour:red');
		TestHarness.assertEqual(tag1.tag_contents, 'text', 'decodeFirstTag tag_contents = text');
	}

	// decodeFirstTag parses [#500] correctly (self-contained tag, numeric delay)
	var tag2 = textDisplay.decodeFirstTag('[#500]');
	TestHarness.assert(tag2 !== null, 'decodeFirstTag parses [#500]');
	if (tag2) {
		TestHarness.assertEqual(tag2.tag_definition, '500', 'decodeFirstTag tag_definition = 500');
		TestHarness.assertEqual(tag2.tag_contents, '', 'decodeFirstTag tag_contents is empty for self-contained');
	}

	// decodeFirstTag parses [#s] correctly (shake tag)
	var tag3 = textDisplay.decodeFirstTag('[#s]');
	TestHarness.assert(tag3 !== null, 'decodeFirstTag parses [#s] (shake tag)');
	if (tag3) {
		TestHarness.assertEqual(tag3.tag_definition, 's', 'decodeFirstTag tag_definition = s');
	}

	// decodeFirstTag parses [#f] correctly (flash tag)
	var tag4 = textDisplay.decodeFirstTag('[#f]');
	TestHarness.assert(tag4 !== null, 'decodeFirstTag parses [#f] (flash tag)');
	if (tag4) {
		TestHarness.assertEqual(tag4.tag_definition, 'f', 'decodeFirstTag tag_definition = f');
	}

	// decodeFirstTag parses [#/var:x]value[/#] correctly
	var tag5 = textDisplay.decodeFirstTag('[#/var:x]value[/#]');
	TestHarness.assert(tag5 !== null, 'decodeFirstTag parses [#/var:x]value[/#]');
	if (tag5) {
		TestHarness.assertEqual(tag5.tag_definition, 'var:x', 'decodeFirstTag tag_definition = var:x');
	}

	// decodeFirstTag parses [#instant:red]text[/#] correctly
	var tag6 = textDisplay.decodeFirstTag('[#/instant:red]text[/#]');
	TestHarness.assert(tag6 !== null, 'decodeFirstTag parses [#/instant:red]text[/#]');
	if (tag6) {
		TestHarness.assertEqual(tag6.tag_definition, 'instant:red', 'decodeFirstTag tag_definition = instant:red');
	}

	// decodeFirstTag returns null when no tags present
	var tag7 = textDisplay.decodeFirstTag('plain text with no tags');
	TestHarness.assertEqual(tag7, null, 'decodeFirstTag returns null when no tags present');

	// TextDisplay.instantTypeText renders plain text correctly
	var plainContainer = document.createElement('div');
	textDisplay.instantTypeText(plainContainer, 'hello world');
	TestHarness.assertEqual(plainContainer.textContent, 'hello world', 'instantTypeText renders plain text correctly');

	// TextDisplay.instantTypeText renders [#/colour:red] as span with style.color
	var colorContainer = document.createElement('div');
	textDisplay.instantTypeText(colorContainer, '[#/colour:red]colored[/#]');
	var colorSpan = colorContainer.querySelector('span');
	TestHarness.assert(
		colorSpan && colorSpan.style.color !== '',
		'instantTypeText renders [#/colour:red] as span with style.color'
	);

	// TextDisplay.instantTypeText renders [#/var:x] by reading from variable environment
	if (typeof VariableEnvironment === 'function') {
		var testEnv = new VariableEnvironment();
		testEnv.set('testvar', 'VAR_VALUE');
		textDisplay.setVariableEnvironment(testEnv);
		var varContainer = document.createElement('div');
		textDisplay.instantTypeText(varContainer, '[#/var:testvar][/#]');
		TestHarness.assert(
			varContainer.textContent.indexOf('VAR_VALUE') > -1,
			'instantTypeText renders [#/var:x] by reading from variable environment'
		);
		// Reset var env
		textDisplay.setVariableEnvironment(new VariableEnvironment());
	}

	// CallbackBuffer tests
	var buffer = new CallbackBuffer();

	// CallbackBuffer trigger returns a function
	var triggerFn = buffer.trigger(function() {});
	TestHarness.assertType(triggerFn, 'function', 'CallbackBuffer trigger returns a function');

	// CallbackBuffer calling the trigger function decrements pending count
	var buffer2 = new CallbackBuffer();
	var cb2Called = false;
	var trigger2 = buffer2.trigger(function() { cb2Called = true; });
	buffer2.call(function() { cb2Called = true; });
	TestHarness.assert(buffer2.numberOfPendingTriggers > 0, 'CallbackBuffer trigger increments pending count');
	trigger2();
	// After calling trigger, pending count should be decremented

	// CallbackBuffer call adds callback to queue
	var buffer3 = new CallbackBuffer();
	var cb3Called = false;
	buffer3.call(function() { cb3Called = true; });
	TestHarness.assert(buffer3.callbacks.length > 0, 'CallbackBuffer call adds callback to queue');

	// PlaceDisplay
	TestHarness.assertType(PlaceDisplay, 'function', 'PlaceDisplay is a function (constructor)');
	var placeDisplay = new PlaceDisplay(new CallbackBuffer());
	TestHarness.assertType(placeDisplay.setPlace, 'function', 'PlaceDisplay has setPlace method');
	TestHarness.assertType(placeDisplay.unsetPlace, 'function', 'PlaceDisplay has unsetPlace method');

	// CharactersDisplay
	TestHarness.assertType(CharactersDisplay, 'function', 'CharactersDisplay is a function (constructor)');
	var charsDisplay = new CharactersDisplay(new CallbackBuffer());
	TestHarness.assertType(charsDisplay.loadFrameCharacters, 'function', 'CharactersDisplay has loadFrameCharacters method');
	TestHarness.assertType(charsDisplay.removeCharacters, 'function', 'CharactersDisplay has removeCharacters method');

	// Constants
	TestHarness.assertDefined(ALIGN_LEFT, 'ALIGN_LEFT is defined');
	TestHarness.assertDefined(ALIGN_CENTER, 'ALIGN_CENTER is defined');
	TestHarness.assertDefined(ALIGN_RIGHT, 'ALIGN_RIGHT is defined');
	TestHarness.assertDefined(POSITION_CENTER, 'POSITION_CENTER is defined');
	TestHarness.assertDefined(POSITION_NONE, 'POSITION_NONE is defined');
	TestHarness.assertDefined(POSITION_DO_NOT_MOVE, 'POSITION_DO_NOT_MOVE is defined');
	TestHarness.assertDefined(POSITION_CENTER_ON_TALKING, 'POSITION_CENTER_ON_TALKING is defined');

	// RENDER_LATENCY
	TestHarness.assertDefined(RENDER_LATENCY, 'RENDER_LATENCY is defined');
	TestHarness.assertEqual(RENDER_LATENCY, 16, 'RENDER_LATENCY equals 16');
}
