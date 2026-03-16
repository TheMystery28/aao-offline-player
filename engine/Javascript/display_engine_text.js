"use strict";
/*
Ace Attorney Online - Text display engine

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'display_engine_text',
	dependencies : ['style_loader', 'sound-howler', 'nodes', 'events', 'objects'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

// Precompiled regex patterns for typeText hot path (avoid per-character allocation)
var TAG_PATTERN = /(\[#(\/?)(.*?)\])|(\[\/#\])/g;
var BLIP_CHAR_PATTERN = /[^\s,;-]/i;
var WHITESPACE_PATTERN = /\s/;

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
// Text display engine. Can be linked to a characterDisplay object to enable lip syncronisation and to a ScreenDisplay object to enable effects
function TextDisplay(character_display, screen_display)
{
	var self = this;
	
	// DOM structure
	self.render = document.createElement('div'); // Parent element to be included in the page
	addClass(self.render, 'display_engine_text');
	
	self.text_box = document.createElement('div');
	addClass(self.text_box, 'textbox');
	addClass(self.text_box, 'bottom'); // Displayed at the bottom by default
	self.dialogue_box = document.createElement('div');
	addClass(self.dialogue_box, 'dialogue');
	self.text_box.appendChild(self.dialogue_box);
	self.name_box = document.createElement('div');
	addClass(self.name_box, 'name');
	self.text_box.appendChild(self.name_box);
	self.render.appendChild(self.text_box);
	
	// Attach variable environment
	self.var_env = new VariableEnvironment(); // Empty var env by default
	self.setVariableEnvironment = function(var_env)
	{
		self.var_env = var_env;
	};
	self.getVariableEnvironment = function()
	{
		return self.var_env;
	};
	
	self.previous_frame_merged = false;
	self.current_speaker;
	self.current_voice_id;
	self.voices = new Object({
	});
	
	self.is_talking = false;
	self.timeSinceBlip = 0;
	
	self.startTalking = function()
	{
		if(!self.is_talking)
		{
			if(character_display)
			{
				character_display.textSyncStart(self.current_speaker);
			}
			
			self.is_talking = true;
		}
	};
	
	self.stopTalking = function()
	{
		if(character_display)
		{
			character_display.textSyncStop();
		}
		
		self.is_talking = false;
	};
	
	// Check if text is empty
	self.isEmptyText = function(text_content)
	{
		// Delete all tags from the text and trim it
		var fake_container = document.createElement('div');
		self.instantTypeText(fake_container, text_content);
		var clean_text_contents = fake_container.textContent.trim();
		
		return clean_text_contents.length == 0; // If no character left, string is empty
	};
	
	self.skip_remaining_text = false; // Temporary instant mode, only for current frame
	self.instant_mode_enabled = false; // Permanent instant mode, active until disabled manually
	
	// Set instant typing mode
	self.setInstantMode = function(enabled)
	{
		self.instant_mode_enabled = enabled;
	};
	
	self.decodeFirstTag = function(text, only_at_start)
	{
		/* Tag Pattern construction :
		 *	(							| Capture opening or self-contained tag
		 * 		\[#
		 * 			(\/?)				| Capture content tag mark
		 * 			(.*?)				| Capture opening tag definition
		 *		\]						| Ends with closing bracket
		 * 	)
		 * 	|							| Or
		 * 	(							| Capture closing tag
		 * 		\[\/#\]
		 * 	)
		 */
		// Reset precompiled pattern (stateful with /g flag)
		TAG_PATTERN.lastIndex = 0;

		// First, match it once to find first tag
		var match = TAG_PATTERN.exec(text);
		if(match !== null && (!only_at_start || match.index == 0))
		{
			// A tag was found
			if(match[4])
			{
				// First tag is a closing tag ? Forbidden, throw exception
				throw 'Unexpected closing tag';
			}
			else
			{
				var text_before = text.substring(0, match.index);
				var tag_definition = match[3];
				
				if(!match[2])
				{
					// First tag is a self-contained tag : return immediately with empty contents.
					return new Object({
						text_before: text_before,
						last_index: TAG_PATTERN.lastIndex,
						tag_definition: tag_definition,
						tag_contents: ''
					});
				}
				else
				{
					// First tag is a content tag : fetch contents
					var tag_contents_start_index = match.index + match[0].length;
					
					var nest_level = 1;
					while(nest_level > 0 && ((match = TAG_PATTERN.exec(text)) !== null))
					{
						if(match[4])
						{
							// A closing tag : decrease nesting level.
							nest_level--;
						}
						else if(match[2])
						{
							// An opening tag : increase nesting level.
							nest_level++;
						}
					}
					
					if(nest_level > 0)
					{
						// No more tag but still in nested environment ? Throw !
						alert(l('text_engine_missing_closing_tag'))
						throw 'Missing expected closing tag';
					}
					else if(nest_level < 0)
					{
						// Unmatched closing tag ? Throw !
						throw 'Unexpected closing tag';
					}
					else
					{
						// Matched with correct end tag : return
						return new Object({
							text_before: text_before,
							last_index: TAG_PATTERN.lastIndex,
							tag_definition: tag_definition,
							tag_contents: text.substring(tag_contents_start_index, match.index)
						});
					}
				}
			}
		}
		
		// No tag matched : return null
		return null;
	};
	
	// Type text instantly in given container and run callback
	self.instantTypeText = function(container, remaining_text, callback)
	{
		self.stopTalking();
		
		// Loop through all tags
		var decoded_tag;
		while(decoded_tag = self.decodeFirstTag(remaining_text))
		{
			appendNodeTextContents(container, decoded_tag.text_before);
			
			// Check tag type
			var colon_position = decoded_tag.tag_definition.indexOf(':');
			if(colon_position >= 0)
			{
				// Colon found : long tag
				var tag_name = decoded_tag.tag_definition.substr(0, colon_position);
				var tag_value = decoded_tag.tag_definition.substr(colon_position + 1);
				
				switch(tag_name)
				{
					case 'instant':
					case 'colour':
						var inner_container = document.createElement('span');
						self.instantTypeText(inner_container, decoded_tag.tag_contents);
						if(tag_value)
						{
							inner_container.style.color = tag_value;
						}
						container.appendChild(inner_container);
						break;
					
					case 'var':
						appendNodeTextContents(container, self.getVariableEnvironment().get(tag_value));
						break;
				}
			}
			
			// Short tags have no effect
			
			remaining_text = remaining_text.substr(decoded_tag.last_index);
		}
		
		// After last tag, append everything
		appendNodeTextContents(container, remaining_text);
		
		if(callback)
		{
			window.setTimeout(callback, 0);
		}
	};
	
	// Type text in given container and run callback when done
	self.typeText = function(container, remaining_text, speed, callback)
	{
		if(self.skip_remaining_text || self.instant_mode_enabled)
		{
			self.instantTypeText(container, remaining_text, callback);
		}
		else
		{
			if(remaining_text.length > 0)
			{
				// If there is text left to display

				// Early exit: skip tag decoding if no tag marker present
				var first_tag = remaining_text.indexOf('[#') === 0 ? self.decodeFirstTag(remaining_text, true) : null;
				
				if(first_tag)
				{
					// Variables that can be altered by tags
					var next_remaining_text = remaining_text.substr(first_tag.last_index); // Text to write next : by default what is after the tag
					var next_delay = 133 / speed; // Time until next character displayed : by default 133ms relative to speed
					
					// Check tag type
					var colon_position = first_tag.tag_definition.indexOf(':');
					if(colon_position >= 0)
					{
						// Colon found : long tag
						var tag_name = first_tag.tag_definition.substr(0, colon_position);
						var tag_value = first_tag.tag_definition.substr(colon_position + 1);
						
						switch(tag_name)
						{
							case 'instant':
								// Instantaneous text display with colour setting
								
								var inner_container = document.createElement('span');
								self.instantTypeText(inner_container, first_tag.tag_contents);
								if(tag_value)
								{
									inner_container.style.color = tag_value;
								}
								container.appendChild(inner_container);
								
								// Instantaneous text : no pause
								next_delay = 0;
								
								break;
							
							case 'colour':
								// Handle non-instantaneous colour tags
								
								// Colour container
								var inner_container = document.createElement('span');
								if(tag_value)
								{
									inner_container.style.color = tag_value;
								}
								container.appendChild(inner_container);
								
								// Type in new container before resuming typing the rest
								self.typeText(inner_container, first_tag.tag_contents, speed, function(){
									self.typeText(container, next_remaining_text, speed, callback);
								});
								
								return; // Interrupt typing, since it'll be resumed by the callback
								
								break;
							
							case 'var':
								// Variable replacements
								
								// Prepend variable value to remaining text
								next_remaining_text = self.getVariableEnvironment().get(tag_value) + next_remaining_text;
								
								// No delay
								next_delay = 0;
								
								break;
						}
					}
					else
					{
						// No colon found : short tag
						
						var tag_time_value;
						var tag_time_value_factor = 1;
						var tag_time_value_relative = false;
						var tag_shake = false;
						var tag_flash = false;
						var tag_bgtask = false;
						
						// Parse tag definition to set flags
						var tag_flags = first_tag.tag_definition.split('');
						while(tag_flags.length > 0)
						{
							var flag = tag_flags.shift();
							
							switch(flag)
							{
								case '0': case '1': case '2': case '3': case '4': 
								case '5': case '6': case '7': case '8': case '9': 
									// Digit : update tag time value
									tag_time_value = (tag_time_value || 0) * 10 + parseInt(flag);
									break;
								
								case 'r':
									// r : make time value relative to speed
									tag_time_value_relative = true;
									break;
								
								case 'p':
									// p : multiply time value by 10 (retrocompatibility)
									tag_time_value_factor = 10;
									break;
								
								case 's':
									// s : trigger shake effect
									tag_shake = true;
									break;
								
								case 'f':
									// f : trigger flash effect
									tag_flash = true;
									break;
								
								case 'b':
									// b : send tag processing to background and resume typing immediately
									tag_bgtask = true;
									break;
							}
						}
						
						// Fix time value
						if(tag_time_value)
						{
							// Apply factor to set value
							tag_time_value = tag_time_value * tag_time_value_factor;
						}
						else
						{
							if(tag_flash || tag_shake)
							{
								// Default time for a flash or shake effect
								tag_time_value = 500;
							}
							else
							{
								// Default time for a pause
								tag_time_value = 1000;
							}
						}
						// If time value is relative, divide by speed
						if(tag_time_value_relative)
						{
							tag_time_value = tag_time_value / speed;
						}
						
						
						// Handle effects
						if(screen_display)
						{
							if(tag_flash)
							{
								addClass(screen_display.render, 'flashing');
								window.setTimeout(function(){ 
									removeClass(screen_display.render, 'flashing');
								}, tag_time_value);
							}
							if(tag_shake)
							{
								addClass(screen_display.render, 'shaking');
								window.setTimeout(function(){ 
									removeClass(screen_display.render, 'shaking');
								}, tag_time_value);
							}
						}
						
						// Set delay depending on b tag
						if(tag_bgtask)
						{
							next_delay = 0;
						}
						else
						{
							next_delay = tag_time_value;
						}
					}
					
					// Handle talking interruption if delay is high enough
					if(next_delay >= 300)
					{
						self.stopTalking();
					}
					
					// If not interrupted before, resume typing after tag
					window.setTimeout(function() {
						self.typeText(container, next_remaining_text, speed, callback);
					}, next_delay);
				}
				else
				{
					// If no tag, typing regular text
					
					// And display the last character
					var next_char = remaining_text.charAt(0);
					if(next_char == '\n')
					{
						// If a new line, convert to <br>
						container.appendChild(document.createElement('br'));
					}
					else
					{
						appendNodeTextContents(container, next_char);
						// Play a text blip if it's been long enough since our last typing sound AND we're dealing with an alphanumeric character
						var blip_delay_ms = getVoiceDelay(self.current_voice_id);

						if(self.timeSinceBlip >= blip_delay_ms && BLIP_CHAR_PATTERN.test(next_char))
						{
							self.voices[self.current_voice_id].play();
							self.timeSinceBlip = 0;
						}
					}
					
					if(!WHITESPACE_PATTERN.test(next_char))
					{
						// If inserted character is not a space, make sure character is talking.
						self.startTalking();
					}
					// TODO : stopTalking when typing a space ? or several successive spaces ?

					// Add our text speed delay to our blip counter

					self.timeSinceBlip += 33 / speed;
					
					// Then move forward in the string
					window.setTimeout(function() {
						self.typeText(container, remaining_text.substring(1), speed, callback);
					}, 33 / speed);
				}
			}
			else
			{
				self.stopTalking();
				
				if(callback)
				{
					window.setTimeout(callback, 0);
				}
			}
		}
	};
	
	
	self.init_complete_for = 0;
	
	// Perform initialisation tasks.
	// Moved to a separate method so it can be run in advance; it will be run just before typing if not triggered before
	// Separation allows the old textbox to be cleared before startup animations finish
	self.initLoadFrame = function(frame_data)
	{
		self.skip_remaining_text = false; // reinit skipping state
		
		if(!self.previous_frame_merged)
		{
			// If not merged to the previous frame, reinit the textbox and change speaker
			emptyNode(self.dialogue_box);
			self.displayed_texts = [];
			self.text_colors = [];
			
			// Update current talking character
			self.current_speaker = frame_data.speaker_id;
		}
		
		// In merged frames, only update the voice if it's explicitly set, OR if the character has changed
		if(!self.previous_frame_merged || getSpeakerName(frame_data) || frame_data.speaker_voice !== VOICE_AUTO)
		{
			self.current_voice_id = getVoiceId(frame_data);

			if(!self.voices[self.current_voice_id])
			{
				var sound_id = 'voice_' + self.current_voice_id;
				
				self.voices[self.current_voice_id] = SoundHowler.getSoundById(sound_id) || SoundHowler.registerSound(sound_id, {
					urls: getVoiceUrls(self.current_voice_id)
				});
			}
		}
		
		self.init_complete_for = frame_data.id;
	};
	
	// Load the text from a frame, and run callback when done typing
	self.loadFrameText = function(frame_data, callback)
	{
		if(self.init_complete_for != frame_data.id)
		{
			// If init not complete yet, for current frame, do it
			self.initLoadFrame(frame_data);
		}

		self.init_complete_for = 0; // Force next frame to reinit
		var previous_frame_merged = self.previous_frame_merged; // store previous merged state
		self.previous_frame_merged = frame_data.merged_to_next; // update merged state for next frame

		if(frame_data.text_content)
		{
			// If the frame contains text, display it
			
			// Always update the speaker name if it's set, but only clear the textbox in non-merged frames if it isn't
			var speaker_name = getSpeakerName(frame_data);
			if(speaker_name)
			{
				setNodeTextContents(self.name_box, speaker_name);
			}
			else if(!previous_frame_merged)
			{
				emptyNode(self.name_box);
			}
			// Create new span for the text and set its colour
			var frameText = document.createElement('span');
			frameText.style.color = frame_data.text_colour;
			
			if(!self.isEmptyText(frame_data.text_content))
			{
				// If text is not empty, display it
				self.dialogue_box.appendChild(frameText);
				self.displayed_texts.push(frame_data.text_content);
				self.text_colors.push(frameText.style.color);
			}
			// If text is empty, type it anyway to get the effects, but do not include the span in the page
			// Start typing text
			self.typeText(frameText, frame_data.text_content, frame_data.text_speed, callback);
		}
		else
		{
			if(callback)
			{
				window.setTimeout(callback, 0);
			}
		}
	};
	
	// Skip : type rest of the text immediately
	self.skip = function()
	{
		self.skip_remaining_text = true;
	};
	
	// Clear the textbox and stop typing
	self.clearText = function()
	{
		self.skip();
		emptyNode(self.dialogue_box);
		self.displayed_texts = [];
		self.text_colors = [];
	};
	
	// Export and restore state of text display engine.
	// displayed_texts and text_colors should be combined in v7.
	// Combining them now would break old saves.
	self.displayed_texts = [];
	self.text_colors = [];
	Object.defineProperty(self, 'state', {
		get: function()
		{
			return {
				name: self.name_box.textContent,
				texts: self.displayed_texts,
				colors: self.text_colors,
				previous_frame_merged: self.previous_frame_merged,
				current_speaker: self.current_speaker
			};
		},
		set: function(state)
		{
			setNodeTextContents(self.name_box, state.name);
			
			emptyNode(self.dialogue_box);
			self.displayed_texts = state.texts;
			for(var i = 0; i < self.displayed_texts.length; i++)
			{
				var frameText = document.createElement('span');
				var color = state.hasOwnProperty("colors") ? state.colors[i] : "white";
				frameText.style.color = color;
				self.dialogue_box.appendChild(frameText);
				self.instantTypeText(frameText, self.displayed_texts[i]);
			}
			
			self.current_speaker = state.current_speaker;
			self.previous_frame_merged = state.previous_frame_merged;
		}
	});
}

//END OF MODULE
Modules.complete('display_engine_text');
