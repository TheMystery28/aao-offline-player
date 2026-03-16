"use strict";
/*
Ace Attorney Online - Action parameters handler

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'actions_parameters',
	dependencies : ['expression_engine', 'objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

function handleScalarValuesRecursively(action_parameters, callback)
{
	/*
	 * Callback function must take two arguments : the parent object and the name of the property to handle. 
	 * It can edit the value of this property.
	 * 
	 * As long as the callback function returns false or null, the recursive function will run through the whole tree, and return false at the end.
	 * When the callback function returns true, the recursive function will stop and return true.
	 */
	
	function recursiveHandle(params, i, callback)
	{
		if(is_scalar(params[i]) || params[i] == null)
		{
			if(callback(params, i))
			{
				// If callback returns true, recursive Handle does so as well.
				return true;
			}
			return false;
		}
		else
		{
			for(var j in params[i])
			{
				if(recursiveHandle(params[i], j, callback))
				{
					// If recursiveHandle call returns true, then stop and return true as well
					return true;
				}
			}
			return false;
		}
	}
	
	var obj_container = new Object({'params': objClone(action_parameters)});
	var return_status = recursiveHandle(obj_container, 'params', callback);
	
	return new Object({
		object: obj_container['params'],
		status: return_status
	});
}

function loadParameterScalar(value, env, literal_expressions)
{
	if(!value)
	{
		return 0;
	}
	
	value = value.toString();
	if(value.startsWith('val='))
	{
		// val prefix indicates a raw value
		return value.substr(4);
	}
	else if(value.startsWith('xpr='))
	{
		// xpr prefix indicates an expression to evaluate
		if(literal_expressions)
		{
			return value.substr(4);
		}
		else
		{
			return evaluate_expression(value.substr(4), env);
		}
	}
	else
	{
		// This shouldn't happen in correctly imported trials.
		debugger;
	}
}

//EXPORTED FUNCTIONS

// Returns the parameters object where all scalar values are computed, using the given environment when expressions are evaluated
// If literal_expressions is false, expressions are evaluated and replaced by their value; otherwise, expressions are litterally copied
function computeParameters(action_parameters, env, literal_expressions)
{
	return handleScalarValuesRecursively(action_parameters, function(params, i)
	{
		params[i] = loadParameterScalar(params[i], env, literal_expressions);
	}).object;
}

// Returns the parameters object where all scalar values are prefixed by "val="
function prefixRawParameters(action_parameters)
{
	return handleScalarValuesRecursively(action_parameters, function(params, i)
	{
		params[i] = 'val=' + params[i];
	}).object;
}

// Returns the parameters object where all scalar values are prefixed by "xpr="
// If convert_scalars_from_raw is true, scalars will also be encapsulated as string constants in the expression.
function prefixXprParameters(action_parameters, convert_scalars_from_raw)
{
	return handleScalarValuesRecursively(action_parameters, function(params, i)
	{
		if(convert_scalars_from_raw)
		{
			// Convert scalar to string, escape single quotes and encapsulate as string constant.
			params[i] = "xpr='" + (params[i] + '').replace(/'/g, "\\'") + "'";
		}
		else
		{
			// Just return as expression.
			params[i] = 'xpr=' + params[i];
		}
	}).object;
}

function coerceParametersToRaw(action_parameters)
{
	return handleScalarValuesRecursively(action_parameters, function(params, i)
	{
		var param_string = params[i].toString();
		if(!param_string.startsWith('val='))
		{
			params[i] = 'val=';
		}
	}).object;
}

// Returns true if all scalar values in the parameters object are raw, not expressions
function areOnlyRawParameters(action_parameters)
{
	return !handleScalarValuesRecursively(action_parameters, function(params, i)
	{
		var param_string = params[i].toString();
		if(!param_string.startsWith('val='))
		{
			return true;
		}
	}).status;
}

// Returns true if the parameters's type depends on variables
function typeDependsOnVariables(param_def)
{
	if(param_def.type_param)
	{
		for(var type_param_name in param_def.type_param)
		{
			if(param_def.type_param[type_param_name].type == 'var')
			{
				return true;
			}
		}
	}
	return false;
}

//END OF MODULE
Modules.complete('actions_parameters');
