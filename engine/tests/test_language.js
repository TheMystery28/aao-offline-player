"use strict";
/**
 * Language system regression tests.
 */
function testLanguage() {
	TestHarness.suite('Language');

	// Languages global object exists
	TestHarness.assertDefined(Languages, 'Languages global object exists');
	TestHarness.assertType(Languages, 'object', 'Languages is an object');

	// Languages.setMainLanguage is a function
	TestHarness.assertType(Languages.setMainLanguage, 'function', 'Languages.setMainLanguage is a function');

	// Languages.requestFiles is a function
	TestHarness.assertType(Languages.requestFiles, 'function', 'Languages.requestFiles is a function');

	// translateNode is a function
	TestHarness.assertType(translateNode, 'function', 'translateNode is a function');

	// l function (localization lookup) is a function
	TestHarness.assertType(l, 'function', 'l (localization lookup) is a function');

	// l('press') returns a non-empty string
	var pressText = l('press');
	TestHarness.assert(
		typeof pressText === 'string' && pressText.length > 0,
		'l(press) returns a non-empty string'
	);

	// l('present') returns a non-empty string
	var presentText = l('present');
	TestHarness.assert(
		typeof presentText === 'string' && presentText.length > 0,
		'l(present) returns a non-empty string'
	);

	// l('start') returns a non-empty string
	var startText = l('start');
	TestHarness.assert(
		typeof startText === 'string' && startText.length > 0,
		'l(start) returns a non-empty string'
	);

	// l('back') returns a non-empty string
	var backText = l('back');
	TestHarness.assert(
		typeof backText === 'string' && backText.length > 0,
		'l(back) returns a non-empty string'
	);

	// translateNode translates elements with data-locale-content attribute
	var testEl = document.createElement('span');
	testEl.setAttribute('data-locale-content', 'start');
	translateNode(testEl);
	TestHarness.assert(
		testEl.textContent.length > 0,
		'translateNode translates elements with data-locale-content attribute'
	);
}
