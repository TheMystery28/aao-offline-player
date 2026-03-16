"use strict";
/*
Ace Attorney Online - Player live debugger

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'player_debug',
	dependencies : ['trial', 'trial_data', 'form_elements', 'events', 'page_loaded', 'nodes'],
	init : function()
	{
		if(trial_information && trial_information.can_write && 'debug' in _GET)
		{
			// Debugger is only activable for trial authors
			debug_enabled = true;
			
			debug_status = document.getElementById('debug_status');
			debug_vars = document.getElementById('debug_vars');
			debug_cr_profiles = document.getElementById('debug_cr_profiles');
			debug_cr_evidence = document.getElementById('debug_cr_evidence');
			debug_scenes = document.getElementById('debug_scenes');
			debug_frames = document.getElementById('debug_frames');
			
			var player_debug = document.getElementById('player_debug');
			player_debug.hidden = false;
			
			// Panel to check and edit player status
			debug_status.frame_id = createFormElement('natural');
			registerEventHandler(debug_status.frame_id, 'change', function() {
				var new_value = this.getValue();
				if(new_value != player_status.current_frame_id)
				{
					readFrame(getRowIndexById('frames', new_value));
				}
			}, false);
			debug_status.appendChild(createLabel(debug_status.frame_id, 'frame_id'));
			
			debug_status.frame_index = createFormElement('natural');
			registerEventHandler(debug_status.frame_index, 'change', function() {
				var new_value = this.getValue();
				if(new_value != player_status.current_frame_index)
				{
					readFrame(new_value);
				}
			}, false);
			debug_status.appendChild(createLabel(debug_status.frame_index, 'frame_index'));
			
			// Button to add a variable
			var debug_add_var = document.getElementById('debug_add_var');
			registerEventHandler(debug_add_var, 'click', function(){
				var var_name;
				if(var_name = prompt(l('var_name')))
				{
					player_status.game_env.set(var_name, '');
					debugRefreshVars();
				}
			}, false);
			
			// Button to show a frame in the debugger
			var debug_show_frame = document.getElementById('debug_show_frame');
			registerEventHandler(debug_show_frame, 'click', function(){
				var frame_id;
				if((frame_id = prompt(l('frame_id'))) && (getRowIndexById('frames', frame_id) > 0))
				{
					debugWatchFrame(frame_id);
					debugRefreshFrames();
				}
			}, false);
			
			// Refresh the debuggers that actually have something to display at startup
			debugRefreshCourtRecords();
			debugRefreshScenes();
		}
	}
}));

//INDEPENDENT INSTRUCTIONS
var debug_enabled;
var debug_status;
var debug_vars;
var debug_cr_profiles;
var debug_cr_evidence;
var debug_scenes;
var debug_frames;
var shown_frames = new Object();

function debugRefreshStatus()
{
	if(debug_enabled)
	{
		debug_status.frame_id.setValue(player_status.current_frame_id);
		debug_status.frame_index.setValue(player_status.current_frame_index);
	}
}

function debugRefreshVars()
{
	if(debug_enabled)
	{
		emptyNode(debug_vars);
		
		var env = player_status.game_env;
		for(var var_name in env.vars)
		{
			var value_input = createFormElement('string');
			value_input.setValue(env.vars[var_name]);
			registerEventHandler(value_input, 'change', function() {
				env.set(var_name, this.getValue());
			}, false);
			debug_vars.appendChild(createLabel(value_input, null, var_name));
		}
	}
}

function debugRefreshCourtRecords()
{
	if(debug_enabled)
	{
		emptyNode(debug_cr_profiles);
		
		for(var i = 1; i < trial_data.profiles.length; i++)
		{
			var profile_hidden = createFormElement('checkbox');
			profile_hidden.setValue(!trial_data.profiles[i].hidden);
			registerEventHandler(profile_hidden, 'change', (function(id) {
				setCrElementHidden('profiles', id, !this.getValue());
			}).bind(profile_hidden, trial_data.profiles[i].id), false);
			debug_cr_profiles.appendChild(createLabel(profile_hidden, null, trial_data.profiles[i].long_name));
		}
		
		emptyNode(debug_cr_evidence);
		
		for(var i = 1; i < trial_data.evidence.length; i++)
		{
			var evidence_hidden = createFormElement('checkbox');
			evidence_hidden.setValue(!trial_data.evidence[i].hidden);
			registerEventHandler(evidence_hidden, 'change', (function(id) {
				setCrElementHidden('evidence', id, !this.getValue());
			}).bind(evidence_hidden, trial_data.evidence[i].id), false);
			debug_cr_evidence.appendChild(createLabel(evidence_hidden, null, trial_data.evidence[i].name));
		}
	}
}

function debugRefreshScenes()
{
	if(debug_enabled)
	{
		emptyNode(debug_scenes);
		
		for(var i = 1; i < trial_data.scenes.length; i++)
		{
			var scene = trial_data.scenes[i];
			
			var scene_section = document.createElement('section');
			
			// Scene title and visibility control
			var scene_title = document.createElement('h4');
			var scene_hidden = createFormElement('checkbox');
			scene_hidden.setValue(!scene.hidden);
			registerEventHandler(scene_hidden, 'change', (function(scene) {
				scene.hidden = !this.getValue();
			}).bind(scene_hidden, scene), false);
			scene_title.appendChild(createLabel(scene_hidden, null, scene.name));
			scene_section.appendChild(scene_title);
			
			// Dialogues
			for(var j = 0; j < scene.dialogues.length; j++)
			{
				var dialogue = scene.dialogues[j];
				
				var dialogue_section = document.createElement('section');
				
				for(var k = 0; k < dialogue.talk_topics.length; k++)
				{
					var talk_topic = dialogue.talk_topics[k];
					
					var talk_topic_hidden = createFormElement('checkbox');
					talk_topic_hidden.setValue(!talk_topic.hidden);
					registerEventHandler(talk_topic_hidden, 'change', (function(talk_topic) {
						talk_topic.hidden = !this.getValue();
					}).bind(talk_topic_hidden, talk_topic), false);
					dialogue_section.appendChild(createLabel(talk_topic_hidden, null, talk_topic.title));
				}
				
				scene_section.appendChild(dialogue_section);
			}
			
			debug_scenes.appendChild(scene_section);
		}
	}
}

function debugWatchFrame(frame_id)
{
	shown_frames[frame_id] = true;
}

function debugRefreshFrames()
{
	if(debug_enabled)
	{
		emptyNode(debug_frames);
	
		for(var frame_id in shown_frames)
		{
			var frame = getRowById('frames', frame_id);
					
			var frame_hidden = createFormElement('checkbox');
			frame_hidden.setValue(!frame.hidden);
			registerEventHandler(frame_hidden, 'change', (function(frame) {
				frame.hidden = !this.getValue();
			}).bind(frame_hidden, frame), false);
			debug_frames.appendChild(createLabel(frame_hidden, null, '#' + frame.id));
		}
	}
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('player_debug');
