"use strict";
/*
Ace Attorney Online - Global functions used by several modules of the display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_globals',
	dependencies : ['style_loader', 'events', 'objects'],
	init : function() {}

}));

//INDEPENDENT INSTRUCTIONS
// Enforced latency in rendering to leave time for redraws/reflows to the browser rendering engine. (in ms)
// Reduced from 50ms to 16ms (~1 frame at 60fps) for smoother transitions.
var RENDER_LATENCY = 16;

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS

/*
The callback buffer allows buffering several callbacks during the execution flow, in order to run them all in batch.
The purpose is to reduce the number of interruptions by JS events and timers, and with it the number of redraws/reflows, avoiding flickering.

The method "trigger" adds a callback function waiting for a notification of completion.
It takes as argument the callback function to run in the end, and returns the "fake" callback function to use to notify of the completion of the source event.
Typical use : img.addEventListener('load', buffer.trigger(function() { ... }), false);

The method "call" adds a callback function that is waiting only for the completion of other source events.
It takes as argument the callback function to run, and does not return anything.

The method "timer" adds a callback function that is waiting only for the completion of other source events, and will fire only after a given time.
All callbacks registered with the same time value will be executed as a single batch.

The method "callWithLatency" is a shorthand for "timer" with the default rendering latency.
*/
function CallbackBuffer() {
	this.callbacks = [];
	this.timedCallbacks = {};
	this.numberOfPendingTriggers = 0;
	this.pendingTimeout = 0;
	
	this.checkAndrun = (function() {
		if(this.numberOfPendingTriggers <= 0)
		{
			// Execute all normal callbacks.
			var callback;
			while(callback = this.callbacks.shift())
			{
				callback();
			}
			
			// Schedule execution of all timed callbacks.
			for(var time in this.timedCallbacks)
			{
				// For each target time...
				window.setTimeout((function(callbacks){
					// Execute all callbacks for that time as a batch.
					var callback;
					while(callback = callbacks.shift())
					{
						callback();
					}
				}).bind(undefined, this.timedCallbacks[time]), time);
			}
			// All callbacks scheduled : clear this object's internal state.
			this.timedCallbacks = {};
			
			this.numberOfPendingTriggers = 0;
			if(this.pendingTimeout)
			{
				window.clearTimeout(this.pendingTimeout);
				this.pendingTimeout = 0;
			}
		}
	}).bind(this);
	
	this.resolvePendingTrigger = (function() {
		this.numberOfPendingTriggers--;
		this.checkAndrun();
	}).bind(this);
	
	this.trigger = function(callback) {
		this.callbacks.push(callback);
		this.numberOfPendingTriggers++;
		
		// If there is at least one trigger, the timeout is useless - execution will only take place after all triggers.
		if(this.pendingTimeout)
		{
			window.clearTimeout(this.pendingTimeout);
			this.pendingTimeout = 0;
		}
		
		return this.resolvePendingTrigger;
	};
	
	this.call = function(callback) {
		this.callbacks.push(callback);
		
		// If there is no pending trigger, do not call immediately (leave time to buffer other tasks in the same execution flow)
		// but make sure that this will be called at the end of the execution flow.
		if(!this.pendingTimeout && this.numberOfPendingTriggers <= 0)
		{
			this.pendingTimeout = window.setTimeout(this.checkAndrun, 0);
		}
	};
	
	this.timer = function(callback, time) {
		// Store the timed callback in the buffer.
		if(!(time in this.timedCallbacks))
		{
			this.timedCallbacks[time] = [];
		}
		this.timedCallbacks[time].push(callback);
		
		// If there is no pending trigger, do not call immediately (leave time to buffer other tasks in the same execution flow)
		// but make sure that this will be called at the end of the execution flow.
		if(!this.pendingTimeout && this.numberOfPendingTriggers <= 0)
		{
			this.pendingTimeout = window.setTimeout(this.checkAndrun, 0);
		}
	};
	
	this.callWithLatency = function(callback) {
		this.timer(callback, RENDER_LATENCY);
	};
	
	return this;
}

