"use strict";
/*
Ace Attorney Online - Generic object manipulation

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'objects',
	dependencies : [],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES

//Deep copy of objects
//Thanks to http://blog.imaginea.com/deep-copy-in-javascript/
var ObjectHandler = {
	//public method
	getCloneOfObject: function(oldObject,level) 
	{
		var tempClone;

		if(level == undefined)
		{
			level = 0;
		}

		if(oldObject == null)
		{
			tempClone = null;
		}
		else if(typeof(oldObject) == "object" && oldObject.length != undefined)
		{
			//array
			tempClone = new Array();
			for(var i = 0; i < oldObject.length; ++i)
			{
				tempClone[tempClone.length] = ObjectHandler.getCloneOfObject(oldObject[i],level + 1);
			}
		}
		else if(typeof(oldObject) == "object")
		{
			//object
			tempClone = new Object();
			for(var i in oldObject)
			{
				tempClone[i] = ObjectHandler.getCloneOfObject(oldObject[i],level + 1);
			}
		}
		else
		{
			//plain
			//print ("Level=" + level + " Plain=" + oldObject);
			tempClone = oldObject;
		}

		return tempClone;
	}
};

//EXPORTED FUNCTIONS
function objClone(what) 
{
	return ObjectHandler.getCloneOfObject(what);
}

//Object comparison. 
function objCompare(obj1, obj2, strict)
{
	function size(obj)
	{
		var size = 0;
		for (var keyName in obj)
		{
			if (keyName != null)
				size++;
		}
		return size;
	}
	
	if(obj1 === obj2)
	{
		return true;
	}
	else if(
		(strict && typeof obj1 != typeof obj2) || // In strict mode, check exact type
		(!strict && is_scalar(obj1) != is_scalar(obj2)) // In non-strict, check that they are both scalar or non-scalar
	)
	{
		return false;
	}
	else
	{
		switch(typeof obj1)
		{
			case 'object' :
			case 'function' :
				if(size(obj1) != size(obj2))
				{
					return false;
				}
				
				for(var i in obj1)
				{
					if(!objCompare(obj1[i], obj2[i], strict))
					{
						return false;
					}
				}
				
				return true;
			
			case 'undefined':
				return false;
				
			default :
				if(strict)
				{
					return obj1 === obj2;
				}
				else
				{
					return obj1 == obj2;
				}
		}
	}
}

function is_scalar (mixed_var) {
    // From http://phpjs.org/functions/is_scalar
    return (/boolean|number|string/).test(typeof mixed_var);
}

//END OF MODULE
Modules.complete('objects');
