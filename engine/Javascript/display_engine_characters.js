"use strict";
/*
Ace Attorney Online - Characters display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_characters',
	dependencies : ['display_engine_globals', 'style_loader', 'events', 'objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS
var OUT_OF_VIEW_POSITION_LEFT = {
	align: ALIGN_LEFT,
	shift: -256 // TODO : un-hardcode element width
};

var OUT_OF_VIEW_POSITION_RIGHT = {
	align: ALIGN_RIGHT,
	shift: 256 // TODO : un-hardcode element width
};

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
function CharactersDisplay(render_buffer)
{
	var self = this;
	self.render_buffer = render_buffer || new CallbackBuffer();
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_characters');
	
	self.currentDefaultPosition = POSITION_CENTER; // Position to insert new unpositioned character. Typically current screen position.
	self.currentPositions = new Object(); // Current positions by ID
	self.charactersById = new Object(); // Character objects present in the scene by character ID
	self.charactersInfoById = new Object(); // Character info by ID
	self.charactersIsTalkingById = new Object();
	self.charactersIsInStartupById = new Object();
	self.charactersIdInDisplayOrder = new Array(); // Characters ID given in the order in which they are displayed (first is displayed in the back)
	
	// Reset the current positions of the scene to the default ones
	self.resetCurrentPositions = function()
	{
		self.currentPositions = new Object();
		for(var i in default_positions)
		{
			self.currentPositions[default_positions[i].id] = default_positions[i];
		}
	};
	self.resetCurrentPositions();
	
	// Get the current position of a character on the scene; null if character isn't on the scene
	self.getCharacterPosition = function(character_id)
	{
		var character_info = self.charactersInfoById[character_id];
		if(character_info)
		{
			return self.currentPositions[character_info.position];
		}
		else
		{
			return null;
		}
	};
	
	// Set display of all characters modified by the frame (either added, removed or updated).
	self.setCharacters = function(frame_characters, frame_characters_erase_previous, transition_enabled, callback)
	{
		// Build O(1) lookup maps from frame_characters for profile_id and position
		var frameCharByProfileId = {};
		var framePositions = {};
		for(var j = 0; j < frame_characters.length; j++)
		{
			frameCharByProfileId[frame_characters[j].profile_id] = frame_characters[j];
			framePositions[frame_characters[j].position] = true;
		}

		// Go through all characters already present in the display, and select appropriate action.
		for(var id in self.charactersInfoById)
		{
			var char_info = self.charactersInfoById[id];
			var defined_char_info = frameCharByProfileId[id];

			if(defined_char_info)
			{
				// If the character is defined in the new frame, update it with the new data.
				self.updateCharacter(defined_char_info, transition_enabled, callback);
			}
			else if(framePositions[char_info.position] || frame_characters_erase_previous)
			{
				// Else, if the character is overridden by another in the frame, or "erase previous" is set, the character is removed.
				self.removeCharacter(char_info, transition_enabled);
			}
		}

		// Go through all characters from the frame not present in the display, and add it.
		for(var j = 0; j < frame_characters.length; j++)
		{
			var char_info = frame_characters[j];

			if(!(char_info.profile_id in self.charactersInfoById))
			{
				self.addCharacter(char_info, transition_enabled, callback);
			}
		}
	};
	
	self.addCharacter = function(character_info, transition_enabled, callback)
	{
		// Default pose : talking if manually synced to always talking; still otherwise (will be made to talk if needed by the sync)
		var default_char_pose = character_info.sync_mode == SYNC_TALK ? 'talking' : 'still';
		
		// Create new image object
		var descriptor = getCharacterDescriptor(character_info, default_char_pose);
		var newElement = generateGraphicElement(descriptor, function(img, newElement){
			
			// Set mirror effect on the element.
			setEffectToGraphicElement(newElement, 'mirror', character_info.mirror_effect);
			
			var target_position = self.currentPositions[character_info.position];
			var playStartupAndCallback = self.playStartup.bind(self, character_info, callback);
			
			// Run the apparition animation.
			var appears_transition = transition_enabled ? character_info.visual_effect_appears : EFFECT_NONE;
			switch(appears_transition)
			{
				case EFFECT_FADING:
					// Fade in effect.
					
					// Add element in initial state.
					newElement.style.opacity = 0;
					setGraphicElementPosition(newElement, target_position, self.render);
					self.render.appendChild(newElement);
					
					var transition_duration = 0.3;
					
					// Buffer an action to transition to target state.
					self.render_buffer.callWithLatency(function() {
						// Set transitions.
						setTransition(newElement, 'opacity', true, transition_duration);
						
						// Set target state
						newElement.style.opacity = 1;
						
						// After transition, proceed to the next step.
						self.render_buffer.timer(playStartupAndCallback, transition_duration * 1000);
					});
					
					break;
				
				case EFFECT_SLIDE_LEFT:
					// Slide in from the left.
					
					// Add element in initial state.
					setGraphicElementPosition(newElement, OUT_OF_VIEW_POSITION_LEFT, self.render);
					self.render.appendChild(newElement);
					
					var transition_duration = 0.5;
					
					// Buffer an action to transition to target state.
					self.render_buffer.callWithLatency(function() {
						// Set transitions.
						setTransition(newElement, 'left', true, transition_duration);
						setTransition(newElement.firstChild, 'left', true, transition_duration);
						
						// Set target state
						setGraphicElementPosition(newElement, target_position, self.render);
						
						// After transition, proceed to the next step.
						self.render_buffer.timer(playStartupAndCallback, transition_duration * 1000);
					});
					
					break;
				
				case EFFECT_SLIDE_RIGHT:
					// Slide in from the right.
					
					// Add element in initial state.
					setGraphicElementPosition(newElement, OUT_OF_VIEW_POSITION_RIGHT, self.render);
					self.render.appendChild(newElement);
					
					var transition_duration = 0.5;
					
					// Buffer an action to transition to target state.
					self.render_buffer.callWithLatency(function() {
						// Set transitions.
						setTransition(newElement, 'left', true, transition_duration);
						setTransition(newElement.firstChild, 'left', true, transition_duration);
						
						// Set target state
						setGraphicElementPosition(newElement, target_position, self.render);
						
						// After transition, proceed to the next step.
						self.render_buffer.timer(playStartupAndCallback, transition_duration * 1000);
					});
					
					break;
				
				case EFFECT_NONE:
				default:
					// Unhandled effect or no effect : just appear instantaneously at the target position.
					setGraphicElementPosition(newElement, target_position, self.render);
					self.render.appendChild(newElement);
					
					// Buffer a call to continue.
					self.render_buffer.call(playStartupAndCallback);
					break;
			}
			
			// Set the character as displayed.
			self.charactersInfoById[character_info.profile_id] = character_info;
			
			// TODO : add option "play frame after move" and "play frame during move", and trigger callback as appropriate for each.
			
		}, self.render_buffer);
		
		// Update state variables with the new character.
		self.charactersById[character_info.profile_id] = newElement;
		self.charactersIdInDisplayOrder.push(character_info.profile_id);
	};
	
	self.updateCharacter = function(character_info, transition_enabled, callback)
	{
		// Default pose : talking if manually synced to always talking; still otherwise (will be made to talk if needed by the sync)
		var default_char_pose = character_info.sync_mode == SYNC_TALK ? 'talking' : 'still';
		
		// Update the sprite, then the position.
		self.setCharacterStatus(character_info, default_char_pose, function(){ 
			self.moveCharacter(character_info, transition_enabled, self.playStartup.bind(self, character_info, callback));
		});
	};
	
	// Trigger transition for and delete disappearing character
	self.removeCharacter = function(character_info, transition_enabled)
	{
		var element = self.charactersById[character_info.profile_id];
		
		var removeElement = self.render.removeChild.bind(self.render, element);
		
		// Run the disapparition animation.
		var disappears_transition = transition_enabled ? character_info.visual_effect_disappears : EFFECT_NONE;
		switch(disappears_transition)
		{
			case EFFECT_FADING:
				// Fade out effect.
				
				// Set one time transition for the element.
				var transition_duration = 0.3;
				
				setTransition(element, 'opacity', true, transition_duration);
				element.style.opacity = 0;
					
				self.render_buffer.timer(removeElement, transition_duration * 1000);
				
				break;
			
			case EFFECT_SLIDE_LEFT:
				// Slide out to the left.
				
				// Set element to be just on the left.
				var target_position = OUT_OF_VIEW_POSITION_LEFT;
				
				var transition_duration = 0.5;
				
				setTransition(element, 'left', true, transition_duration);
				setTransition(element.firstChild, 'left', true, transition_duration);
				setGraphicElementPosition(element, target_position, self.render);
				
				self.render_buffer.timer(removeElement, transition_duration * 1000);
				
				break;
			
			case EFFECT_SLIDE_RIGHT:
				// Slide out to the right.
				
				// Set element to be just on the right.
				var target_position = OUT_OF_VIEW_POSITION_RIGHT;
				
				var transition_duration = 0.5;
				
				setTransition(element, 'left', true, transition_duration);
				setTransition(element.firstChild, 'left', true, transition_duration);
				setGraphicElementPosition(element, target_position, self.render);
				
				self.render_buffer.timer(removeElement, transition_duration * 1000);
				
				break;
			
			case EFFECT_NONE:
			default:
				// Unhandled effect or no effect : just disappear instantaneously.
				self.render_buffer.call(removeElement);
				break;
		}
		
		//Remove character data
		delete self.charactersById[character_info.profile_id];
		delete self.charactersInfoById[character_info.profile_id];
		delete self.charactersIsInStartupById[character_info.profile_id];
		delete self.charactersIsTalkingById[character_info.profile_id];
		
		// Remove character from display order list.
		self.charactersIdInDisplayOrder.splice(self.charactersIdInDisplayOrder.indexOf(character_info.profile_id), 1);
	};
	
	// Update the position of a character.
	// Assumes the character is already displayed.
	self.moveCharacter = function(character_info, transition_enabled, callback)
	{
		var element = self.charactersById[character_info.profile_id];
		
		//Set mirror status
		setEffectToGraphicElement(element, 'mirror', character_info.mirror_effect);
		
		var current_position = self.charactersInfoById[character_info.profile_id];
		var position_changed = (character_info.position != current_position.position);
		
		//Update character info.
		self.charactersInfoById[character_info.profile_id] = character_info;

		var position = self.currentPositions[character_info.position];
		//Now to move, if needed.
		if(position_changed && transition_enabled)
		{
			// If we have a transition to display...
			
			// Set up the transition.
			var transition_duration = 0.5;
			setTransition(element, 'left', true, transition_duration);
			setTransition(element.firstChild, 'left', true, transition_duration);
			
			// Move to target position.
			setGraphicElementPosition(element, position, self.render);
			
			// At the end of the move, callback.
			if(callback)
			{
				self.render_buffer.timer(callback, transition_duration * 1000);
			}
		}
		else
		{
			// Else, just move and callback.
			setGraphicElementPosition(element, position, self.render);
			if(callback)
			{
				self.render_buffer.callWithLatency(callback);
			}
		}
		// TODO : add option "play frame after move" and "play frame during move", and trigger callback as appropriate for each.
	};
	
	// Remove all displayed characters.
	self.removeCharacters = function()
	{
		for(var i in self.charactersInfoById)
		{
			self.removeCharacter(self.charactersInfoById[i]);
		}
	};
	
	// Change the animation status of a character on the scene
	self.setCharacterStatus = function(character_info, status, callback)
	{
		var element = self.charactersById[character_info.profile_id];
		
		updateGraphicElement(element, getCharacterDescriptor(character_info, status), callback, self.render_buffer);
	};
	
	// Trigger talking sprite for all synchronised characters
	self.textSyncStart = function(speaker_id)
	{
		for(var i in self.charactersInfoById)
		{
			var char_info = self.charactersInfoById[i];
			if(char_info.sync_mode == SYNC_SYNC || 
				(char_info.sync_mode == SYNC_AUTO && char_info.profile_id == speaker_id))
			{
				if(!self.charactersIsInStartupById[i])
				{
					self.setCharacterStatus(char_info, 'talking');
				}
				self.charactersIsTalkingById[i] = true;
			}
		}
	};
	
	// Trigger still sprite for all synchronised characters
	self.textSyncStop = function()
	{
		for(var i in self.charactersInfoById)
		{
			var char_info = self.charactersInfoById[i];
			if(char_info.sync_mode == SYNC_SYNC || char_info.sync_mode == SYNC_AUTO)
			{
				if(!self.charactersIsInStartupById[i])
				{
					self.setCharacterStatus(char_info, 'still');
				}
				self.charactersIsTalkingById[i] = false;
			}
		}
	};
	
	self.playStartup = function(character_info, callback)
	{
		// Restores the synchronisation of the character after the startup annimation
		function resumeSync(character_info)
		{
			self.charactersIsInStartupById[character_info.profile_id] = false;
			
			switch(character_info.sync_mode)
			{
				case SYNC_SYNC:
				case SYNC_AUTO:
					
					if(self.charactersIsTalkingById[character_info.profile_id])
					{
						self.setCharacterStatus(character_info, 'talking');
					}
					else
					{
						self.setCharacterStatus(character_info, 'still');
					}
				
					break;
				
				case SYNC_TALK:
					self.setCharacterStatus(character_info, 'talking');
					break;
				
				case SYNC_STILL:
					self.setCharacterStatus(character_info, 'still');
					break;
			}
		}
		
		var pose_desc = getPoseDesc(character_info);
		
		if(character_info.startup_mode == STARTUP_SKIP
			|| pose_desc.startup_duration == 0 )
		{
			// In this mode, do not play startup and run callback immediately
			if(callback)
			{
				callback();
			}
		}
		else
		{
			// Launch startup sprite
			self.charactersIsInStartupById[character_info.profile_id] = true;
			self.setCharacterStatus(character_info, 'startup');
			
			if(character_info.startup_mode == STARTUP_PLAY_BEFORE)
			{
				// In this mode, delay callback to after the startup animation is complete
				self.render_buffer.timer(function(){
					resumeSync(character_info);
					if(callback)
					{
						callback();
					}
				}, pose_desc.startup_duration);
			}
			else
			{
				// In this mode, run callback immediately
				self.render_buffer.timer(function(){
					resumeSync(character_info);
				}, pose_desc.startup_duration);
				
				if(callback)
				{
					callback();
				}
			}
		}
	};
	
	// Get the information about the frame's characters.
	// Resolve actual runtime values for dynamic settings.
	self.getCharactersInfo = function(frame_characters)
	{
		var characters = new Array();
		
		for(var i = 0; i < frame_characters.length; i++)
		{
			//Clone character info to avoid side effects from other functions
			var character_info = new objClone(frame_characters[i]);
			var to_be_pushed = true; // Determine if character actually has to be displayed on screen
			
			// Handle unpositioned characters
			if(character_info.position == POSITION_NONE)
			{
				//if position is none
				if(self.charactersInfoById[character_info.profile_id])
				{
					//and character was already displayed, set position to the same
					character_info.position = self.charactersInfoById[character_info.profile_id].position;
				}
				else
				{
					// New unpositioned character : shouldn't happen in V6 games, but frequent in legacy trials
					// Set character to current screen position
					character_info.position = self.currentDefaultPosition;
				}
			}
			
			// Handle characters with no sprite set
			if(character_info.sprite_id == 0)
			{
				if(self.charactersInfoById[character_info.profile_id])
				{
					//If character was already displayed, set sprite to the same
					character_info.sprite_id = self.charactersInfoById[character_info.profile_id].sprite_id;
				}
				else
				{
					//Otherwise, it won't be displayed : remove from the list
					to_be_pushed = false;
				}
			}
			
			// Handle characters with "auto" appearance effect set - possible in legacy files.
			if(character_info.visual_effect_appears == EFFECT_AUTO)
			{
				// Set no effect instead.
				character_info.visual_effect_appears = EFFECT_NONE;
			}
			
			// Handle characters with "auto" disappearance effect set.
			if(character_info.visual_effect_disappears == EFFECT_AUTO)
			{
				if(self.charactersInfoById[character_info.profile_id])
				{
					// If character already displayed, fetch its previously set disappearance effect.
					character_info.visual_effect_disappears = self.charactersInfoById[character_info.profile_id].visual_effect_disappears;
				}
				else
				{
					// If not displayed yet, default to no effect.
					character_info.visual_effect_disappears = EFFECT_NONE;
				}
			}
			
			// Add character to list if needed
			if(to_be_pushed)
			{
				characters.push(character_info);
			}
		}
		
		return characters;
	};
	
	// Load characters from frame data
	self.loadFrameCharacters = function(frame_data, transition_enabled, callback)
	{
		// If there is a place set, update the positions list
		if(frame_data.place)
		{
			self.resetCurrentPositions();
			var place_data = getPlace(frame_data.place);
			for(var i = 0; i <= place_data.positions.length - 1; i++)
			{
				self.currentPositions[place_data.positions[i].id] = place_data.positions[i];
			}
			
			// TODO : decide what to do if characters haven't been cleared - should characters on a place-specific position be cleared anyway ?
		}
		
		// Get characters information
		var characters = self.getCharactersInfo(frame_data.characters); 
		
		var loaded_characters = -1; // -1, so that we can trigger the function immediately after
		var checkAndRunCallback = callback ? (function() {
			if(++loaded_characters == characters.length)
			{
				// If all characters are loaded, run callback
				callback();
			}
		}) : null;
		if(checkAndRunCallback)
		{
			checkAndRunCallback(); // Trigger the function, even if there is no character
		}
		
		// Set all characters.
		self.setCharacters(characters, frame_data.characters_erase_previous, transition_enabled, checkAndRunCallback);
	};
	
	// Export and restore state of characters display engine.
	Object.defineProperty(self, 'state', {
		get: function()
		{
			return {
				currentDefaultPosition: self.currentDefaultPosition,
				currentPositions: self.currentPositions,
				characters: self.charactersInfoById,
				characters_order: self.charactersIdInDisplayOrder
			};
		},
		set: function(state)
		{
			self.removeCharacters();
			
			self.currentDefaultPosition = state.currentDefaultPosition;
			self.currentPositions = state.currentPositions;
			
			// Erase all previous characters and do not use animation transitions.
			var character_array = state.characters_order.map(x => state.characters[x]);
			self.setCharacters(character_array, true, false);
		}
	});
}

//END OF MODULE
Modules.complete('display_engine_characters');