// Generate a javascript Image element, making sure the animation restarts if any, and callback
function generateImageElement(uri, callback, callback_buffer)
{
	function handleLoadError(image, loadCompleteTrigger)
	{
		console.log(image.src, 'failed to load');
		//debugger;
		
		// Force dimensions to be non-null, since place rendering relies on bg dimensions
		image.width = 256;
		image.height = 192;
		
		if(loadCompleteTrigger)
		{
			loadCompleteTrigger();
		}
	}
	
	var img = new Image();
	
	var loadCompleteTrigger;
	if(callback_buffer && callback)
	{
		loadCompleteTrigger = callback_buffer.trigger(callback.bind(undefined, img));
	}
	else if(callback)
	{
		loadCompleteTrigger = callback.bind(undefined, img);
	}
	else
	{
		loadCompleteTrigger = null;
	}
	
	if(uri == '')
	{
		// Avoid launching a request to the current page, or being stuck by https://bugzilla.mozilla.org/show_bug.cgi?id=599975
		
		// Defer load error handling : callback should not be called during the execution of the current flow.
		window.setTimeout(handleLoadError.bind(undefined, img, loadCompleteTrigger), 0);
		return img;
	}
	
	registerEventHandler(img, 'load', function()
	{
		// Reset animation : Webkit seems to restart only when resetting an object, not when creating a new one...
		unregisterEvent(img, 'load'); // Avoid loop of load events...
		window.setTimeout(function()
		{
			img.src = uri; // Reassign uri after a null timeout : webkit sometimes doesn't react if it's done inline
			
			if(loadCompleteTrigger)
			{
				loadCompleteTrigger();
			}
		}, 0);
	}, false);
	registerEventHandler(img, 'error', handleLoadError.bind(undefined, img, loadCompleteTrigger), false);
	img.src = uri;
	
	return img;
}

// Generate a graphic element from its descriptor
function generateGraphicElement(graphic_descriptor, callback, callback_buffer)
{
	if('uri' in graphic_descriptor)
	{
		//if this describes an image file, load it
		var element = document.createElement('div');
		
		var inner_element = document.createElement('div');
		addClass(inner_element, 'inner_elt');
		element.appendChild(inner_element);
		
		var uri = graphic_descriptor.uri;
		
		var img = generateImageElement(uri, function(img)
		{
			// Set parent element's size
			element.style.height = img.height + 'px';
			element.style.width = img.width + 'px';
			inner_element.style.height = img.height + 'px';
			inner_element.style.width = img.width + 'px';
			element.setAttribute('data-height', img.height);
			element.setAttribute('data-width', img.width);
			
			if(callback)
			{
				callback(img, element);
			}
		}, callback_buffer);
		inner_element.appendChild(img);
		
		return element;
	}
	else
	{
		// TODO : handle non-picture elements some day ?
		console.warn('generateGraphicElement: non-picture element not supported', graphic_descriptor);
	}
}

// Update a graphic element using a descriptor
function updateGraphicElement(element, graphic_descriptor, callback, callback_buffer)
{
	var inner_element = element.firstChild;

	if('uri' in graphic_descriptor)
	{
		// Skip reload if the URI hasn't changed — avoids re-fetching the same sprite
		var current_img = inner_element ? inner_element.lastChild : null;
		if(current_img && current_img.src && current_img.src.indexOf(graphic_descriptor.uri) !== -1
			&& graphic_descriptor.uri !== '')
		{
			// URI matches — skip image reload, just call back
			if(callback_buffer && callback)
			{
				callback_buffer.call(callback.bind(undefined, current_img, element));
			}
			else if(callback)
			{
				window.setTimeout(callback.bind(undefined, current_img, element), 0);
			}
			return;
		}

		var new_img = generateImageElement(graphic_descriptor.uri, function(img)
		{
			// Set parent element's size
			element.style.height = img.height + 'px';
			element.style.width = img.width + 'px';
			inner_element.style.height = img.height + 'px';
			inner_element.style.width = img.width + 'px';
			element.setAttribute('data-height', img.height);
			element.setAttribute('data-width', img.width);

			inner_element.replaceChild(img, inner_element.firstChild); // Replace the image.

			if(callback)
			{
				callback(img, element);
			}
		}, callback_buffer);
	}
	else
	{
		// TODO : handle non-picture elements some day ?
		console.warn('updateGraphicElement: non-picture element not supported', graphic_descriptor);
	}
}

