"use strict";
/*
Ace Attorney Online - Functions to generate data about a trial

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'trial_data',
	dependencies : ['objects_model', 'trial_object_model', 'trial'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS

function getNewRowId(type)
{
	return getNewId(trial_data[type]);
}

function insertDataRow(type, position, new_row)
{
	var data_table = trial_data[type];

	//TODO remove debugging calls
	if(isNaN(new_row.id)) { debugger; }
	if(position === 0){ debugger; }

	if(!position && position !== 0)
	{
		//if position is undefined or non numeric, insert at end
		data_table.push(new_row);
	}
	else
	{
		//else, insert at given position
		data_table.splice(position, 0, new_row);
	}

	// Invalidate id index since array positions have changed.
	invalidateIdIndex(data_table);
}

function createDataRow(type, given_id)
{
	var newObject = getObjectFromModel(getTrialSubModelFromType(type));
	
	// Set the id of the new object as appropriate.
	if(('id' in newObject) && newObject.id === null)
	{
		// If new object has a null ID, it has to be defined. Either provided, or new unique ID.
		// Note : here, new unique ID only works for top level elements. ID must always be given for deeper elements.
		newObject.id = given_id || getNewRowId(type);
	}
	
	return newObject;
}

function createAndInsertDataRow(type, position)
{
	return createNewRowAt([type], position, trial_data, trial_object_model);
}

/*
Creates a data row based on a preset and inserts it.
*/
function createAndInsertDataRowFromPreset(type, position, preset)
{
	return createNewRowAt([type], position, trial_data, trial_object_model, preset);
}

function deleteDataRow(type, position)
{
	var data_table = trial_data[type];

	data_table.splice(position, 1);

	// Invalidate id index since array positions have changed.
	invalidateIdIndex(data_table);
}

function deleteDataRowInterval(type, start_index, end_index)
{
	for(var i = end_index; i >= start_index; i--)
	{
		deleteDataRow(type, i);
	}
}

function moveDataRowInterval(type, start_index, end_index, target_index)
{
	var data_table = trial_data[type];

	var nb_moved_rows = 1 + end_index - start_index;
	var rows = data_table.splice(start_index, nb_moved_rows);
	if(target_index > start_index)
	{
		// If target index after the removed interval, shift to the new target index before inserting.
		target_index -= nb_moved_rows;
	}
	data_table.splice.apply(data_table, [target_index, 0].concat(rows));

	// Invalidate id index since array positions have changed.
	invalidateIdIndex(data_table);
}

//Get the index in the trial data tables of the row that possesses a given ID
function getRowIndexById(type, id)
{
	return getIndexById(trial_data[type], id);
}

//Get the row that possesses a given ID
function getRowById(type, id)
{
	return getById(trial_data[type], id);
}

/*
Creation / Deletion of conversations 
*/
function createNewConversationAt(position, last_frame_properties, first_frame_properties)
{
	//Insert first conversation frame
	var first_frame = createAndInsertDataRow('frames', position);
	first_frame.wait_time = 1;
	
	// Set first frame properties
	if(first_frame_properties)
	{
		for(var prop_name in first_frame_properties)
		{
			first_frame[prop_name] = first_frame_properties[prop_name];
		}
	}
	
	//Insert last conversation frame
	var last_frame = createAndInsertDataRow('frames', position + 1);
	last_frame.wait_time = 1;
	
	// Set last frame properties
	if(last_frame_properties)
	{
		for(var prop_name in last_frame_properties)
		{
			last_frame[prop_name] = last_frame_properties[prop_name];
		}
	}
	
	// Build and return the conversation descriptor
	return new Object({
		start: first_frame.id,
		end: last_frame.id
	});
}

function deleteConversation(conv_descriptor)
{
	var first_frame = getRowIndexById('frames', conv_descriptor.start);
	var last_frame = getRowIndexById('frames', conv_descriptor.end);
	
	for(var i = last_frame; i >= first_frame; i--)
	{
		deleteDataRow('frames', i);
	}
}

/*
Creation of new blocks
*/

