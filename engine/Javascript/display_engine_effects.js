"use strict";
/*
Ace Attorney Online - Functions for handling effects on the top screen

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_effects',
	dependencies : ['display_engine_globals', 'frame_data', 'events', 'objects', 'nodes'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS

// takes a screen_display node to handle positioning and/or width/height of the fade holder
function ScreenFadeDisplay(screen_display, render_buffer) 
{
	var self = this;
	self.render_buffer = render_buffer || new CallbackBuffer();
	
	// DOM structure
	self.container = document.createElement('div');
	addClass(self.container, 'fadeholder');
	
	// make the screen fade
	self.setScreenFade = function(duration, colour, type, position) 
	{
		// add the fade holder in its initial state
		var opacity = (type == FADEMODE_FADEIN) ? 1 : 0;
		var newOpacity = (type == FADEMODE_FADEIN) ? 0 : 1;
		self.container.style.opacity = opacity;
 		self.container.style.background = colour;
		
		// set width of the fade holder so it works in scrolls
		var place_display = screen_display.place_display.render;
		self.container.style.width = place_display.style.width;

		// if we have to fade everything, we need a higher z-index than the text box; apply the associated class
		if(position == FADEPOS_EVERYTHING) 
		{
			addClass(self.container, 'fadeall');
		}
		// if not, remove the class in case the previous frame had a fade that faded everything
		else 
		{
			removeClass(self.container, 'fadeall');
		}
		
		// add ourselves in the right place in the DOM
		var node;

		switch(position) {

			case FADEPOS_BGONLY:
				node = screen_display.characters_display.render;
				break;
			
			case FADEPOS_BGCHARACTER:
				node = screen_display.characters_display.render;
				break;

			case FADEPOS_BEHINDTEXT:
				node = screen_display.popups_display.render;
				break;

			case FADEPOS_EVERYTHING:
				node = screen_display.text_display.render;
				break;

			default:
				console.error('AAO Error: Invalid screen fade position!')
				node = null;
				break;
		}
		var ref_node = (position == FADEPOS_BGONLY) ? node : node.nextSibling;

		node.parentNode.insertBefore(self.container, ref_node);

		// set transition and fade
		if(duration > 0) 
		{
			self.render_buffer.callWithLatency(function() {
				setTransition(self.container, 'opacity', true, duration / 1000);
				self.container.style.opacity = newOpacity;
			});
		}
		// if there's no fade duration, we don't need a transition
		else
		{
			self.container.style.opacity = newOpacity;
		}

		self.current_fade = new Object({
			duration: duration,
			colour: colour,
			type: type,
			position: position
		});
	};
	
	// remove the screen fade
	self.clearScreenFade = function() 
	{
		self.current_fade = null;

		if(document.body.contains(self.container)) 
		{
			self.container.parentNode.removeChild(self.container);
		}
	};

	// Export and restore state
	self.current_fade = null;

	Object.defineProperty(self, 'state', {
		get: function()
		{
			// get state of the current fade
			return {
				fade: self.current_fade
			};
		},
		set: function(state)
		{
			// get rid of the current fade
			self.clearScreenFade();
			// Saves from pre-fade AAO don't even have a state!
			if(state && state.fade)
			{
				var fade = state.fade;
				self.setScreenFade(0, fade.colour, fade.type, fade.position);
			}
		}
	});
}


//END OF MODULE
Modules.complete('display_engine_effects');
