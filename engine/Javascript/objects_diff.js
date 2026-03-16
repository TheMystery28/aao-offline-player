"use strict";
/*
Ace Attorney Online - Object diffs and patch algorithm.

Should also include 3-way merge algorithm at some point, cf.
Re-used and rewritten algorithm from https://github.com/dominictarr/adiff && https://github.com/dominictarr/xdiff.
Cf diff3 algo http://www.cis.upenn.edu/~bcpierce/papers/diff3-short.pdf
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'objects_diff',
	dependencies : ['objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

/*
 * DIFF GENERATION.
 * Methods to generate a diff from an original element to a modified one.
 */

// Get diff from one original object to a target object.
function getObjectDiff(originalObject, targetObject)
{
	var diff = {};
	
	// Go through all target object properties.
	for(var propertyName in targetObject)
	{
		var targetProperty = targetObject[propertyName];
		
		if(propertyName in originalObject)
		{
			// If property exists in the source object...
			var originalProperty = originalObject[propertyName];
			
			var propDiff = getDiff(originalProperty, targetProperty);
			if(propDiff != null)
			{
				// ...and its diff is not null, store it.
				diff[propertyName] = propDiff;
			}
		}
		else
		{
			// If property was added, store its value in the diff.
			diff[propertyName] = targetProperty;
		}
	}
	
	if(Object.keys(diff).length > 0)
	{
		// If there are changed properties in the diff, return it.
		return diff;
	}
	else
	{
		// If empty diff, return null.
		return null;
	}
}

// Get diff from one original array to a target array.
function getArrayDiff(originalArray, targetArray, id_attribute)
{
	// Returns 0 for false, 1 for "same", 2 for "verified as completely equal".
	function isSameElement(a, b, id_attribute)
	{
		if(a === null)
		{
			return b === null ? 2 : 0;
		}
		else if(is_scalar(a))
		{
			return is_scalar(b) && a === b ? 2 : 0;
		}
		else if(id_attribute && (id_attribute in a))
		{
			return ((id_attribute in b) && (a[id_attribute] === b[id_attribute])) ? 1 : 0;
		}
		else
		{
			return objCompare(a, b) ? 2 : 0;
		}
	}
	
	function getOriginalElementForMapping(targetIndex)
	{
		// Go through all unmapped elements in the original array.
		for(var i in unmappedOriginalIndexes)
		{
			var areSame = isSameElement(originalArray[i], targetArray[targetIndex], id_attribute);
			var areEqual = (areSame === 2);
			
			if(areSame)
			{
				// If objects are the same, map this new on this original.
				// TODO : improve algorithm in case several objects match to minimise the number of shift blocks.
				return {
					original_index: i,
					areEqual: areEqual
				};
			}
		}
		
		return null;
	}
	
	function hasEdits(diff)
	{
		// Check for length edit
		if(diff.length != originalArray.length)
		{
			return true;
		}
		
		// Check for original row edits
		if(Object.keys(diff.originalRowEdits).length > 0)
		{
			return true;
		}
		
		// Check for actual block edits
		for(var i = 0; i < diff.blockEdits.length; i++)
		{
			var blockEdit = diff.blockEdits[i];
			if(('shift' in blockEdit && blockEdit.shift != 0) || 'inserts' in blockEdit)
			{
				// If there is a block edit which is either insert or shift by non-0, there are block edits.
				return true;
			}
		}
		return false;
	}
	
	var diff = {
		originalRowEdits:{},
		blockEdits:[],
		length: targetArray.length
	};
	
	// Create object which has all original array indexes as keys. It represents the "still unmapped original indexes".
	var unmappedOriginalIndexes = {};
	for(var i = 0; i < originalArray.length; i++)
	{
		unmappedOriginalIndexes[i] = 1;
	}
	
	// Go through all elements of the target array, to find out how to rebuild each one from the original array.
	for(var i = 0; i < targetArray.length; i++)
	{
		var originalElt = getOriginalElementForMapping(i);
		
		if(originalElt)
		{
			// If an original element can be mapped, perform the mapping.
			var j = originalElt.original_index;
			var areEqual = originalElt.areEqual;
			
			// Update the block edits if required.
			var shift = i - j;
			if(diff.blockEdits.length == 0 ||
				diff.blockEdits[diff.blockEdits.length - 1].shift != shift)
			{
				// If the last block edit does not match the expected shift, append a new one.
				diff.blockEdits.push({
					start_index: i,
					end_index: i,
					shift: shift
				});
			}
			else
			{
				// If shift matches the last block, update the last block to include this index.
				var lastBlock = diff.blockEdits[diff.blockEdits.length - 1];
				lastBlock.end_index = i;
			}
			
			if(!areEqual)
			{
				var rowDiff = getDiff(originalArray[j], targetArray[i]);
				
				if(rowDiff != null)
				{
					diff.originalRowEdits[j] = rowDiff;
				}
			}
			
			// Remove the index we just mapped from the unmapped list.
			delete unmappedOriginalIndexes[j];
		}
		else
		{
			// Element cannot be mapped to an original : it is an inserted row.
			
			if(diff.blockEdits.length == 0 ||
				!('inserts' in diff.blockEdits[diff.blockEdits.length - 1]))
			{
				// If the last block edit is not an insert, append a new one.
				diff.blockEdits.push({
					start_index: i,
					end_index: i,
					inserts: [ targetArray[i] ]
				});
			}
			else
			{
				// If last block is an insert, update it to include this new row.
				var lastBlock = diff.blockEdits[diff.blockEdits.length - 1];
				lastBlock.end_index = i;
				lastBlock.inserts.push(targetArray[i]);
			}
		}
	}
	
	if(hasEdits(diff))
	{
		return diff;
	}
	else
	{
		return null;
	}
}

