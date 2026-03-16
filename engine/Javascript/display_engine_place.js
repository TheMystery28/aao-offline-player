"use strict";
/*
Ace Attorney Online - Place display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_place',
	dependencies : ['display_engine_globals', 'style_loader', 'events', 'objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
// Place display engine. 
function PlaceDisplay(render_buffer)
{
	var self = this;
	self.render_buffer = render_buffer || new CallbackBuffer();
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_place');
	self.bg_elt = document.createElement('div'); // Background element
	self.render.appendChild(self.bg_elt);
	self.clickable_map = document.createElement('map'); // Map for pointing areas
	self.render.appendChild(self.clickable_map);
	self.clickable_background = new Image(); // False background for pointing areas
	self.clickable_background.style.opacity = 0;
	addClass(self.clickable_background, 'clickable');
	self.render.appendChild(self.clickable_background);
	self.background_objects = document.createElement('div');
	self.render.appendChild(self.background_objects);
	self.contents = document.createElement('div');
	self.render.appendChild(self.contents);
	self.foreground_objects = document.createElement('div');
	self.render.appendChild(self.foreground_objects);
	
	// Stored status data
	self.objects = new Object({
		foreground_objects: new Object(),
		background_objects: new Object()
	});
	self.place_id;
	self.place_data;
	self.bg_desc;
	
	// Set the background of the screen	
	self.setBackground = function(graphic_descriptor, callback)
	{		
		if(self.bg_desc && self.bg_desc == graphic_descriptor)
		{
			//if the background is already in place
			callback();
		}
		else
		{
			// Set the background
			var bg_obj_desc = getObjectDescriptor(graphic_descriptor, 'bg_subdir');
			if('uri' in bg_obj_desc)
			{
				// Set image background; set object size and callback when dimensions are known
				var old_bg_elt = self.bg_elt;
				
				self.bg_elt = generateImageElement(bg_obj_desc.uri, function(img)
				{
					self.render.style.width = img.width + 'px';
					self.render.style.height = img.height + 'px';
					self.render.replaceChild(img, old_bg_elt);
					
					// Also set clickable background properly
					self.clickable_background.src = bg_obj_desc.uri;
					self.clickable_background.style.width = img.width + 'px';
					self.clickable_background.style.height = img.height + 'px';
					
					if(callback)
					{
						callback();
					}
				}, self.render_buffer);
			}
			else if('colour' in bg_obj_desc)
			{
				// Set colour background and callback
				self.bg_elt.style.display = 'none';
				
				self.render.style.background = bg_obj_desc.colour;
				self.render.style.width = '256px';
				self.render.style.height = '192px';
				if(callback)
				{
					callback();
				}
			}
			else
			{
				// Graphic descriptor of no known type ? That's a bug !
				debugger;
			}
			
			self.bg_desc = graphic_descriptor;
		}
	};
	
	// Set an object from the object list of the place
	self.setObject = function(layer, index)
	{
		var descriptor = self.place_data[layer][index];
		
		if(!self.objects[layer][descriptor.id])
		{
			var object_desc = getObjectDescriptor(descriptor);
			var object;
			if('uri' in object_desc)
			{
				object = new Image();
				object.src = object_desc.uri;
			}
			else if('colour' in object_desc)
			{
				object = document.createElement('div');
				object.style.width = '100%';
				object.style.height = '100%';
				object.style.background = object_desc.colour;
			}
			
			object.style.position = 'absolute';
			object.style.left = descriptor.x + 'px';
			object.style.top = descriptor.y + 'px';
			
			self[layer].appendChild(object);
			self.objects[layer][descriptor.id] = object;
		}
		
		self.objects[layer][descriptor.id].style.visibility = descriptor.hidden ? 'hidden' : 'inherit' ;
	};
	
	// Update visibility of all objects
	self.refreshObjects = function()
	{
		for(var layer in {'foreground_objects':0, 'background_objects':0})
		{
			for(var id in self.objects[layer])
			{
				self.objects[layer][id].style.visibility = getById(self.place_data[layer], id).hidden ? 'hidden' : 'visible' ;
			}
		}
	};
	
	// Clear all objects from the screen
	self.removeObjects = function()
	{
		for(var layer in {'foreground_objects':0, 'background_objects':0})
		{
			for(var id in self.objects[layer])
			{
				self[layer].removeChild(self.objects[layer][id]);
			}
			self.objects[layer] = new Object();
		}
	};
	
	// Read the place of a frame and apply it on the screen
	// Callback parameter and return value indicate whether the place changed or not
	self.setPlace = function(place_data, callback)
	{
		if(place_data)
		{
			// If there is place data given, handle it
			
			var place_id = place_data.id;
			//Has the place changed ?
			if(place_id != PLACE_NONE && self.place_id != place_id)
			{
				self.place_id = place_id;
				self.place_data = place_data;
				
				//remove previous objects
				self.removeObjects();
				
				//set the background
				self.setBackground(self.place_data.background, callback ? callback.bind(undefined, true) : null);
				
				//set all objects
				for(var i = 0; i < self.place_data.foreground_objects.length; i++)
				{
					self.setObject('foreground_objects', i);
				}
				for(var i = 0; i < self.place_data.background_objects.length; i++)
				{
					self.setObject('background_objects', i);
				}
				return true;
			}
			else
			{
				// Update object visibility if needed. Can happen with save states.
				self.place_data = place_data;
				self.refreshObjects();

				// No change : run callback immediately
				if(callback)
				{
					callback(false);
				}
				return false;
			}
		}
		else
		{
			// If no place data, stay on current place
			
			if(self.bg_desc)
			{
				// If there already is a background, run callback immediately
				if(callback)
				{
					callback(false);
				}
				return false;
			}
			else
			{
				// Else, set a transparent background before calling back
				self.setBackground({colour: 'transparent'}, callback ? callback.bind(undefined, true) : null);
				return true;
			}
		}
	};
	
	// Remove current place information; set a transparent background instead
	self.unsetPlace = function(callback)
	{
		self.place_id = PLACE_NONE;
		self.place_data = getPlace(PLACE_NONE);
		
		//null value : no background
		var graphic_descriptor = new Object({
			colour: 'transparent'
		});
		self.removeObjects();
		self.setBackground(graphic_descriptor);
		
		if(callback)
		{
			callback();
		}
	};
	
	// Interactivity : assign a callback on click on an object
	self.setObjectCallback = function(layer, id, callback)
	{
		var object = self.objects[layer][id];
		if(object)
		{
			unregisterEvent(object, 'click');
			registerEventHandler(object, 'click', callback, false);
		}
		addClass(object, 'clickable');
	};
	
	// Interactivity : generate and return a clickable area
	self.getClickableArea = function(area_desc)
	{
		var area = document.createElement('area');
		var shape = area_desc.split(':');
		var coords = shape[1];
		shape = shape[0];
		
		area.shape = shape;
		if(coords)
		{
			area.coords = coords;
		}
		
		// Firefox matches area from first to last : make sure default is last.
		if(shape == 'default')
		{
			self.clickable_map.appendChild(area);
		}
		else
		{
			self.clickable_map.insertBefore(area, self.clickable_map.firstChild);
		}
		
		// Make sure map is associated with image
		// For some reason, associating just at generation fails...
		self.clickable_map.name = 'clickable_map';
		self.clickable_background.useMap = '#clickable_map';
		
		return area;
	};
	
	self.setPlace(getPlace(PLACE_ERASER));
}

//END OF MODULE
Modules.complete('display_engine_place');
