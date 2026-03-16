"use strict";
/*
Ace Attorney Online - Object model for trial data.
Defines all existing properties in the trial data format with their default value.

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'trial_object_model',
	dependencies : ['objects_model', 'default_data'],
	init : function() {
		// Initialise the object model only after the constants are defined by default_data module.
		trial_object_model = getTrialObjectModel();
	}
}));

//INDEPENDENT INSTRUCTIONS
function getTrialObjectModel()
{
	return {
		profiles: [{
			id: null,
			long_name: "", // Name as displayed in the court records.
			short_name: "", // Name as displayed during dialogues.
			description: "",
			civil_status: "",
			hidden: false,
			base: "Inconnu", // Name of the AAO base profile used.
			icon: "", // URL of the profile icon if overriding the base profile's.
			custom_sprites:[{
				id: null,
				name: '', // Name displayed in editor menus.
				talking: '', // URL for talking animation
				still: '', // URL for non-talking animation
				startup: '', // URL for startup animation
				startup_duration: 0 // Startup animation duration (in ms)
			}],
			voice: VOICE_MALE // Character default voice. Used for frames which have speaker_voice set to VOICE_AUTO.
		}],
		evidence: [{
			id: null,
			name: "",
			description: "",
			metadata: "",
			hidden: false,
			icon: "",
			icon_external: false, // If false, icon is the name of an evidence icon from the AAO repository. If true, icon is an arbitrary image URL.
			check_button_data: [{
				type: 'text', // Type of the check details page. Can be text, image or sound.
				content: '' // It type is text, then interpreted as text contents. If type is image or sound, interpreted as a URL to the referenced object.
			}]
		}],
		places: [{
			id: null,
			name: "",
			background: {
				image: "",
				external: false, // If false, image is the name of a background from the AAO repository. If true, image is an arbitrary image URL.
				hidden: false
			},
			positions:[],
			background_objects:[{ // Objects displayed in front of the background, but behind characters.
				id: null,
				name: '',
				image: '',
				external: true, // External should always be true for background objects - there is no background object repository.
				hidden: 0,
				x: 0, // Horizontal position of the object in pixels. x=0 means left edge.
				y: 0 // Vertical position of the object in pixels. y=0 means top edge.
			}],
			foreground_objects:[{ // Objects displayed in front of characters. Same properties as above.
				id: null,
				name: '',
				image: '',
				external: true,
				hidden: 0,
				x: 0,
				y: 0
			}]
		}],
		sounds: [{
			id: null,
			name: "", // Name as displayed in the editor menus.
			path: "",
			external: false,
			volume: 100 // 100 is default volume of the sound. Can be increased or decreased.
		}],
		music: [{
			id: null,
			name: "",
			path: "",
			external: false,
			volume: 100, // 100 is default volume of the music. Can be increased or decreased.
			loop_start: 0 // Time to restart playing from when the end of the track is reached. (in ms since beginning of the track)
		}],
		popups: [{
			id: null,
			name: "",
			path: "",
			external: false
		}],
		cross_examinations: [{
			id: null,
			start: 0,
			end: 0,
			cocouncil_start: 0,
			cocouncil_end: 0,
			statements: [{
				id : 0,
				contradictions: [{
					contrad_elt: null,
					destination: 0
				}],
				pressing_conv_start: 0,
				pressing_conv_end: 0,
				optional_conv_start: 0,
				optional_conv_end: 0
			}],
			failure_conv_start: 0,
			failure_conv_end: 0
		}],
		scenes: [{
			id: null,
			name: '', // Name of the place displayed in the move list if not overridden.
			hidden: false,
			dialogues: [{
				id: null,
				start: 0,
				main: 0,
				talk: 0,
				present: 0,
				end: 0,
				intro_start: 0,
				intro_end: 0,
				talk_topics: [{
					id: null,
					title: '', // Title of the talk topic as displayed to the player.
					hidden: false,
					icon: 0, // Icon displayed for this topic. 0 : no icon.
					start: 0,
					end: 0
				}],
				present_conversations: [{
					elt: null,
					start: 0,
					end: 0
				}],
				locks: { // If null, psyche locks disabled for this dialogue.
					locks_to_display: [{ // List of psyche locks to display on screen for this dialogue.
						id: null,
						type: 'jfa_lock', // Image used for the lock. Currently, only jfa_lock is supported.
						x: 0, // Horizontal position of the center of the lock in pixels. x=0 means left edge.
						y: 0 // Vertical position of the center of the lock in pixels. y=0 means top edge.
					}],
					hidden: false // If true, psyche locks button will not be displayed in the player. Can be changed through actions.
				}
			}],
			current_dialogue: 0, // Id of the currently active dialogue for this scene.
			examinations: [{
				id: null,
				start: 0,
				examine: 0,
				end: 0,
				place: PLACE_NONE,
				examine_conversations: [{
					area: null, // Descriptor for the clickable area that will trigger this conversation. If null, the conversation will match any position clicked.
					start: 0,
					end: 0
				}],
				deduce_conversations: [{
					area: null,
					elt: null,
					start: 0,
					end: 0
				}],
				enable_deduction: false
			}],
			current_examination: 0, // Id of the currently active examination for this scene.
			start: 0,
			move: 0,
			end: 0,
			move_list: [{
				scene_type: 0, // Type of scene. Currently either 0 (nothing) or 'scenes'.
				scene_id: 0,
				name_override: '' // If empty, then the target scene's name will be displayed.
			}]
		}],
		frames: [{
			id: null,
			speaker_name: "",
			speaker_use_name: false, // If true, the value of "speaker_name" is used. Otherwise, the short_name of the profile referenced by speaker_id is used.
			speaker_id: PROFILE_EMPTY,
			speaker_voice: VOICE_AUTO, // If VOICE_AUTO, use the voice of the profile referenced by speaker_id.
			sound: SOUND_NONE,
			music: MUSIC_UNCHANGED,
			music_fade: {
				duration: 0, // In milliseconds, the duration of the fade
				// Below fields are only applicable to crossfades
				to_volume: 100, // Music volume at the end of the crossfade, relative to the track's default volume, in percent
				same_position: false // Crossfade to new music at same playback position
			},
			place: PLACE_NONE,
			place_position: POSITION_NONE, // Position ID : if negative, one of the default positions available in all places. If positive, position specific to this place. If POSITION_NONE, POSITION_CENTER_ON_TALKING or POSITION_DO_NOT_MOVE, let the player logic decide the alignment.
			place_transition: TRANSITION_NO, // Type of transition for moving into this place if it changes. Only TRANSITION_NO is supported at this time.
			characters: [{ // List of characters to manipulate in this frame. If character is already displayed, it will be modified (sprite changed and moved) to match these new settings; else it will be added.
				profile_id: 0,
				sprite_id: 0,
				sync_mode: SYNC_AUTO, // Text typing and talking animation synchronisation mode. SYNC_SYNC: use talking animation while text is being typed. SYNC_AUTO: same, but only if profile_id is the same as speaker_id. Other values : SYNC_STILL, SYNC_TALK.
				startup_mode: STARTUP_SKIP, // Startup animation mode. STARTUP_PLAY_DURING: Play startup animation while text is being typed. STARTUP_PLAY_BEFORE: Play startup animation before starting to type text. STARTUP_SKIP: Do not play.
				position: POSITION_NONE, // If POSITION_NONE, character is added at the current screen position.
				mirror_effect: false, // If true, horizontal mirror effect applied.
				visual_effect_appears: EFFECT_NONE, // Visual effect to apply when this character appears on screen.
				visual_effect_disappears: EFFECT_AUTO // Visual effect to apply when this character will be removed from the screen.
			}],
			characters_erase_previous: false, // If true, previously displayed characters which are not in this frames characters list are wiped.
			popups: [{
				popup_id: 0,
				position: POSITION_NONE, // If POSITION_NONE, popup is displayed at the current screen position.
				mirror_effect: false
			}],
			fade: {
				fade_type: FADEMODE_FADEOUT,
				fade_colour: '#000000',
				fade_duration: 1000,
				fade_placement: FADEPOS_EVERYTHING
			},
			action_name: "",
			action_parameters: [], // Action parameters structure is not described by this object model. See action_descriptions.php.
			text_colour: "white",
			text_content: "",
			text_speed: 1,
			hidden: false,
			wait_time: 0, // If 0, player needs to proceed manually to next frame. Else, number of milliseconds until proceeding automatically.
			merged_to_next: false // If true, next frame text will be appended to this frame's when playing. Action processing is affected as well.
		}]
	};
}

//EXPORTED VARIABLES
var trial_object_model;

//EXPORTED FUNCTIONS
// Get the object model path from the type
function getTrialSubModelFromType(type)
{
	switch(type)
	{
		case 'frames' :
			return trial_object_model.frames[0];
			break;
		
		case 'profiles' :
			return trial_object_model.profiles[0];
			break;
		
		case 'evidence' :
			return trial_object_model.evidence[0];
			break;
		
		case 'sprite' :
			return trial_object_model.profiles[0].custom_sprites[0];
			break;
		
		case 'character_info' :
			return trial_object_model.frames[0].characters[0];
			break;
		
		case 'popup_info' :
			return trial_object_model.frames[0].popups[0];
			break;

		case 'fade_info' :
			return trial_object_model.frames[0].fade;
		
		case 'places' :
			return trial_object_model.places[0];
			break;
		
		case 'music' :
			return trial_object_model.music[0];
			break;

		case 'music_fade' :
			return trial_object_model.frames[0].music_fade;
			break;

		case 'music_crossfade' :
			return trial_object_model.frames[0].music_fade.crossfade;
			break;
		
		case 'sounds' :
			return trial_object_model.sounds[0];
			break;
			
		case 'popups' :
			return trial_object_model.popups[0];
			break;
		
		case 'contradiction' :
			return trial_object_model.cross_examinations[0].statements[0].contradictions[0];
			break;
		
		case 'lock_info':
			return trial_object_model.scenes[0].dialogues[0].locks.locks_to_display[0];
			break;
	}
}

//END OF MODULE
Modules.complete('trial_object_model');
