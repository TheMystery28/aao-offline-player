"use strict";
/*
Ace Attorney Online - Top screen display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_screen',
	dependencies : ['display_engine_globals', 'display_engine_text', 'display_engine_place', 'display_engine_characters', 'display_engine_popups', 'display_engine_locks', 'display_engine_cr_icons', 'display_engine_effects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
function ScreenDisplay()
{
	var self = this;
	self.render_buffer = new CallbackBuffer();
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_screen');
	self.viewport = document.createElement('div');
	addClass(self.viewport, 'viewport');
	self.render.appendChild(self.viewport);
	self.place_display = new PlaceDisplay(self.render_buffer);
	self.viewport.appendChild(self.place_display.render);
	self.characters_display = new CharactersDisplay(self.render_buffer);
	self.place_display.contents.appendChild(self.characters_display.render);
	self.locks_display = new LocksDisplay();
	self.render.appendChild(self.locks_display.render);
	self.cr_icons_display = new CrIconsDisplay(self);
	self.render.appendChild(self.cr_icons_display.render);
	self.popups_display = new PopupsDisplay(self.render_buffer);
	self.render.appendChild(self.popups_display.render);
	self.screen_fade_display = new ScreenFadeDisplay(self, self.render_buffer);
	self.text_display = new TextDisplay(self.characters_display, self);
	self.render.appendChild(self.text_display.render);
	self.overlay = document.createElement('div');
	addClass(self.overlay, 'overlay');
	self.render.appendChild(self.overlay);
	
	self.currentPositionData = getPosition(POSITION_CENTER); // Current position of the screen. Center by default.
	
	// PRIVATE METHODS
	
	// Get a position by ID, handling dynamic positioning modes for the scene
	self.getPosition = function(position_id, place_data, character_id)
	{
		switch(position_id)
		{
			case POSITION_NONE: // Align on talking : remove shift from character position
				// Position of the currently talking character
				var character_position = self.characters_display.getCharacterPosition(character_id);
				if(character_position)
				{
					// Remove shift
					var target_position = objClone(character_position);
					target_position.shift = 0;
					return target_position;
				}
				else
				{
					// Talking character isn't on the scene : return current screen position
					return self.currentPositionData;
				}
				break;
			
			case POSITION_CENTER_ON_TALKING:
				var character_position = self.characters_display.getCharacterPosition(character_id);
				return character_position || self.currentPositionData;
				break;
			
			case POSITION_DO_NOT_MOVE: // Current screen position
				return self.currentPositionData;
				break;
			
			default:
				return getPosition(position_id, place_data);
				break;
		}
	};
	
	// Set the position of the screen over the scene
	self.setScreenPosition = function(position, transition)
	{
		self.currentPositionData = position;
		self.characters_display.currentDefaultPosition = position.id;
		
		var aligned_leftpos;
		switch(position.align)
		{
			case ALIGN_LEFT :
				aligned_leftpos = 0;
				break;
			
			case ALIGN_CENTER :
				aligned_leftpos = - Math.floor((self.place_display.render.clientWidth / 2) - (self.viewport.clientWidth / 2));
				break;
			
			case ALIGN_RIGHT : 
				aligned_leftpos = - Math.floor(self.place_display.render.clientWidth - self.viewport.clientWidth);
				break;
		}
		
		if(transition != TRANSITION_NO)
		{
			// If transition is enabled, set it up.
			var transition_duration = 0.5;
			var timing_fct = null;
			switch(transition)
			{
				case TRANSITION_LINEAR:
					timing_fct = 'linear';
					break;
				case TRANSITION_BEZIER:
					timing_fct = 'cubic-bezier(.7,.01,.3,1)';
					break;
				case TRANSITION_EASE_IN:
					timing_fct = 'cubic-bezier(.6,0,1,1)';
					break;
				case TRANSITION_EASE_OUT:
					timing_fct = 'cubic-bezier(0,0,.4,1)';
					break;
			}
			setTransition(self.place_display.render, 'left', true, transition_duration, timing_fct);
		}
		
		self.place_display.render.style.left = (aligned_leftpos - position.shift) + 'px';
	};
	
	// PUBLIC METHODS
	
	// Attach variable environment
	self.setVariableEnvironment = function(var_env)
	{
		return self.text_display.setVariableEnvironment(var_env);
	};
	self.getVariableEnvironment = function()
	{
		return self.text_display.getVariableEnvironment();
	};
	
	// Load graphics a given frame
	self.loadFrameGraphics = function(frame_data, clear_previous, callback)
	{
		if(clear_previous)
		{
			//if needed, remove all properties of the previous frame
			self.clearScreen();
		}
		
		var place_data = getPlace(frame_data.place);
		
		//First, load the place.
		var place_changed = self.place_display.setPlace(place_data, function(place_changed) {
			// When place is loaded, set the position of the screen over the place (Transition only if did not change place).
			self.setScreenPosition(self.getPosition(frame_data.place_position, place_data, frame_data.speaker_id), frame_data.place_transition * !place_changed);
			// Update displayed CR icons, now that the position is updated, and auto-position can run properly.
			self.iconsUpdate();
		});
		
		// Then load the popups
		self.popups_display.loadFramePopups(frame_data);
		
		// Then load the characters and callback
		self.characters_display.loadFrameCharacters(frame_data, !place_changed, callback);
		
		// Then fade in or out
		self.screenFadeUpdate(frame_data);
	};
	
	// Load a given frame
	self.loadFrame = function(frame_data, clear_previous, callback)
	{
		// Init the textbox for this frame
		self.text_display.initLoadFrame(frame_data);
		
		// Load the graphics from the frame
		self.loadFrameGraphics(frame_data, clear_previous, function() {
			// Then play the text
			self.text_display.loadFrameText(frame_data, callback);
		});
	};
	
	// Clear all characters on screen
	self.clearCharacters = function()
	{
		self.characters_display.removeCharacters();
	};
	
	// Clear the place on the screen
	self.unsetPlace = function()
	{
		self.place_display.unsetPlace();
	};
	
	// Clear the textbox and stop typing
	self.clearText = function()
	{
		self.text_display.clearText();
	};
	
	// Clear all displayed CR icons
	self.clearIcons = function()
	{
		self.cr_icons_display.clearIcons();
	};

	// Clear the screen fade
	self.clearScreenFade = function()
	{
		self.screen_fade_display.clearScreenFade();
	};
	
	// Clear the whole screen
	self.clearScreen = function()
	{
		self.clearCharacters();
		self.unsetPlace();
		self.clearText();
		self.clearIcons();
		self.clearScreenFade();
	};
	
	// Skip text typing
	self.skip = function()
	{
		self.text_display.skip();
	};
	
	// Set instant typing mode
	self.setInstantMode = function(enabled)
	{
		self.text_display.setInstantMode(enabled);
	};
	
	// Update visibility of all objects on current place
	self.refreshPlaceObjects = function()
	{
		self.place_display.refreshObjects();
	};
	
	// Manage CR icons displayed on screen
	self.iconsChanges = new Object({
		clear: false,
		show: new Array()
	});
	self.iconsPrepareClear = function()
	{
		self.iconsChanges.clear = true;
	};
	self.iconsPrepareAdd = function(icon)
	{
		self.iconsChanges.show.push(icon);
	};
	self.iconsUpdate = function()
	{
		if(self.iconsChanges.clear)
		{
			self.cr_icons_display.clearIcons(self.iconsChanges.show);
			self.iconsChanges.clear = false;
		}
		
		for(var i = 0; i < self.iconsChanges.show.length; i++)
		{
			self.cr_icons_display.showCrIcon(self.iconsChanges.show[i]);
		}
		self.iconsChanges.show = new Array();
	};
	
	self.screenFadeUpdate = function(frame_data) 
	{
		var fade_data = frame_data.fade;

		if(!fade_data) 
		{
			self.screen_fade_display.clearScreenFade();
		}
		else
		{
			self.screen_fade_display.setScreenFade(fade_data.fade_duration, fade_data.fade_colour, fade_data.fade_type, fade_data.fade_placement);
		}
	};
	
	// Export and restore state of screen display engine.
	Object.defineProperty(self, 'state', {
		get: function()
		{
			return {
				position: self.currentPositionData,
				place: {
					place_id: self.place_display.place_id
				},
				characters: self.characters_display.state,
				locks: self.locks_display.state,
				cr_icons: self.cr_icons_display.state,
				popups: self.popups_display.state,
				text: self.text_display.state,
				fade: self.screen_fade_display.state
			};
		},
		set: function(state)
		{
			// Setting the place display requires a callback for actions waiting on the place to load.
			self.place_display.setPlace(getPlace(state.place.place_id), function(){
				self.setScreenPosition(state.position);
				self.cr_icons_display.state = state.cr_icons;
			});
			
			self.characters_display.state = state.characters;
			self.locks_display.state = state.locks;
			self.popups_display.state = state.popups;
			self.text_display.state = state.text;
			self.screen_fade_display.state = state.fade;
		}
	});
}

//END OF MODULE
Modules.complete('display_engine_screen');
