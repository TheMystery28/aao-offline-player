"use strict";
/*
Ace Attorney Online - Event managers

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'events',
	dependencies : ['nodes'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
function registerEventHandler(element, event, handler, capture)
{
	//structure to store the event handlers on the object
	if(!element.event_handlers)
	{
		element.event_handlers = new Object();
	}
	if(!element.event_handlers[event])
	{
		element.event_handlers[event] = new Array();
	}
	
	//store this precise event handler
	element.event_handlers[event].push(new Object({
		handler: handler,
		capture: capture
	}));
	
	//add event listener
	element.addEventListener(event, handler, capture);
	
	//return index of the event in the list
	return element.event_handlers[event].length - 1;
}

function unregisterEventHandler(element, event, index)
{
	if(element.event_handlers && element.event_handlers[event] && element.event_handlers[event][index])
	{
		//remove event listener
		element.removeEventListener(event, element.event_handlers[event][index].handler, element.event_handlers[event][index].capture);
		
		//Delete event from the list
		element.event_handlers[event][index] = null;
	}
}

function unregisterEvent(element, event)
{
	if(element.event_handlers && element.event_handlers[event])
	{
		for(var i = 0; i < element.event_handlers[event].length; i++)
		{
			unregisterEventHandler(element, event, i);
		}
		element.event_handlers[event] = null;
	}
}

function unregisterAllEvents(element)
{
	if(element.event_handlers)
	{
		for(var i in element.event_handlers)
		{
			unregisterEvent(element, i);
		}
		element.event_handlers = null;
	}
}

function triggerEvent(element, event)
{
	if(element.event_handlers && element.event_handlers[event])
	{
		var fake_event = new Event(event, { bubbles: false, cancelable: false });

		for(var i = 0; i < element.event_handlers[event].length; i++)
		{
			// Skip null entries left by unregisterEventHandler
			if(element.event_handlers[event][i])
			{
				element.event_handlers[event][i].handler.bind(element)(fake_event);
			}
		}
	}
}

function hasDefaultFocusEvent(element)
{
	return (hasClass(element, 'select')) || ['select', 'option', 'input', 'button'].indexOf(element.tagName.toLowerCase()) > -1;
}

//Get the element that will be focused by a click on the given element.
//Search can be restricted to a container - return null if no focus parent found.
function getFocusParent(element, container)
{
	var focus_target = element;
	container = container || document.body;
	var contained = true;
	while(focus_target && (contained = nodeContains(container, focus_target)) && !hasDefaultFocusEvent(focus_target))
	{
		focus_target = focus_target.parentNode;
	}
	
	return contained ? focus_target : null;
}

//Fix for focus, addressing part of Webkit bugs...
//cf. https://bugs.webkit.org/show_bug.cgi?id=22261
registerEventHandler(document, 'mousedown', function(e){
	var name = e.target.tagName.toLowerCase();
	if(name == 'button' || (name == 'input' && e.target.type == 'color')) {
		e.target.focus();
		e.preventDefault();
	}
}, true);

//END OF MODULE
Modules.complete('events');