function setEffectToGraphicElement(element, effect_name, enabled)
{
	var inner_element = element.firstChild;
	var img = inner_element.lastChild;
	
	switch(effect_name)
	{
		case 'mirror':
			
			if(enabled)
			{
				addClass(inner_element, 'mirrored');
			}
			else
			{
				removeClass(inner_element, 'mirrored');
			}
			
			break;
		
		default:
			console.warn('setEffectToGraphicElement: unknown effect', effect_name);
			break;
	}
}

// Set the position of a graphic element on the scene
function setGraphicElementPosition(element, position, container)
{
	var inner_element = element.firstChild;
	inner_element.style.left = position.shift+'px';
	switch(position.align)
	{
		case ALIGN_LEFT :
			element.style.left = 0;
			break;
		
		case ALIGN_CENTER :
			element.style.left = Math.floor((container.clientWidth / 2) - (element.getAttribute('data-width') / 2))+'px';
			break;
		
		case ALIGN_RIGHT : 
			element.style.left = Math.floor(container.clientWidth - element.getAttribute('data-width'))+'px';
			break;
	}
	
	// Vertically align on the bottom
	element.style.bottom = 0;
	
	// TODO : really handle vertical align as well some day ?
}

/*
 * Update the CSS transition property of a node according to the transition descriptors held by the node.
 */
function updateTransitionProperty(node)
{
	var property_values = [];
	
	// Loop through all transitioned properties.
	for(var prop in node.current_transitions)
	{
		if(node.current_transitions[prop].length > 0)
		{
			// Apply the first transition in the list for this property.
			var transition_desc = node.current_transitions[prop][0];
			property_values.push(transition_desc.property + ' ' + transition_desc.duration + 's ' + transition_desc.timing_fct);
		}
	}
	node.style.transition = property_values.join(',');
}

/*
 * Cancel a transition on a node, given its transition descriptor.
 */
function cancelTransition(node, transition_desc)
{
	var t_index = node.current_transitions[transition_desc.property].indexOf(transition_desc);
	if(t_index != -1)
	{
		// If transition is still defined on the node, remove it and update the property.
		node.current_transitions[transition_desc.property].splice(t_index, 1);
		updateTransitionProperty(node);
	}
}

/*
 * Sets a transition onto a node : adds the transition descriptor to the node, and if necessary registers a call to cancel it later on.
 */
function setTransition(node, property, only_once, duration, timing_fct)
{
	if(!node)
	{
		//if node has been deleted while transitioning, do not crash
		return;
	}
	
	// Ensure that the node has an array storing the current transitions.
	if(!node.current_transitions)
	{
		node.current_transitions = {};
	}
	if(!(property in node.current_transitions))
	{
		node.current_transitions[property] = [];
	}
	
	// Create new transition descriptor
	var new_transition = {
		property: property,
		duration: duration,
		timing_fct: timing_fct || 'linear'
	};
	
	// Add transition to the head of the list, and update the property.
	node.current_transitions[property].unshift(new_transition);
	updateTransitionProperty(node);
	
	if(only_once)
	{
		// If transition is enabled only once, register event to remove it when over.
		window.setTimeout(cancelTransition.bind(undefined, node, new_transition), duration * 1000);
	}
	
	return new_transition;
}

//END OF MODULE
Modules.complete('display_engine_globals');
