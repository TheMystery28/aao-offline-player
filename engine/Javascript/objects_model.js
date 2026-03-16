"use strict";
/*
Ace Attorney Online - Object model manipulation.
The object model is an object which describes a structure as well as default values for scalar fields.
It also provides functionalities to manage arrays of objects with unique IDs.

An object model can be read as follows, property by property :
- If the model property has a scalar value, the real object property must have a scalar value of same type, the value from the model being the default.
- If the model property has an object value, the real object property must have an object or null value, null being the default. If non-null, the object uses the model property value as model.
- If the model property has an array value, the real object property must have an array value, empty array being the default. All elements of that array use the model property's first and only element as model.

In the object model, if a property is named "id" and set with a null value, it will be filled with a unique numeric identifier when generating elements in an array.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'objects_model',
	dependencies : ['objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

// Determine first index. Normally, 0, but 1 if in case of 0 dummy element at start.
function getArrayFirstIndex(array)
{
	return (array[0] === 0) ? 1 : 0;
}

// Build a hash map of id -> index for an array with id-bearing objects.
// Stored as array.__id_index for O(1) lookups.
function buildIdIndex(array)
{
	var map = {};
	var first = getArrayFirstIndex(array);
	for(var i = first; i < array.length; i++)
	{
		if(array[i] && array[i].id !== undefined)
		{
			map[array[i].id] = i;
		}
	}
	array.__id_index = map;
	return map;
}

// Invalidate the id index after array mutations (insert, delete, move).
// The index will be lazily rebuilt on the next getIndexById call.
function invalidateIdIndex(array)
{
	if(array)
	{
		delete array.__id_index;
	}
}

function getDefaultValueFromModel(modelProperty)
{
	if(modelProperty == null || is_scalar(modelProperty))
	{
		// If row model property is scalar or null, use it as default value.
		return modelProperty;
	}
	else if(Array.isArray(modelProperty))
	{
		// If row model property is an array, set an empty array in the new object.
		return [];
	}
	else
	{
		// Row model property is an object : set it to null by default.
		return null;
	}
}

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS

// Get an unused unique ID in an array of objects.
function getNewId(array)
{
	var first_index = getArrayFirstIndex(array);
	
	var max_id = 0;
	for(var i = !isNaN(first_index) ? first_index : 1; i < array.length; i++)
	{
		max_id = Math.max(max_id, array[i].id);
	}
	
	return max_id + 1;
}

// Get the index in array of the element that possesses a given ID
// Uses a lazily-built hash map for O(1) lookups instead of O(n) scanning.
// Returns -1 if object not found
function getIndexById(array, id)
{
	if(isNaN(id) || !array)
	{
		return -1;
	}

	// Use or build the id-to-index hash map
	var index = array.__id_index;
	if(!index)
	{
		index = buildIdIndex(array);
	}

	var result = index[id];
	return result !== undefined ? result : -1;
}

// Get the element from an array having a given ID
function getById(array, id)
{
	if(isNaN(id) || !array)
	{
		return null;
	}
	
	return array[getIndexById(array, id)];
}

// Generate a new object from a given model
function getObjectFromModel(objectModel)
{
	var newElement = {};
	
	// Go through all model properties.
	for(var propertyName in objectModel)
	{
		var modelProperty = objectModel[propertyName];
		newElement[propertyName] = getDefaultValueFromModel(modelProperty);
	}
	
	return newElement;
}

/* Extend a source object according to its object model.
 * This means that every property defined in the object model is present in the returned object, using default value is it was undefined on the source object.
 */
function extendObjectWithModel(sourceObject, objectModel)
{
	// Go through all model properties.
	for(var propertyName in objectModel)
	{
		var modelProperty = objectModel[propertyName];
		
		if(propertyName in sourceObject)
		{
			// If property already exists in the source object, process the extension recursively on it.
			var objectProperty = sourceObject[propertyName];
			
			if(Array.isArray(objectProperty))
			{
				// If property is an array, recursively apply to each array element using the corresponding row model.
				for(var i = getArrayFirstIndex(objectProperty); i < objectProperty.length; i++)
				{
					// Row model is always the entry at index 0 of the model array.
					extendObjectWithModel(objectProperty[i], modelProperty[0]);
				}
			}
			else if(objectProperty != null && !is_scalar(objectProperty))
			{
				// If property is a non-null, non-array object, recursively apply on it.
				extendObjectWithModel(objectProperty, modelProperty);
			}
		}
		else
		{
			// If property is not defined in the source object, define it using default value from the model.
			sourceObject[propertyName] = getDefaultValueFromModel(modelProperty);
		}
	}
}