function createNewCrossExamination(position)
{
	// Add cocouncil_conversation frames
	var cocouncil_end_frame = createAndInsertDataRow('frames', position);
	var cocouncil_start_frame = createAndInsertDataRow('frames', position);
	
	// Add first frame
	var first_frame = createAndInsertDataRow('frames', position);
	
	var newCE = new Object({
		id: getNewRowId('cross_examinations'),
		start: first_frame.id,
		end: cocouncil_end_frame.id,
		cocouncil_start: cocouncil_start_frame.id,
		cocouncil_end: cocouncil_end_frame.id,
		statements: new Array(),
		failure_conv_start: 0,
		failure_conv_end: 0
	});
	
	trial_data.cross_examinations.push(newCE);
	
	first_frame.action_name = 'CEStart';
	first_frame.action_parameters = new Object();
	first_frame.wait_time = 1;
	
	cocouncil_start_frame.action_name = 'CEPause';
	cocouncil_start_frame.action_parameters = new Object();
	cocouncil_start_frame.wait_time = 1;
	
	cocouncil_end_frame.action_name = 'CERestart';
	cocouncil_end_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			ce_desc: newCE.id
		})
	}));
	cocouncil_end_frame.wait_time = 1;
}

function createNewDialogue(position, dialogue_descriptor)
{
	var dialogue_intro_first_frame = createAndInsertDataRow('frames', position);
	var dialogue_intro_hide_frame = createAndInsertDataRow('frames', position + 1);
	var dialogue_intro_last_frame = createAndInsertDataRow('frames', position + 2);
	var dialogue_menu_frame = createAndInsertDataRow('frames', position + 3);
	var dialogue_talk_frame = createAndInsertDataRow('frames', position + 4);
	var dialogue_present_frame = createAndInsertDataRow('frames', position + 5);
	var dialogue_present_others_first_frame = createAndInsertDataRow('frames', position + 6);
	var dialogue_present_others_return_frame = createAndInsertDataRow('frames', position + 7);
	var dialogue_end_frame = createAndInsertDataRow('frames', position + 8);
	
	var newDialogue = new Object({
		id: dialogue_descriptor.section_id,
		
		start: dialogue_intro_first_frame.id,
		main: dialogue_menu_frame.id,
		talk: dialogue_talk_frame.id,
		present: dialogue_present_frame.id,
		end: dialogue_end_frame.id,
		
		intro_start: dialogue_intro_first_frame.id,
		intro_end: dialogue_intro_last_frame.id,
		
		talk_topics: [],
		present_conversations: [new Object({
			elt: null,
			start: dialogue_present_others_first_frame.id,
			end: dialogue_present_others_return_frame.id
		})],
		locks: null
	});
	
	dialogue_intro_first_frame.wait_time = 1;
	dialogue_intro_first_frame.hidden = 1;
	dialogue_intro_first_frame.action_name = 'GoTo';
	dialogue_intro_first_frame.action_parameters = prefixRawParameters(new Object({
		global: new Object({
			target: dialogue_menu_frame.id
		})
	}));
	
	dialogue_intro_hide_frame.wait_time = 1;
	dialogue_intro_hide_frame.action_name = 'RevealFrame';
	dialogue_intro_hide_frame.action_parameters = prefixRawParameters(new Object({
		multiple: new Object({
			frame: [new Object({
					target: dialogue_intro_first_frame.id
			})]
		})
	}));
	
	dialogue_intro_last_frame.wait_time = 1;
	dialogue_intro_last_frame.action_name = 'GoTo';
	dialogue_intro_last_frame.action_parameters = prefixRawParameters(new Object({
		global: new Object({
			target: dialogue_menu_frame.id
		})
	}));
	
	dialogue_menu_frame.action_name = 'DialogueMenu';
	dialogue_menu_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			dialogue: dialogue_descriptor
		})
	}));
	
	dialogue_talk_frame.action_name = 'DialogueTalk';
	dialogue_talk_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			dialogue: dialogue_descriptor
		})
	}));
	
	dialogue_present_frame.action_name = 'DialoguePresent';
	dialogue_present_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			dialogue: dialogue_descriptor
		})
	}));
	
	dialogue_present_others_first_frame.wait_time = 1;
	
	dialogue_present_others_return_frame.wait_time = 1;
	dialogue_present_others_return_frame.action_name = 'GoTo';
	dialogue_present_others_return_frame.action_parameters = prefixRawParameters(new Object({
		global: new Object({
			target: dialogue_menu_frame.id
		})
	}));
	
	return newDialogue;
}

