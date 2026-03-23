"use strict";
/**
 * Object utilities regression tests.
 */
function testObjects() {
	TestHarness.suite('Objects');

	// Function existence
	TestHarness.assertType(objClone, 'function', 'objClone is a function');
	TestHarness.assertType(objCompare, 'function', 'objCompare is a function');
	TestHarness.assertType(getIndexById, 'function', 'getIndexById is a function');
	TestHarness.assertType(getById, 'function', 'getById is a function');
	TestHarness.assertType(getNewId, 'function', 'getNewId is a function');

	// objClone creates deep copy (modifying clone doesn't affect original)
	var original = { a: 1, b: { c: 2 } };
	var clone = objClone(original);
	clone.b.c = 99;
	TestHarness.assertEqual(original.b.c, 2, 'objClone creates deep copy (modifying clone does not affect original)');

	// objClone handles nested objects
	var nested = { x: { y: { z: 3 } } };
	var nestedClone = objClone(nested);
	TestHarness.assertEqual(nestedClone.x.y.z, 3, 'objClone handles nested objects');
	nestedClone.x.y.z = 100;
	TestHarness.assertEqual(nested.x.y.z, 3, 'objClone nested modification does not affect original');

	// objClone handles arrays
	var arr = [1, [2, 3], { a: 4 }];
	var arrClone = objClone(arr);
	TestHarness.assert(Array.isArray(arrClone), 'objClone handles arrays (result is array)');
	TestHarness.assertEqual(arrClone.length, 3, 'objClone array preserves length');
	arrClone[1][0] = 99;
	TestHarness.assertEqual(arr[1][0], 2, 'objClone array deep copy (modifying clone does not affect original)');

	// objClone handles null
	var nullClone = objClone(null);
	TestHarness.assertEqual(nullClone, null, 'objClone handles null');

	// getIndexById returns correct index for existing id
	var testArray = [undefined, { id: 10 }, { id: 20 }, { id: 30 }];
	// Clear any cached index
	delete testArray.__id_index;
	TestHarness.assertEqual(getIndexById(testArray, 10), 1, 'getIndexById returns correct index for existing id (10 → 1)');
	TestHarness.assertEqual(getIndexById(testArray, 20), 2, 'getIndexById returns correct index for existing id (20 → 2)');
	TestHarness.assertEqual(getIndexById(testArray, 30), 3, 'getIndexById returns correct index for existing id (30 → 3)');

	// getIndexById returns -1 for non-existing id
	TestHarness.assertEqual(getIndexById(testArray, 999), -1, 'getIndexById returns -1 for non-existing id');

	// getById returns correct row for existing id
	var row = getById(testArray, 20);
	TestHarness.assert(row !== null && row.id === 20, 'getById returns correct row for existing id');
}