/* Reduce a source object so that it only contains properties which differ from its object model.
 * This means that restoring the source object in full can be performed with the extendObjectWithModel function.
 */
function reduceObjectFromModel(sourceObject, objectModel)
{
	// Go through all model properties.
	for(var propertyName in objectModel)
	{
		var modelProperty = objectModel[propertyName];
		
		if(propertyName in sourceObject)
		{
			// If property exists in the source object...
			var objectProperty = sourceObject[propertyName];
			
			if(Array.isArray(objectProperty))
			{
				// If property is an array, recursively apply to each array element using the corresponding row model.
				for(var i = getArrayFirstIndex(objectProperty); i < objectProperty.length; i++)
				{
					// Row model is always the entry at index 0 of the model array.
					reduceObjectFromModel(objectProperty[i], modelProperty[0]);
				}
			}
			else if(objectProperty != null && !is_scalar(objectProperty))
			{
				// If property is a non-null, non-array object, recursively apply on it.
				reduceObjectFromModel(objectProperty, modelProperty);
			}
			else
			{
				// If property is a scalar value, compare with model.
				if(objectProperty === modelProperty)
				{
					// If property is the same as on model, remove it.
					delete sourceObject[propertyName];
				}
			}
		}
	}
}

// Insert a new row in an array according to a row model.
// A default row can be generated from the model, or an original can be provided.
function insertNewRowFromModel(index, array, rowModel, original)
{
	var newElement;
	if(original)
	{
		// If original row is provided, clone it.
		newElement = objClone(original);
	}
	else
	{
		// Else, generate a new row from the object model.
		newElement = getObjectFromModel(rowModel);
	}
	
	// Assign new unique id if necessary, by checking if the id field in the model is supposed to be auto-generated.
	if(('id' in rowModel) && rowModel.id === null)
	{
		newElement.id = getNewId(array);
	}
	
	// Insert new child.
	if(!index && index !== 0)
	{
		//If index is undefined, insert at end.
		array.push(newElement);
	}
	else
	{
		// Otherwise, use given position.
		array.splice(index, 0, newElement);
	}

	// Invalidate the id index since array positions have changed.
	invalidateIdIndex(array);

	return newElement;
}

/* Fetch a part of the object according to a path, and the corresponding object model.
 * A path is an array of path elements to follow in order, starting from the root object, to reach the designated object.
 * Each path element can be :
 * - A string : the name of the target property in the object designated by previous path elements.
 * - An integer : the index of the target element in the array designated by previous path elements.
 * - An object : a more complex matching rule.
 * Currently, only ID matching rule is supported : if the object has an 'id' property, the element with that ID is searched in the array designated by previous path elements.
 */
function getSubObjectAndModel(path, rootObject, rootModel) {
	var current_object = rootObject;
	var current_model = rootModel;
	
	var path_element;
	while(path_element = path.shift()) {
		switch(typeof path_element)
		{
			case 'string':
				// If string index, browsing an object. Fetch corresponding property.
				current_object = current_object[path_element];
				current_model = current_model[path_element];
				break;
			case 'number':
				// If numeric index, browsing an array. Fetch the corresponding element.
				current_object = current_object[path_element];
				current_model = current_model[0]; // Array row model is always the first and only row.
				break;
			case 'object':
				// Matching rule.
				if('id' in path_element)
				{
					// If ID matching rule, browsing an array. Fetch element with given ID.
					current_object = getById(current_object, path_element.id);
					current_model = current_model[0]; // Array row model is always the first and only row.
				}
				break;
		}
	}
	
	return {
		object: current_object,
		model: current_model
	};
}

// Insert new row in the array at a given path.
// A default row can be generated from the model, or an original to copy can be provided.
function createNewRowAt(arrayPath, index, rootObject, rootModel, original)
{
	var array_data = getSubObjectAndModel(arrayPath, rootObject, rootModel);
	
	// Row model is the first row for the array object model.
	return insertNewRowFromModel(index, array_data.object, array_data.model[0], original);
}

//END OF MODULE
Modules.complete('objects_model');
