"use strict";
/*
Ace Attorney Online - Psyche locks display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_locks',
	dependencies : ['display_engine_globals', 'events', 'objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
function LocksDisplay()
{
	var self = this;
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_locks');
	
	self.graphics_chains = document.createElement('div');
	self.render.appendChild(self.graphics_chains);
	self.graphics_locks = document.createElement('div');
	self.render.appendChild(self.graphics_locks);
	
	self.current_locks_dialogue_desc = null; // Descriptor for the dialogue of the current locks
	self.displayed_locks = new Array(); // Data about the displayed locks
	
	// Set the locks from given data
	self.showLocks = function(dialogue_descriptor, locks_to_display)
	{
		emptyNode(self.graphics_chains);
		emptyNode(self.graphics_locks);
		
		self.current_locks_dialogue_desc = dialogue_descriptor;
		self.displayed_locks = new Array();
		
		for(var i = 0; i < locks_to_display.length; i++)
		{
			var chain = generateImageElement(cfg.picture_dir + cfg.locks_subdir + 'fg_chains_appear.gif?id=' + locks_to_display[i].id);
			var lock = generateImageElement(cfg.picture_dir + cfg.locks_subdir + locks_to_display[i].type + '_appears.gif?id=' + locks_to_display[i].id);
			
			var left_pos = locks_to_display[i].x - 256;
			var top_pos = locks_to_display[i].y - 192;
			
			chain.style.left = (lock.style.left = left_pos + 'px');
			chain.style.top = (lock.style.top = top_pos + 'px');
			
			self.graphics_chains.appendChild(chain);
			self.graphics_locks.appendChild(lock);
			
			self.displayed_locks.push(new Object({
				id: locks_to_display[i].id,
				type: locks_to_display[i].type,
				x: locks_to_display[i].x,
				y: locks_to_display[i].y,
				graphic_chain: chain,
				graphic_lock: lock,
				broken: false
			}));
			
			// If lock already broken (eg when restoring state save) perform breakLock immediately
			if(('broken' in locks_to_display[i]) && locks_to_display[i].broken)
			{
				self.breakLock(dialogue_descriptor, locks_to_display[i].id);
			}
		}
	};
	
	// Break a lock
	self.breakLock = function(dialogue_descriptor, lock_id)
	{
		if(objCompare(self.current_locks_dialogue_desc, dialogue_descriptor))
		{
			// Only work if for the currently displayed locks - do nothing otherwise
			
			var lock_data;
			if(lock_id == 0)
			{
				// Auto mode : search for first unbroken lock
				var index = 0;
				while(index < self.displayed_locks.length && self.displayed_locks[index].broken)
				{
					index++;
				}
				lock_data = self.displayed_locks[index];
			}
			else
			{
				// Search for lock of given id
				var index = 0;
				while(index < self.displayed_locks.length && self.displayed_locks[index].id != lock_id)
				{
					index++;
				}
				lock_data = self.displayed_locks[index];
			}
			
			if(lock_data && !lock_data.broken)
			{
				// Only act if lock not broken yet
				
				// Generate breaking lock animation
				var new_lock = generateImageElement(cfg.picture_dir + cfg.locks_subdir + lock_data.type + '_explodes.gif?id=' + lock_data.id);
				new_lock.style.left = (lock_data.x - 256) + 'px';
				new_lock.style.top = (lock_data.y - 192) + 'px';
				
				// Replace the picture
				self.graphics_locks.replaceChild(new_lock, lock_data.graphic_lock);
				lock_data.graphic_lock = new_lock;
				
				// Note down lock as broken
				lock_data.broken = true;
			}
		}
	};
	
	// Hide all locks
	self.hideLocks = function(animate)
	{
		emptyNode(self.graphics_locks);
		
		if(animate)
		{
			for(var i = 0; i < self.displayed_locks.length; i++)
			{
				var new_chain = generateImageElement(cfg.picture_dir + cfg.locks_subdir + 'fg_chains_disappear.gif?id=' + self.displayed_locks[i].id);
				new_chain.style.left = (self.displayed_locks[i].x - 256) + 'px';
				new_chain.style.top = (self.displayed_locks[i].y - 192) + 'px';
				
				self.graphics_chains.replaceChild(new_chain, self.displayed_locks[i].graphic_chain);
				self.displayed_locks[i].graphic_chain = new_chain;
			}
		}
		else
		{
			emptyNode(self.graphics_chains);
		}
		
		self.displayed_locks = new Array();
		self.current_locks_dialogue_desc = null;
	};
	
	// Export and restore state of locks display engine.
	Object.defineProperty(self, 'state', {
		get: function()
		{
			return {
				current_locks_dialogue_desc: self.current_locks_dialogue_desc,
				// Remove links to physical objects from descriptors.
				displayed_locks: self.displayed_locks.map(function(lock_descriptor) {
					return {
						id: lock_descriptor.id,
						type: lock_descriptor.type,
						x: lock_descriptor.x,
						y: lock_descriptor.y,
						broken: lock_descriptor.broken
					};
				})
			};
		},
		set: function(state)
		{
			self.showLocks(state.current_locks_dialogue_desc, state.displayed_locks);
		}
	});
}

//END OF MODULE
Modules.complete('display_engine_locks');
