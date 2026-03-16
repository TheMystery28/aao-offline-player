"use strict";
/*
Ace Attorney Online - CR icons display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_cr_icons',
	dependencies : ['display_engine_globals', 'frame_data', 'events', 'objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
// Can be linked to a ScreenDisplay to enable auto positioning
function CrIconsDisplay(screen_display)
{
	var self = this;
	
	self.last_frame_was_merged = false;
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_cr_icons');
	
	self.displayed_icons = new Object(); // Currently displayed icons
	
	// Add an icon
	self.showCrIcon = function(icon)
	{
		var position = icon.position;
		var element_desc = icon.element_desc;
		
		var element_data = getRowById(element_desc.type, element_desc.id);
		var icon_def = position + '_' + element_desc.type + '_' + element_desc.id;
		
		// If element doesn't exist or icon is already displayed, do nothing
		if(element_data && !(icon_def in self.displayed_icons))
		{
			var img_container = document.createElement('div');
			var img = new Image();
			
			// Set image position
			switch(position)
			{
				case 'topright':
					img_container.style.top = 5 + 'px';
					img_container.style.right = 5 + 'px';
					break;
				case 'topleft':
					img_container.style.top = 5 + 'px';
					img_container.style.left = 5 + 'px';
					break;
				case 'bottomright':
					img_container.style.bottom = 5 + 'px';
					img_container.style.right = 5 + 'px';
					break;
				case 'bottomleft':
					img_container.style.bottom = 5 + 'px';
					img_container.style.left = 5 + 'px';
					break;
				case 'auto':
					img_container.style.top = 5 + 'px';
					if(!screen_display || !screen_display.currentPositionData || screen_display.currentPositionData.align != ALIGN_RIGHT)
					{
						img_container.style.right = 5 + 'px';
					}
					else
					{
						img_container.style.left = 5 + 'px';
					}
					break;
			}
			
			// Set image size dynamically to enable transition
			img_container.style.width = 0;
			img_container.style.height = 0;
			registerEventHandler(img, 'load', function(){
				img_container.style.width = img.width + 'px';
				img_container.style.height = img.height + 'px';
				img_container.appendChild(img);
			}, false);
			
			// Set image URL
			switch(element_desc.type)
			{
				case 'profiles':
					img.src = getProfileIconUrl(element_data);
					break;
				
				case 'evidence':
					img.src = getEvidenceIconUrl(element_data);
					break;
				
				default:
					debugger;
					break;
			}
			
			// Add the image
			self.displayed_icons[icon_def] = img_container;
			self.render.insertBefore(img_container, null);
		}
	};
	
	// Remove all icons
	self.clearIcons = function(exceptions)
	{
		// Process list of exceptions to get definition strings
		var exceptions_def = new Array();
		if(exceptions)
		{
			for(var i = 0; i < exceptions.length; i++)
			{
				exceptions_def.push(exceptions[i].position + '_' + exceptions[i].element_desc.type + '_' + exceptions[i].element_desc.id);
			}
		}
		
		var removed_icons = new Array();
		
		// Go through all icons
		for(var icon_def in self.displayed_icons)
		{
			// Except for given exceptions
			if(exceptions_def.indexOf(icon_def) == -1)
			{
				// Transition back to null size
				self.displayed_icons[icon_def].style.width = 0;
				self.displayed_icons[icon_def].style.height = 0;
				
				// And remove from screen data
				removed_icons.push(self.displayed_icons[icon_def]);
				delete self.displayed_icons[icon_def];
			}
		}
		
		// Actually remove the icons after a while
		window.setTimeout((function(icons){
			for(var i = 0; i < icons.length; i++)
			{
				icons[i].parentNode.removeChild(icons[i]);
			}
		}).bind(undefined, removed_icons), 300);
	};
	
	// Export and restore state of cr icons display engine.
	Object.defineProperty(self, 'state', {
		get: function()
		{
			return {
				displayed_icons: Object.keys(self.displayed_icons)
			};
		},
		set: function(state)
		{
			self.clearIcons();
			
			if(state && state.displayed_icons)
			{
				for(var i = 0; i < state.displayed_icons.length; i++)
				{
					var icon_def_split = state.displayed_icons[i].split(/_/g);
					
					self.showCrIcon({
						position: icon_def_split[0],
						element_desc: {
							type: icon_def_split[1],
							id: icon_def_split[2]
						}
					});
				}
			}
		}
	});
}

//END OF MODULE
Modules.complete('display_engine_cr_icons');
