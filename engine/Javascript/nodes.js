"use strict";
/*
Ace Attorney Online - Generic functions to manage DOM nodes

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'nodes',
	dependencies : [],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function emptyNode(node)
{
	node.textContent = '';
}

function setNodeTextContents(node, text)
{
	emptyNode(node);
	
	appendNodeTextContents(node, text);
}

function appendNodeTextContents(node, text)
{
	var text_lines = text.toString().split(/\n/g);
	node.appendChild(document.createTextNode(text_lines[0]));
	for(var i = 1; i < text_lines.length; i++)
	{
		node.appendChild(document.createElement('br'));
		node.appendChild(document.createTextNode(text_lines[i]));
	}
}

function getFirstChildOfType(elt, tagName)
{
	var tagNameCap = tagName.toUpperCase();
	
	var result = elt.firstChild;
	while(result && result.tagName != tagNameCap)
	{
		result = result.nextSibling;
	}
	
	if(result && result.tagName == tagNameCap)
	{
		return result;
	}
	else
	{
		return false;
	}
}

function getFirstSiblingOfType(elt, tagName)
{
	var tagNameCap = tagName.toUpperCase();
	
	var result = elt.nextSibling;
	while(result && result.tagName != tagNameCap)
	{
		result = result.nextSibling;
	}
	
	if(result && result.tagName == tagNameCap)
	{
		return result;
	}
	else
	{
		return false;
	}
}

function nodeContains(container, contained)
{
	if(container.compareDocumentPosition)
	{
		return !!(container.compareDocumentPosition(contained) & 16);
	}
	else
	{
	    // IE feedback 780874 shows contains always gives "false" on a node; IE does not plan to fix this
		return container.contains(contained);
	}
}

function nodeContainsOrIs(container, contained)
{
	return (container == contained) || nodeContains(container, contained);
}

function setClass(node, class_name)
{
	node.className = class_name;
}

function addClass(node, class_name)
{
	if(class_name.indexOf(' ') > -1)
	{
		node.classList.add.apply(node.classList, class_name.split(' '));
	}
	else
	{
		node.classList.add(class_name);
	}
}

function removeClass(node, class_name)
{
	node.classList.remove(class_name);
}

function toggleClass(node, class_name, condition)
{
	if(condition)
	{
		addClass(node, class_name);
	}
	else
	{
		removeClass(node, class_name);
	}
}

function hasClass(node, class_name)
{
	return node.classList.contains(class_name);
}

//Returns the position of an object on the screen as an object with 2 fields : top and left, in pixels.
function getNodeScreenPosition(node)
{
	var elt = node;
	
	var position = new Object({left: 0, top: 0});
	
	//Accumulate offsets
	
	while(elt.offsetParent)
	{
		position.left += elt.offsetLeft;
		position.top += elt.offsetTop;
		
		elt = elt.offsetParent;
	}
	
	position.left += elt.offsetLeft;
	position.top += elt.offsetTop;
	
	//Deduce scrolling
	
	elt = node.parentNode;
	while(elt)
	{
		position.left -= elt.scrollLeft || 0;
		position.top -= elt.scrollTop || 0;
		
		if(elt.style && elt.style.position.indexOf('fixed') != -1)
		{
			return position;
		}
		
		elt = elt.parentNode;
	}
	
	return position;
}

//END OF MODULE
Modules.complete('nodes');
