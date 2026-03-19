"use strict";
/*
Ace Attorney Online - Player actions handler

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'player_actions',
	dependencies : ['display_engine_place', 'expression_engine', 'frame_data', 'objects', 'player_courtrecord', 'player_debug'],
	init : function()
	{
		registerEventHandler(document.getElementById('present-center'), 'click', function()
		{
			// Click on present button
			
			// Load selected evidence from the evidence display element
			var selected_evidence = document.getElementById('evidence-display').getAttribute('data-selected-elt', '');
			
			if(selected_evidence != '')
			{
				// If a piece of evidence is selected
				
				// Load target frames from this element
				var target_frame_by_element = JSON.parse(this.getAttribute('data-target-by-elt'));
				var failure_frame = this.getAttribute('data-fail-target');
				
				// Match to get destination frame
				var destination_frame;
				if(selected_evidence in target_frame_by_element)
				{
					destination_frame = target_frame_by_element[selected_evidence];
				}
				else
				{
					destination_frame = failure_frame;
				}
				
				readFrame(getRowIndexById('frames', destination_frame));
			}
			
			// Else, no piece of evidence selected : do nothing
		}, false);
		
		registerEventHandler(document.getElementById('present-topright'), 'click', function()
		{
			// Click on CE present button : generate real present interface
			generatePresentEvidence(
				'all', 
				JSON.parse(this.getAttribute('data-target-by-elt')), 
				this.getAttribute('data-fail-target'), 
				this.getAttribute('data-return-frame')
			);
		}, false);
		
		// Prepare form to input values
		var input_form = document.getElementById('inputs');
		registerEventHandler(input_form, 'submit', function(e){
			for(var var_name in input_form.input_map)
			{
				player_status.game_env.set(var_name, input_form.input_map[var_name].getValue());
			}
			debugRefreshVars();
			readFrame(player_status.next_frame_index);
			
			e.preventDefault();
		}, false);
		
		// Lets expression engine return player health.
		register_custom_function('player_health', function(){
			return player_status.health;
		});
	}
}));

//INDEPENDENT INSTRUCTIONS

// Set the health bar display
function refreshHealthBar()
{
	// Old saves don't have flash_health status, so those flash 0.
	var flash = player_status.hasOwnProperty("flash_health") ? Math.min(player_status.flash_health, player_status.health) : 0;
	document.getElementById('lifebar-green').style.width = player_status.health - flash + 'px';
	document.getElementById('lifebar-flash').style.width = flash + 'px';
	document.getElementById('lifebar-red').style.width = (120 - player_status.health) + 'px';
}

// Generate the panel to select an area/object in a place
function generateExaminePlace(place_id, target_frame_by_area, target_frame_by_object)
{
	// Generate pointing area
	var examination_container = document.getElementById('examination');
	emptyNode(examination_container);
	var place = getPlace(place_id);
	
	// Generate the place display
	var place_display = new PlaceDisplay();
	examination_container.appendChild(place_display.render);
	place_display.setPlace(place);
	
	// Set clickable areas
	for(var area in target_frame_by_area)
	{
		var area_elt = place_display.getClickableArea(area);
		area_elt.href = 'javascript:readFrame(' + getRowIndexById('frames', target_frame_by_area[area]) + ');';
	}
	
	// Set clickable objects
	for(var object_str in target_frame_by_object)
	{
		var obj_split = object_str.split('#');
		place_display.setObjectCallback(obj_split[0], obj_split[1], readFrame.bind(undefined, getRowIndexById('frames', target_frame_by_object[object_str])));
	}
}

// Generate the panel to select an area/object in a place
function generateExaminePicture(img_uri, target_frame_by_area)
{
	// Generate pointing area
	var examination_container = document.getElementById('examination');
	emptyNode(examination_container);
	
	// Generate image map for the pointing areas
	var map = document.createElement('map');
	map.name = 'examination_map';
	examination_container.appendChild(map);
	
	for(var area_desc in target_frame_by_area)
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
		
		area.href = 'javascript: readFrame(' + getRowIndexById('frames', target_frame_by_area[area_desc]) + ');';
		
		// Firefox matches area from first to last : make sure default is last.
		if(shape == 'default')
		{
			map.appendChild(area);
		}
		else
		{
			map.insertBefore(area, map.firstChild);
		}
	}
	
	// Generate rendering area
	var render = document.createElement('div');
	render.id = 'render';
	examination_container.appendChild(render);
	
	// Generate background
	var background = new Image();
	background.src = img_uri;
	background.useMap = '#examination_map';
	render.appendChild(background);
}

function generatePresentEvidence(type_lock, target_frame_by_element, failure_frame, back_frame, back_action)
{
	// No evidence currently selected
	var evidence_display = document.getElementById('evidence-display');
	emptyNode(evidence_display);
	evidence_display.setAttribute('data-selected-elt', '');
	
	// Store target frames as a property of the present button
	var present_button = document.getElementById('present-center');
	present_button.setAttribute('data-target-by-elt', JSON.stringify(target_frame_by_element));
	present_button.setAttribute('data-fail-target', failure_frame);
	
	// Switch to select evidence mode
	setClass(bottom_screen, 'present');
	var content = document.getElementById('content');
	setClass(content, 'cr-select cr-' + type_lock);
	
	// Display back button if needed
	if(back_frame > 0)
	{
		var back = document.getElementById('back');
		back.setAttribute('data-target-frame', getRowIndexById('frames', back_frame));
		back.setAttribute('data-target-action', back_action || '');
		addClass(bottom_screen, 'back');
	}
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS

// Auto-continue helper: if the current case is part of a sequence and there's a next part,
// redirect to it. Returns true if redirecting, false if no next part (caller should show end screen).
function tryGoToNextPart()
{
	// Only applies if we're in a sequence with a list of parts
	if(!trial_information || !trial_information.sequence || !trial_information.sequence.list)
	{
		return false;
	}

	var part_list = trial_information.sequence.list;
	var target_part = 0;

	for(var i = 0; i < part_list.length - 1; i++)
	{
		if(part_list[i].id == trial_information.id)
		{
			// Found current part — next part is at index i + 1
			target_part = part_list[i + 1].id;
			break;
		}
	}

	if(target_part == 0)
	{
		// No next part found (we're the last part, or not in the list)
		return false;
	}

	// Prepare minimal save data for the part jump (no data transfer)
	var saveData = getSaveData();
	saveData.trial_id = target_part;
	saveData.current_music_id = MUSIC_STOP;
	saveData.top_screen_state = null;
	saveData.player_status = new Object({
		game_env: new VariableEnvironment(global_env),
		health: 120,
		flash_health: 0,
		game_over_target: 0
	});
	saveData.trial_data_diffs = {};
	saveData.trial_data_base_dates = {};

	// Cancel any pending wait timer before redirecting
	if(typeof wait_timer !== 'undefined' && wait_timer) { window.clearTimeout(wait_timer); wait_timer = null; }
	// Redirect to next part
	window.location.href = '?trial_id=' + target_part + '&save_data=' + encodeURIComponent(Base64.encode(JSON.stringify(saveData)));
	return true;
}

function runFrameActionBefore(frame_data, computed_parameters)
{
	// Show the skip button if needed - use the wait button otherwise.
	function showSkipButton()
	{
		if(player_status.proceed_timer)
		{
			setClass(bottom_screen, 'wait'); // Show wait filler "button"
		}
		else
		{
			setClass(bottom_screen, 'skip'); // Show skip button
		}
	}
	
	switch(frame_data.action_name)
	{
		// Actions to manipulate the court records
		
		case 'HideElements':
		case 'RevealElements':
			var hidden = frame_data.action_name == 'HideElements' ? true : false;
			
			var elements = computed_parameters.multiple.element;
			for(var i = 0; i < elements.length; i++)
			{
				setCrElementHidden(elements[i].element_desc.type, elements[i].element_desc.id, hidden);
			}
			showSkipButton();
			break;
		
		case 'DisplayElements':
			var elements = computed_parameters.multiple.element;
			for(var i = 0; i < elements.length; i++)
			{
				top_screen.iconsPrepareAdd(elements[i]);
			}
			showSkipButton();
			break;
		
		// Actions to manipulate the game flow
		
		case 'HideFrame':
		case 'RevealFrame':
			var hidden = frame_data.action_name == 'HideFrame' ? true : false;
		
			var frames = computed_parameters.multiple.frame; // List of frames to update
			for(var i = 0; i < frames.length; i++)
			{
				var frame = getRowById('frames', frames[i].target);
				if(frame) 
				{
					frame.hidden = hidden;
					debugWatchFrame(frames[i].target);
				}
			}
			debugRefreshFrames();
			showSkipButton();
			break;
		
		case 'HideObject':
		case 'RevealObject':
			var hidden = frame_data.action_name == 'HideObject' ? true : false;
			
			var object_list = computed_parameters.multiple.object;
			for(var i = 0; i < object_list.length; i++)
			{
				var object_desc = object_list[i].object_desc;
				
				var object_place = getPlace(object_desc.place_id);
				if(object_place)
				{
					var object = getById(object_place[object_desc.layer], object_desc.id);
					if(object) 
					{
						object.hidden = hidden;
					}
				}
			}
			
			top_screen.refreshPlaceObjects();

			showSkipButton();
			break;
		
		case 'HideScene':
		case 'RevealScene':
			var hidden = frame_data.action_name == 'HideScene' ? true : false;
			
			var scene_desc = computed_parameters.global.scene;
			var scene = getById(trial_data[scene_desc.scene_type], scene_desc.scene_id);
			
			if(scene)
			{
				scene.hidden = hidden;
			}
			
			debugRefreshScenes();
			showSkipButton();
			break;
		
		case 'RevealDialogueIntro':
		case 'HideDialogueIntro':
			var hidden = frame_data.action_name == 'HideDialogueIntro' ? true : false;
			
			var dialogue_desc = computed_parameters.global.dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);

			if(scene) 
			{
				var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
				var intro_start_frame = getRowById('frames', dialogue.intro_start);
				intro_start_frame.hidden = !hidden;
				debugWatchFrame(dialogue.intro_start);
			}
			debugRefreshFrames();
			showSkipButton();
			
			break;
		
		case 'HideTalkTopic':
		case 'RevealTalkTopic':
			var hidden = frame_data.action_name == 'HideTalkTopic' ? true : false;
			
			var topic_desc = computed_parameters.global.talk_topic;
			var scene = getById(trial_data[topic_desc.scene_type], topic_desc.scene_id);

			if(scene) 
			{
				var dialogue = getById(scene[topic_desc.section_type], topic_desc.section_id);
				var topic = getById(dialogue[topic_desc.conv_type], topic_desc.conv_id);
				
				topic.hidden = hidden;
			}
			debugRefreshScenes();
			showSkipButton();
			break;
		
		case 'RevealDialogueLocks':
		case 'HideDialogueLocks':
			
			var hidden = frame_data.action_name == 'HideDialogueLocks' ? true : false;
			
			var dialogue_desc = computed_parameters.global.dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);

			if(scene)
			{
				var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
			
				if(dialogue.locks)
				{
					dialogue.locks.hidden = hidden;
					//TODO : add psyche locks to debugger
				}
			}

			showSkipButton();
			break;
		
		case 'SetGameOver':
			player_status.game_over_target = computed_parameters.global.target;
			showSkipButton();
			break;
			
		// Psyche locks actions
		
		case 'LocksShow':
			var dialogue_desc = computed_parameters.context.parent_dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
			var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
			
			top_screen.locks_display.showLocks(dialogue_desc, dialogue.locks.locks_to_display);
			showSkipButton();
			break;
		
		case 'LocksBreak':
			var dialogue_desc = computed_parameters.context.parent_dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
			var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
			
			for(var i = 0; i < computed_parameters.multiple.lock.length; i++)
			{
				top_screen.locks_display.breakLock(dialogue_desc, computed_parameters.multiple.lock[i].lock_desc.lock_id);
			}
			showSkipButton();
			break;
			
		case 'LocksHide':
			// TODO : add action parameter to decide whether to animate
			top_screen.locks_display.hideLocks(true);
			showSkipButton();
			break;
		
		// Actions to manipulate health
		
		case 'FlashHealth':
			player_status.flash_health = parseInt(computed_parameters.global.points);
			refreshHealthBar();
			showSkipButton();
			break;
		
		// Actions to handle CEs
		
		case 'CEStatement':
			// Display CE wait and skip buttons 
			setClass(bottom_screen, 'ce-skip');
			break;
		
		default:
			showSkipButton();
			break;
	}
}

function runFrameActionAfter(frame_data, computed_parameters)
{
	// Show the proceed button if needed - use the wait button otherwise.
	function showProceedButton()
	{
		if(player_status.proceed_click)
		{
			setClass(bottom_screen, 'proceed'); // Show proceed button
		}
		else
		{
			setClass(bottom_screen, 'wait'); // Show wait button
		}
	}
	
	// If passing null as argument, no action
	if(!frame_data)
	{
		showProceedButton();
		return;
	}
	
	switch(frame_data.action_name)
	{		
		// Actions to manipulate the game flow
		
		case 'GoTo':
			player_status.next_frame_index = getRowIndexById('frames', computed_parameters.global.target);
			showProceedButton();
			break;
			
		case 'GameOver':
			
			var target_part = 0;
			var target_frame = 0;
			
			// Even if a proceed commmand is triggered, don't go to a frame.
			player_status.next_frame_index = 0;
			
			switch(parseInt(computed_parameters.global.action))
			{
				case 0:
					// Action is to end and do nothing
					// Auto-continue: if in a collection with a next part, redirect instead
					if(!tryGoToNextPart())
					{
						window.setTimeout(readFrame.bind(undefined, 0), 0);
					}
					break;
				
				case 1:
					// Action is to go to next part : just set target part to next and jump into next case.
					
					var part_list = trial_information.sequence.list;
					for(var i = 0; !target_part && i < part_list.length - 1; i++)
					{
						if(part_list[i].id == trial_information.id)
						{
							// If current trial is part at index i
							
							// Set target part to id of part i + 1
							target_part = part_list[i + 1].id;
							// And do not set a target frame (leave it to 0)
						}
					}
					
					if(target_part == 0)
					{
						// If no target part was found, revert to do nothing
						window.setTimeout(readFrame.bind(undefined, 0), 0);
						break;
					}
					// Else, jump into next case
				
				case 2:
					// Action is to go to a given part and frame : generate URL and redirect
					
					// If target not set by above case, load from parameters
					if(!target_part)
					{
						target_part = computed_parameters.global.target_part;
						target_frame = computed_parameters.global.target_frame;
					}
					
					// Prepare save data that will be used for part jump.
					var saveData = getSaveData();
					saveData.trial_id = target_part;
					saveData.current_music_id = MUSIC_STOP; // Don't carry over music, since we can't be sure it exists in the next part
					saveData.top_screen_state = null; // No top screen information saved
					saveData.player_status = new Object({
						game_env: saveData.player_status.game_env,
						health: saveData.player_status.health,
						flash_health: saveData.player_status.flash_health,
						game_over_target: 0
					}); // Player status is stripped of trial-level information that becomes meaningless on transfer.
					
					// Set next frame, if it makes sense to do so
					if(target_frame > 0)
					{
						saveData.player_status.next_frame_id = target_frame;
					}
					
					// Filter data transfer
					if(computed_parameters.global.data_transfer != 2)
					{
						// If not full transfer
						
						// Filter out health
						saveData.player_status.health = 120;
						saveData.player_status.flash_health = 0;
						
						// Filter out trial diffs
						saveData.trial_data_diffs = {};
						saveData.trial_data_base_dates = {};
					}
					if(computed_parameters.global.data_transfer == 0)
					{
						// If no transfer at all, also filter out variables
						saveData.player_status.game_env = new VariableEnvironment(global_env);
					}
					
					// Save data computed : launch redirection !
					// Cancel any pending wait timer to prevent it from firing after
					// the redirect and overwriting the location with a different URL.
					if(wait_timer) { window.clearTimeout(wait_timer); wait_timer = null; }
					window.location.href = '?trial_id=' + target_part + '&save_data=' + encodeURIComponent(Base64.encode(JSON.stringify(saveData)));

					break;
			}
			
			break;
		
		// Actions to manipulate health
		
		case 'SetHealth': case 'ReduceHealth': case 'IncreaseHealth':
			
			var health_value = parseInt(computed_parameters.global.points);
			
			switch(frame_data.action_name)
			{
				case 'SetHealth':
					player_status.health = health_value;
					break;
				
				case 'ReduceHealth':
					player_status.health -= health_value;
					break;
				
				case 'IncreaseHealth':
					player_status.health += health_value;
					break;
			}
			
			player_status.health = Math.min(player_status.health, 120);
			player_status.flash_health = 0;
			
			if(player_status.health > 0)
			{
				refreshHealthBar();
				showProceedButton();
			}
			else
			{
				player_status.health = 0;
				refreshHealthBar();
				window.setTimeout(readFrame.bind(undefined, getRowIndexById('frames', player_status.game_over_target)), 0);
			}
			break;
		
		// Actions to get player input
		
		case 'MultipleChoices':
			var options_element = document.getElementById('options');
			emptyNode(options_element); // Empty the list of options
			
			// Create the list of options
			var answers = computed_parameters.multiple.answer;
			for(var i = 0; i < answers.length; i++)
			{
				var answer_element = document.createElement('a');
				setNodeTextContents(answer_element, answers[i].answer_text); // Set option text
				registerEventHandler(answer_element, 'click', readFrame.bind(undefined, getRowIndexById('frames', answers[i].answer_dest)), false);
				
				options_element.appendChild(answer_element);
			}
			setClass(bottom_screen, 'options'); // Display the options panel
			
			// Set the back button if needed
			if(computed_parameters.global && computed_parameters.global.locks_show_return === "true")
			{
				// Fetch data of the parent dialogue
				var parent_dialogue = computed_parameters.context.parent_dialogue;
				var scene = getById(trial_data[parent_dialogue.scene_type], parent_dialogue.scene_id);
				var dialogue = getById(scene[parent_dialogue.section_type], parent_dialogue.section_id);
				
				// Set back button to the talk frame of the dialogue
				var back = document.getElementById('back');
				back.setAttribute('data-target-frame', getRowIndexById('frames', dialogue.talk));
				back.setAttribute('data-target-action', 'hide_locks');
				addClass(bottom_screen, 'back'); // Display the button
			}
			
			break;
		
		case 'AskForEvidence':
			
			// Get list of destinations
			var target_frame_by_element = new Object();
			
			var elements = computed_parameters.multiple.element;
			for(var i = 0; i < elements.length; i++)
			{
				target_frame_by_element[elements[i].element_desc.type + '_' + elements[i].element_desc.id] = elements[i].element_dest;
			}
			
			// Get back frame data
			var back_frame = 0;
			var back_action = '';
			if(computed_parameters.global.locks_show_return === "true")
			{
				// Load data from the parent dialogue
				var dialogue_desc = computed_parameters.context.parent_dialogue;
				var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
				var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
				
				back_frame = dialogue.main;
				back_action = 'hide_locks';
			}
			
			generatePresentEvidence(
				computed_parameters.global.type_lock, 
				target_frame_by_element, 
				computed_parameters.global.failure_dest, 
				back_frame,
				back_action
			);
		
			break;
		
		case 'PointArea':
			
			// Collect data for all targets
			var target_frame_by_area = new Object();
			var target_frame_by_object = new Object();
			
			var areas = computed_parameters.multiple.area;
			
			for(var i = 0; i < areas.length; i++)
			{
				if(is_scalar(areas[i].area_def))
				{
					// Area def is a shape for the image map
					target_frame_by_area[areas[i].area_def] = areas[i].area_dest;
				}
				else
				{
					// Area def is an object descriptor
					target_frame_by_object[areas[i].area_def.layer + '#' + areas[i].area_def.id] = areas[i].area_dest;
				}
			}
			
			if(computed_parameters.global.failure_dest)
			{
				target_frame_by_area['default'] = computed_parameters.global.failure_dest;
			}
			
			// Generate Examine window
			if(computed_parameters.global.background.match(/^-?\d+$/))
			{
				// If background is a place ID
				generateExaminePlace(computed_parameters.global.background, 
					target_frame_by_area, 
					target_frame_by_object
				);
			}
			else
			{
				// Else, background is a URL
				generateExaminePicture(computed_parameters.global.background, 
					target_frame_by_area
				);
			}
			
			setClass(bottom_screen, 'examination');
			
			// Display return button if needed
			if(computed_parameters.global.locks_show_return === "true")
			{
				// Load data from the parent dialogue
				var dialogue_desc = computed_parameters.context.parent_dialogue;
				var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
				var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
				
				var back = document.getElementById('examination-back');
				back.setAttribute('data-target-frame', getRowIndexById('frames', dialogue.main));
				back.setAttribute('data-target-action', 'hide_locks');
				addClass(bottom_screen, 'examination-back');
			}
			
			break;
		
		case 'InputVars':
			
			var input_form = document.getElementById('inputs');
			emptyNode(input_form);
			var variables = computed_parameters.multiple.variable;
			
			input_form.input_map = new Object();
			
			for(var i = 0; i < variables.length; i++)
			{
				var input = createFormElement(variables[i].var_type);
				
				input_form.input_map[variables[i].var_name] = input;
				input_form.appendChild(input);
			}
			
			var input_button = document.createElement('input');
			input_button.type = 'submit';
			input_button.setAttribute('data-locale-value', 'enter');
			translateNode(input_button);
			input_form.appendChild(input_button);
			
			setClass(bottom_screen, 'inputs');
			
			break;
		
		// Psyche locks actions
		
		case 'LocksEnd':
		
			// Hide the locks
			top_screen.locks_display.hideLocks(true);
			
			// Redirect to dialogue talk frame
			var dialogue_desc = computed_parameters.context.parent_dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
			var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
				
			player_status.next_frame_index = getRowIndexById('frames', dialogue.talk);
			showProceedButton();
			
			break;
		
		// Actions to manipulate variables
			
		case 'DefineVars':
			var variables = computed_parameters.multiple.variable;
			for(var i = 0; i < variables.length; i++)
			{
				player_status.game_env.set(variables[i].var_name, variables[i].var_value);
			}
			debugRefreshVars();
			showProceedButton();
			break;
		
		case 'TestExprValue':
			var global_params = computed_parameters.global;
			
			// Evaluate the expression value
			var expression_value;
			switch(global_params.expr_type)
			{
				case 'expression': // As an expression
					expression_value = evaluate_expression(global_params.expression, player_status.game_env);
					break;
				
				case 'var_name': // As a variable name
					expression_value = player_status.game_env.get(global_params.var_name);
					break;
			}
			
			var matched = false; // Flag : has one value matched ?
			var values = computed_parameters.multiple.values;
			for(var i = 0; i < values.length && !matched; i++)
			{
				if(expression_value == values[i].value)
				{
					// If match, redirect to success frame
					player_status.next_frame_index = getRowIndexById('frames', values[i].value_dest);
					matched = true;
				}
			}
			
			if(!matched)
			{
				// If no match, redirect to failure frame
				player_status.next_frame_index = getRowIndexById('frames', global_params.failure_dest);
			}
			
			showProceedButton();
			break;
		
		case 'EvaluateConditions':
			
			var matched = false; // Flag : has one condition matched ?
			var conditions = computed_parameters.multiple.condition;
			for(var i = 0; i < conditions.length && !matched; i++)
			{
				if(evaluate_expression(conditions[i].expression, player_status.game_env))
				{
					// If condition met, redirect to success frame
					player_status.next_frame_index = getRowIndexById('frames', conditions[i].cond_dest);
					matched = true;
				}
			}
			
			if(!matched)
			{
				// If no condition matched, redirect to failure frame
				player_status.next_frame_index = getRowIndexById('frames', computed_parameters.global.failure_dest);
			}
			
			showProceedButton();
			break;
		
		// Actions to handle CEs
		
		case 'CERestart':
			
			var ce = getRowById('cross_examinations', computed_parameters.context.ce_desc);
			
			// Redirect to start of the CE
			player_status.next_frame_index = getRowIndexById('frames', ce.start);
			
			showProceedButton();
			
			break;
		
		case 'CEReturnAfter':
			// Get statement data
			var statement_desc = computed_parameters.context.statement_desc;
			var ce = getRowById('cross_examinations', statement_desc.ce_id);
			var statement_index = getIndexById(ce.statements, statement_desc.statement_id);
			
			if(statement_index < ce.statements.length - 1)
			{
				// If not the last statement, redirect to next
				player_status.next_frame_index = getRowIndexById('frames', ce.statements[statement_index + 1].id);
			}
			else if(ce.cocouncil_start != 0)
			{
				// If there is a cocouncil conversation, redirect to it
				player_status.next_frame_index = getRowIndexById('frames', ce.cocouncil_start);
			}
			else
			{
				// Else, redirect to start of the CE
				player_status.next_frame_index = getRowIndexById('frames', ce.start);
			}
			
			showProceedButton();
			
			break;
		
		case 'CEStatement':
			
				// If in CE mode, get statement data
				var statement_desc = computed_parameters.context.statement_desc;
				var ce = getRowById('cross_examinations', statement_desc.ce_id);
				var statement = getById(ce.statements, statement_desc.statement_id);
				
				setClass(bottom_screen, 'ce'); // Show ce buttons
				
				// Get target frames for each contradiction
				var target_frame_by_element = new Object();
				for(var i = 0; i < statement.contradictions.length; i++)
				{
					target_frame_by_element[statement.contradictions[i].contrad_elt.type + '_' + statement.contradictions[i].contrad_elt.id] = statement.contradictions[i].destination;
				}
				var failure_frame = ce.failure_conv_start || ce.start;
				
				// Set that information onto the present button
				var present_topright = document.getElementById('present-topright');
				present_topright.setAttribute('data-target-by-elt', JSON.stringify(target_frame_by_element));
				present_topright.setAttribute('data-fail-target', failure_frame);
				present_topright.setAttribute('data-return-frame', statement.id);
				
				if(statement.pressing_conv_start != 0)
				{
					// If there is a pressing conversation
					
					// Set press button to the start of the conversation
					document.getElementById('press').setAttribute('data-target-frame', getRowIndexById('frames', statement.pressing_conv_start));
					addClass(bottom_screen, 'press'); // Display the button
				}
			
			break;
		
		// Actions to handle dialogues
		
		case 'DialogueMenu':
			
			var dialogue_desc = computed_parameters.context.dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
			var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
			
			if(dialogue_desc.scene_type == 'scenes')
			{
				//If in a regular scene, display buttons for this dialogue and current examination of the scene
				
				var examination = getById(scene.examinations, scene.current_examination); // Get data for the current examination of the place
				
				// Set buttons to their target frame
				document.getElementById('options-investigation-examine').setAttribute('data-target-frame', getRowIndexById('frames', examination.examine));
				document.getElementById('options-investigation-move').setAttribute('data-target-frame', getRowIndexById('frames', scene.move));
				document.getElementById('options-investigation-talk').setAttribute('data-target-frame', getRowIndexById('frames', dialogue.talk));
				document.getElementById('options-investigation-present').setAttribute('data-target-frame', getRowIndexById('frames', dialogue.present));
				
				setClass(bottom_screen, 'options-investigation');
			}
			else
			{
				console.warn('AAI scenes not yet implemented'); // TODO : implement AAI scenes
			}
			
			break;
		
		case 'DialogueTalk':
			
			var dialogue_desc = computed_parameters.context.dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
			var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
			
			var options_element = document.getElementById('options');
			emptyNode(options_element); // Empty the list of options
			
			for(var i = 0; i < dialogue.talk_topics.length; i++)
			{
				var topic = dialogue.talk_topics[i];
				if(!topic.hidden)
				{
					var topic_element = document.createElement('a');
					setNodeTextContents(topic_element, topic.title); // Set option text
					registerEventHandler(topic_element, 'click', readFrame.bind(undefined, getRowIndexById('frames', topic.start)), false);
					
					options_element.appendChild(topic_element);
				}
			}
			
			// Set back button to the main frame of the dialogue
			document.getElementById('back').setAttribute('data-target-frame', getRowIndexById('frames', dialogue.main));
			
			setClass(bottom_screen, 'options back'); // Display the options panel and back button
			
			// Set the psyche locks button if needed
			if(dialogue.locks && !dialogue.locks.hidden)
			{
				document.getElementById('locks').setAttribute('data-target-frame', getRowIndexById('frames', dialogue.locks.start));
				addClass(bottom_screen, 'locks');
			}
			
			break;
		
		case 'DialoguePresent':
			
			var dialogue_desc = computed_parameters.context.dialogue;
			var scene = getById(trial_data[dialogue_desc.scene_type], dialogue_desc.scene_id);
			var dialogue = getById(scene[dialogue_desc.section_type], dialogue_desc.section_id);
			
			var target_frame_by_element = new Object();
			var failure_frame;
			for(var i = 0; i < dialogue.present_conversations.length; i++)
			{
				var elt;
				if(elt = dialogue.present_conversations[i].elt)
				{
					target_frame_by_element[elt.type + '_' + elt.id] = dialogue.present_conversations[i].start;
				}
				else
				{
					failure_frame = dialogue.present_conversations[i].start;
				}
			}
			
			generatePresentEvidence('all', target_frame_by_element, failure_frame, dialogue.main);
			
			break;
		
		// Actions to handle examinations
		
		case 'ExaminationExamine':
			
			var examination_desc = computed_parameters.context.examination;
			var scene = getById(trial_data[examination_desc.scene_type], examination_desc.scene_id);
			var examination = getById(scene[examination_desc.section_type], examination_desc.section_id);
			
			// Generate examination panel container
			var target_frame_by_area = new Object();
			var target_frame_by_object = new Object();
			for(var i = 0; i < examination.examine_conversations.length; i++)
			{
				var conv = examination.examine_conversations[i];
				if(conv.area === null)
				{
					// Null means default area
					target_frame_by_area['default'] = conv.start;
				}
				else if(is_scalar(conv.area))
				{
					// Area string
					target_frame_by_area[conv.area] = conv.start;
				}
				else
				{
					// Object descriptor
					target_frame_by_object[conv.area.layer + '#' + conv.area.id] = conv.start;
				}
			}
			generateExaminePlace(examination.place, target_frame_by_area, target_frame_by_object);
			
			// Set back button to the main frame of the current dialogue
			var dialogue = getById(scene.dialogues, scene.current_dialogue);
			
			document.getElementById('examination-back').setAttribute('data-target-frame', getRowIndexById('frames', dialogue.main));
			
			setClass(bottom_screen, 'examination examination-back'); // Display the examination panel
			
			break;
		
		// Actions to handle scenes
		
		case 'SceneMove':
			
			var scene_desc = computed_parameters.context.scene;
			var scene = getById(trial_data[scene_desc.scene_type], scene_desc.scene_id);
			
			var options_element = document.getElementById('options');
			emptyNode(options_element); // Empty the list of options
			
			for(var i = 0; i < scene.move_list.length; i++)
			{
				var move = scene.move_list[i];
				
				var target_scene = getById(trial_data[move.scene_type], move.scene_id);
				
				if(target_scene && !target_scene.hidden)
				{
					var move_element = document.createElement('a');
					setNodeTextContents(move_element, move.name_override || target_scene.name); // Set option text
					registerEventHandler(move_element, 'click', readFrame.bind(undefined, getRowIndexById('frames', target_scene.start)), false);
					
					options_element.appendChild(move_element);
				}
			}
			
			// Set back button to the main frame of the current dialogue
			var dialogue = getById(scene.dialogues, scene.current_dialogue);
			
			document.getElementById('back').setAttribute('data-target-frame', getRowIndexById('frames', dialogue.main));
			
			setClass(bottom_screen, 'options back'); // Display the options panel and back button
			
			break;
		
		default:
			showProceedButton();
			break;
	}
}

//END OF MODULE
Modules.complete('player_actions');
