"use strict";
/**
 * Expression engine regression tests.
 */
function testExpressionEngine() {
	TestHarness.suite('Expression Engine');

	// evaluate_expression is a function
	TestHarness.assertType(evaluate_expression, 'function', 'evaluate_expression is a function');

	// register_custom_function is a function
	TestHarness.assertType(register_custom_function, 'function', 'register_custom_function is a function');

	// VariableEnvironment is a function (constructor)
	TestHarness.assertType(VariableEnvironment, 'function', 'VariableEnvironment is a function (constructor)');

	// VariableEnvironment set and get store and retrieve values
	var env1 = new VariableEnvironment();
	env1.set('x', 42);
	TestHarness.assertEqual(env1.get('x'), 42, 'VariableEnvironment set and get store and retrieve values');

	// VariableEnvironment with parent reads from parent when local not set
	var parentEnv = new VariableEnvironment();
	parentEnv.set('inherited', 'from_parent');
	var childEnv = new VariableEnvironment(parentEnv);
	TestHarness.assertEqual(
		childEnv.get('inherited'), 'from_parent',
		'VariableEnvironment with parent reads from parent when local not set'
	);

	// VariableEnvironment child overrides parent
	childEnv.set('inherited', 'from_child');
	TestHarness.assertEqual(
		childEnv.get('inherited'), 'from_child',
		'VariableEnvironment child value overrides parent'
	);

	// computeParameters is a function
	TestHarness.assertType(computeParameters, 'function', 'computeParameters is a function');

	// Custom function 'current_frame_id' is registered and returns player_status.current_frame_id
	if (typeof player_status !== 'undefined') {
		var testEnv = new VariableEnvironment(global_env);
		var result = evaluate_expression("f:current_frame_id()", testEnv);
		TestHarness.assertEqual(
			result, player_status.current_frame_id,
			'Custom function current_frame_id returns player_status.current_frame_id'
		);
	}

	// Custom function 'player_health' — check if registered
	// player_health is registered in player_debug.js, not player.js
	// Just verify we can evaluate a simple expression
	var simpleEnv = new VariableEnvironment();
	simpleEnv.set('a', 5);
	var evalResult = evaluate_expression('a + 3', simpleEnv);
	TestHarness.assertEqual(evalResult, 8, 'evaluate_expression correctly evaluates a + 3 = 8');
}
