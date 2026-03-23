"use strict";
/**
 * DOM utilities regression tests (EXHAUSTIVE).
 */
function testNodes() {
	TestHarness.suite('Nodes');

	// Function existence
	TestHarness.assertType(emptyNode, 'function', 'emptyNode is a function');
	TestHarness.assertType(setNodeTextContents, 'function', 'setNodeTextContents is a function');
	TestHarness.assertType(appendNodeTextContents, 'function', 'appendNodeTextContents is a function');
	TestHarness.assertType(getFirstChildOfType, 'function', 'getFirstChildOfType is a function');
	TestHarness.assertType(getFirstSiblingOfType, 'function', 'getFirstSiblingOfType is a function');
	TestHarness.assertType(nodeContains, 'function', 'nodeContains is a function');
	TestHarness.assertType(nodeContainsOrIs, 'function', 'nodeContainsOrIs is a function');
	TestHarness.assertType(setClass, 'function', 'setClass is a function');
	TestHarness.assertType(addClass, 'function', 'addClass is a function');
	TestHarness.assertType(removeClass, 'function', 'removeClass is a function');
	TestHarness.assertType(toggleClass, 'function', 'toggleClass is a function');
	TestHarness.assertType(hasClass, 'function', 'hasClass is a function');
	TestHarness.assertType(getNodeScreenPosition, 'function', 'getNodeScreenPosition is a function');

	// emptyNode removes all children from a div
	var div1 = document.createElement('div');
	div1.appendChild(document.createElement('span'));
	div1.appendChild(document.createTextNode('hello'));
	emptyNode(div1);
	TestHarness.assertEqual(div1.childNodes.length, 0, 'emptyNode removes all children from a div');

	// emptyNode on already-empty node does not crash
	var div2 = document.createElement('div');
	var noCrash = true;
	try { emptyNode(div2); } catch (e) { noCrash = false; }
	TestHarness.assert(noCrash, 'emptyNode on already-empty node does not crash');

	// setNodeTextContents sets single-line text
	var div3 = document.createElement('div');
	setNodeTextContents(div3, 'hello world');
	TestHarness.assertEqual(div3.textContent, 'hello world', 'setNodeTextContents sets single-line text');

	// setNodeTextContents converts \\n to <br> elements
	var div4 = document.createElement('div');
	setNodeTextContents(div4, 'line1\nline2');
	var hasBr = false;
	for (var i = 0; i < div4.childNodes.length; i++) {
		if (div4.childNodes[i].tagName === 'BR') hasBr = true;
	}
	TestHarness.assert(hasBr, 'setNodeTextContents converts \\n to <br> elements');

	// appendNodeTextContents appends without clearing
	var div5 = document.createElement('div');
	setNodeTextContents(div5, 'first');
	appendNodeTextContents(div5, ' second');
	TestHarness.assertEqual(div5.textContent, 'first second', 'appendNodeTextContents appends without clearing');

	// getFirstChildOfType returns first child matching tag name
	var parent1 = document.createElement('div');
	parent1.appendChild(document.createTextNode('text'));
	var span1 = document.createElement('span');
	parent1.appendChild(span1);
	TestHarness.assertEqual(getFirstChildOfType(parent1, 'span'), span1, 'getFirstChildOfType returns first child matching tag name');

	// getFirstChildOfType returns false when no match
	var parent2 = document.createElement('div');
	parent2.appendChild(document.createElement('span'));
	TestHarness.assertEqual(getFirstChildOfType(parent2, 'p'), false, 'getFirstChildOfType returns false when no match');

	// getFirstSiblingOfType returns next sibling matching tag name
	var container = document.createElement('div');
	var first = document.createElement('div');
	var second = document.createElement('span');
	container.appendChild(first);
	container.appendChild(second);
	TestHarness.assertEqual(getFirstSiblingOfType(first, 'span'), second, 'getFirstSiblingOfType returns next sibling matching tag name');

	// getFirstSiblingOfType returns false when no match
	var container2 = document.createElement('div');
	var lone = document.createElement('div');
	container2.appendChild(lone);
	TestHarness.assertEqual(getFirstSiblingOfType(lone, 'span'), false, 'getFirstSiblingOfType returns false when no match');

	// nodeContains returns true for parent containing child
	var outer = document.createElement('div');
	var inner = document.createElement('span');
	outer.appendChild(inner);
	document.body.appendChild(outer);
	TestHarness.assert(nodeContains(outer, inner), 'nodeContains returns true for parent containing child');

	// nodeContains returns false for unrelated elements
	var unrelated = document.createElement('div');
	document.body.appendChild(unrelated);
	TestHarness.assert(!nodeContains(outer, unrelated), 'nodeContains returns false for unrelated elements');

	// nodeContainsOrIs returns true for same element
	TestHarness.assert(nodeContainsOrIs(outer, outer), 'nodeContainsOrIs returns true for same element');

	// Clean up
	document.body.removeChild(outer);
	document.body.removeChild(unrelated);

	// setClass replaces all classes
	var classDiv = document.createElement('div');
	classDiv.className = 'old-class';
	setClass(classDiv, 'new-class');
	TestHarness.assertEqual(classDiv.className, 'new-class', 'setClass replaces all classes');

	// addClass adds single class
	var addDiv = document.createElement('div');
	addClass(addDiv, 'test-class');
	TestHarness.assert(addDiv.classList.contains('test-class'), 'addClass adds single class');

	// addClass handles space-separated multiple classes
	var multiDiv = document.createElement('div');
	addClass(multiDiv, 'class-a class-b');
	TestHarness.assert(
		multiDiv.classList.contains('class-a') && multiDiv.classList.contains('class-b'),
		'addClass handles space-separated multiple classes'
	);

	// removeClass removes class, leaves others
	var removeDiv = document.createElement('div');
	removeDiv.className = 'keep remove-me';
	removeClass(removeDiv, 'remove-me');
	TestHarness.assert(
		removeDiv.classList.contains('keep') && !removeDiv.classList.contains('remove-me'),
		'removeClass removes class, leaves others'
	);

	// toggleClass(node, cls, true) adds, toggleClass(node, cls, false) removes
	var toggleDiv = document.createElement('div');
	toggleClass(toggleDiv, 'toggled', true);
	TestHarness.assert(toggleDiv.classList.contains('toggled'), 'toggleClass(node, cls, true) adds class');
	toggleClass(toggleDiv, 'toggled', false);
	TestHarness.assert(!toggleDiv.classList.contains('toggled'), 'toggleClass(node, cls, false) removes class');

	// hasClass returns true when present, false when absent
	var hasDiv = document.createElement('div');
	hasDiv.className = 'present';
	TestHarness.assert(hasClass(hasDiv, 'present'), 'hasClass returns true when present');
	TestHarness.assert(!hasClass(hasDiv, 'absent'), 'hasClass returns false when absent');
}