function createNewExamination(position, examination_descriptor)
{
	var examination_examine_frame = createAndInsertDataRow('frames', position);
	var examination_examine_others_first_frame = createAndInsertDataRow('frames', position + 1);
	var examination_examine_others_return_frame = createAndInsertDataRow('frames', position + 2);
	var examination_deduce_others_first_frame = createAndInsertDataRow('frames', position + 3);
	var examination_deduce_others_return_frame = createAndInsertDataRow('frames', position + 4);
	var examination_end_frame = createAndInsertDataRow('frames', position + 5);
	
	var newExamination = new Object({
		id: examination_descriptor.section_id,
		
		start: examination_examine_frame.id,
		examine: examination_examine_frame.id,
		end: examination_end_frame.id,
		
		place: PLACE_NONE,
		
		examine_conversations: [new Object({
			area: null,
			start: examination_examine_others_first_frame.id,
			end: examination_examine_others_return_frame.id
		})],
		
		deduce_conversations: [new Object({
			area: null,
			elt: null,
			start: examination_deduce_others_first_frame.id,
			end: examination_deduce_others_return_frame.id
		})],
		
		enable_deduction: false
	});
	
	examination_examine_frame.action_name = 'ExaminationExamine';
	examination_examine_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			examination: examination_descriptor
		})
	}));
	
	examination_examine_others_first_frame.wait_time = 1;
	
	examination_examine_others_return_frame.wait_time = 1;
	examination_examine_others_return_frame.action_name = 'GoTo';
	examination_examine_others_return_frame.action_parameters = prefixRawParameters(new Object({
		global: new Object({
			target: examination_examine_frame.id
		})
	}));
	
	examination_deduce_others_first_frame.wait_time = 1;
	
	examination_deduce_others_return_frame.wait_time = 1;
	examination_deduce_others_return_frame.action_name = 'GoTo';
	examination_deduce_others_return_frame.action_parameters = prefixRawParameters(new Object({
		global: new Object({
			target: examination_examine_frame.id
		})
	}));
	
	return newExamination;
}

function createNewScene(position)
{
	var scene_id = getNewRowId('scenes');
	
	var scene_first_frame = createAndInsertDataRow('frames', position);
	
	var newDialogue = createNewDialogue(position + 1, 
		new Object({
			scene_type: 'scenes',
			scene_id: scene_id,
			section_type: 'dialogues',
			section_id: 1
		})
	);
	
	var dialogue_end_position = getRowIndexById('frames', newDialogue.end);
	
	var newExamination = createNewExamination(dialogue_end_position + 1, 
		new Object({
			scene_type: 'scenes',
			scene_id: scene_id,
			section_type: 'examinations',
			section_id: 1
		})
	);
	
	var examination_end_position = getRowIndexById('frames', newExamination.end);
	
	var scene_move_frame = createAndInsertDataRow('frames', examination_end_position + 1);
	
	var newScene = new Object({
		id: scene_id,
		name: '',
		hidden: false,
		
		dialogues: [newDialogue],
		current_dialogue: 1,
		
		examinations: [newExamination],
		current_examination: 1,
		
		start: scene_first_frame.id,
		move: scene_move_frame.id,
		end: scene_move_frame.id,
		
		move_list: []
	});
	
	trial_data.scenes.push(newScene);
	
	scene_first_frame.wait_time = 1;
	scene_first_frame.action_name = 'SceneStart';
	scene_first_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			scene: new Object({
				scene_type: 'scenes',
				scene_id: newScene.id,
			})
		})
	}));
	
	scene_move_frame.action_name = 'SceneMove';
	scene_move_frame.action_parameters = prefixRawParameters(new Object({
		context: new Object({
			scene: new Object({
				scene_type: 'scenes',
				scene_id: newScene.id,
			})
		})
	}));
}

/*
Deletion of blocks 
*/
function deleteFramesBlock(block_type, block_id)
{
	var block_index = getRowIndexById(block_type, block_id);
	var block = trial_data[block_type][block_index];
	
	// Delete all frames between start and end of the block
	var start_frame_index = getRowIndexById('frames', block.start);
	var end_frame_index = getRowIndexById('frames', block.end);

	trial_data.frames.splice(start_frame_index, 1 + end_frame_index - start_frame_index);
	invalidateIdIndex(trial_data.frames);

	// Delete block data
	trial_data[block_type].splice(block_index, 1);
	invalidateIdIndex(trial_data[block_type]);
}

//END OF MODULE
Modules.complete('trial_data');
