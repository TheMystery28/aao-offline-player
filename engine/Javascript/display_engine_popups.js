"use strict";
/*
Ace Attorney Online - Popups display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_popups',
	dependencies : ['display_engine_globals', 'frame_data', 'events', 'objects', 'nodes'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
function PopupsDisplay(render_buffer)
{
	var self = this;
	self.render_buffer = render_buffer || new CallbackBuffer();
	
	self.last_frame_was_merged = false;
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_popups');
	
	self.popups = new Array(); // Currently displayed popup objects
	
	// Set a popup on the scene
	self.setPopup = function(popup_info)
	{
		//Clone popup info to avoid side effects from other functions
		var popup_info = new objClone(popup_info);
		self.displayed_popups.push(popup_info);
		
		if(popup_info.popup_id !== 0)
		{
			// Only act if a popup is actually selected
			var popup_index = self.popups.length;
			
			var descriptor = getPopupDescriptor(popup_info);
			
			var newElement = generateGraphicElement(descriptor, function(img, newElement){
				addClass(newElement, 'ts_popup');
				self.setPopupEffect(popup_info, popup_index, img);
				
				self.render.appendChild(newElement);
			}, self.render_buffer);
			
			self.popups.push(newElement);
		}
	};
	
	// Set the effect to apply to the popup
	self.setPopupEffect = function(popup_info, popup_index, img)
	{
		//Set mirror status
		toggleClass(img, 'mirrored', popup_info.mirror_effect);
	};
	
	// Remove all displayed popups
	self.removePopups = function()
	{
		while(self.popups.length > 0)
		{
			//Remove element
			var element = self.popups.pop();
			self.render_buffer.call(self.render.removeChild.bind(self.render, element));
		}
		self.displayed_popups = [];
	};
	
	// Load popups from frame data
	self.loadFramePopups = function(frame_data, callback)
	{
		if(
			!self.last_frame_was_merged ||
			frame_data.popups.length > 0
		)
		{
			// If new frame, or redefining frame popups, remove currently shown popups
			self.removePopups();
		}
		self.last_frame_was_merged = frame_data.merged_to_next;
		
		//Load all popups
		for(var i = 0; i < frame_data.popups.length; i++)
		{
			self.setPopup(frame_data.popups[i]);
		}
		
		if(callback)
		{
			callback();
		}
	};
	
	// Export and restore state of popups display engine.
	self.displayed_popups = [];
	Object.defineProperty(self, 'state', {
		get: function()
		{
			return {
				popups: self.displayed_popups,
				last_frame_was_merged: self.last_frame_was_merged
			};
		},
		set: function(state)
		{
			self.removePopups();
			for(var i = 0; i < state.popups.length; i++)
			{
				self.setPopup(state.popups[i]);
			}
			
			self.last_frame_was_merged = state.last_frame_was_merged;
		}
	});
}

//END OF MODULE
Modules.complete('display_engine_popups');