/*
 * DIFF PATCHING.
 * Methods to patch a diff onto an original element and return the target one.
 * Original object is untouched.
 */

function patchObject(originalObject, diff)
{
	if(diff === null)
	{
		// Null diff means target object is identical : clone the original.
		return objClone(originalObject);
	}
	
	var result = {};
	
	// Properties from the diff are processed and added to the result.
	for(var propertyName in diff)
	{
		if(propertyName in originalObject)
		{
			// If overriding existing property, treat it as a diff to patch.
			result[propertyName] = patch(originalObject[propertyName], diff[propertyName]);
		}
		else
		{
			// If new property, simply set it.
			result[propertyName] = diff[propertyName];
		}
	}
	
	// Properties from the original object that were not present in the diff are deeply cloned over.
	for(var propertyName in originalObject)
	{
		if(!(propertyName in result))
		{
			result[propertyName] = objClone(originalObject[propertyName]);
		}
	}
	
	return result;
}

function fixLegacyArrayDiff(originalArray, diff)
{
	// Legacy diff can use -1 to indicate unchanged length from original array.
	if(diff.length === -1)
	{
		diff.length = originalArray.length;
	}
	
	// Legacy diff can contain unordered block edits. Sort them now.
	diff.blockEdits.sort(function(a, b){
		return a.start_index - b.start_index;
	});
	
	// Legacy diff can contain indexes not mapped into blockEdits.
	// Declare them as in a blockEdit of shift 0.
	var last_mapped_index = -1;
	var newBlockEdits = [];
	for(var i = 0; i < diff.blockEdits.length; i++)
	{
		var blockEdit = diff.blockEdits[i];
		if(blockEdit.startIndex > last_mapped_index + 1)
		{
			// If there is unmapped space before this blockEdit, insert a new one.
			newBlockEdits.push({
				start_index: last_mapped_index + 1,
				end_index: blockEdit.startIndex -1,
				shift: 0
			});
		}
		
		// Update last mapped index.
		last_mapped_index = blockEdit.end_index;
	}
	
	// If there is unmapped space after the last blockEdit, insert a new one.
	if(last_mapped_index < diff.length - 1)
	{
		newBlockEdits.push({
			start_index: last_mapped_index + 1,
			end_index: diff.length -1,
			shift: 0
		});
	}
	
	if(newBlockEdits.length > 0)
	{
		// Insert new blockEdits and re-sort if needed.
		Array.prototype.push.apply(diff.blockEdits, newBlockEdits);
		diff.blockEdits.sort(function(a, b){
			return a.start_index - b.start_index;
		});
	}
}

function patchArray(originalArray, diff)
{
	if(diff === null)
	{
		// Null diff means target object is identical : clone the original.
		return objClone(originalArray);
	}
	
	var result = [];
	
	fixLegacyArrayDiff(originalArray, diff);
	
	var currentBlockEditIndex = 0;
	
	// Diff indicates target length : all these indexes need to be filled one by one.
	for(var i = 0; i < diff.length; i++)
	{
		if(diff.blockEdits[currentBlockEditIndex].end_index < i)
		{
			currentBlockEditIndex += 1;
		}
		
		var currentBlockEdit = diff.blockEdits[currentBlockEditIndex];
		
		if('shift' in currentBlockEdit)
		{
			// If index is in shift block, resolve underlying element and patch it.
			var originalIndex = i - currentBlockEdit.shift;
			
			if(originalIndex in diff.originalRowEdits)
			{
				// If row was edited, patch it.
				result.push(patch(originalArray[originalIndex], diff.originalRowEdits[originalIndex]));
			}
			else
			{
				// Else, clone from original.
				result.push(objClone(originalArray[originalIndex]));
			}
		}
		else
		{
			// If index is in inserts block, resolve row from the block's inserts list and clone it.
			result.push(objClone(currentBlockEdit.inserts[i - currentBlockEdit.start_index]));
		}
	}
	
	return result;
}

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS

/* Generate a diff from a given original element to a target element.
 */
function getDiff(originalElement, targetElement)
{
	if(Array.isArray(originalElement))
	{
		// Array : get array diff.
		return getArrayDiff(originalElement, targetElement, 'id');
	}
	else if(originalElement != null && !is_scalar(originalElement))
	{
		// Non-null, non-array Object : get object diff.
		return getObjectDiff(originalElement, targetElement);
	}
	else
	{
		// Scalar : new value if changed, null if same.
		return originalElement == targetElement ? null : targetElement;
	}
}

/* Patch a diff onto an original element to get the transformed one.
 */
function patch(originalElement, diff)
{
	if(Array.isArray(originalElement))
	{
		// Array : patch array diff.
		return patchArray(originalElement, diff);
	}
	else if(originalElement != null && !is_scalar(originalElement))
	{
		// Non-null, non-array Object : patch object diff.
		return patchObject(originalElement, diff);
	}
	else
	{
		// Scalar : original element if null, new value otherwise.
		return diff == null ? originalElement : diff;
	}
}

//END OF MODULE
Modules.complete('objects_diff');
