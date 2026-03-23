"use strict";
/*
Ace Attorney Online - Player game saving module

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'player_save',
	dependencies : ['engine_events', 'objects_diff', 'trial', 'base64', 'var_environments', 'player_sound', 'player_debug', 'nodes', 'events'],
	init : function() { }
}));


//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES
var trial_data_diffs = {};
var trial_data_base_dates = {};


//EXPORTED FUNCTIONS
function getSaveData()
{
	trial_data_diffs[trial_information.id] = getDiff(initial_trial_data, trial_data);
	trial_data_base_dates[trial_information.id] = trial_information.last_edit_date;
	
	var save = {
		trial_id: trial_information.id,
		save_date: Math.round(new Date().getTime() / 1000), // Store save date as a UNIX timestamp.
		player_status: player_status,
		top_screen_state: top_screen.state,
		current_music_id: current_music_id,
		trial_data_diffs: trial_data_diffs,
		trial_data_base_dates: trial_data_base_dates
	};
	
	return save;
}

function getSaveString()
{
	return JSON.stringify(getSaveData());
}

function loadSaveData(save)
{
	if(save.trial_id != trial_information.id)
	{
		// Check if the save is from another part of the same sequence
		if(trial_information.sequence && trial_information.sequence.list)
		{
			var seq_ids = trial_information.sequence.list.map(function(p) { return p.id; });
			if(seq_ids.indexOf(save.trial_id) !== -1)
			{
				// Redirect to that part with the save data
				var url = new URL(window.location.href);
				url.searchParams.set('trial_id', save.trial_id);
				url.searchParams.set('save_data', Base64.encode(JSON.stringify(save)));
				window.location.href = url.toString();
				return;
			}
		}
		alert(l('trial_doesnt_match_save'));
		return;
	}
	
	trial_data_diffs = save.trial_data_diffs;
	trial_data_base_dates = save.trial_data_base_dates;
	
	if(save.trial_data_base_dates[trial_information.id] < trial_information.last_edit_date)
	{
		// If trial edited since the game was saved, display a warning.
		alert(l('trial_edited_since_save'));
	}
	
	// Patch trial data using the provided diff.
	trial_data = patch(initial_trial_data, save.trial_data_diffs[save.trial_id]);
	
	refreshCrElements();
	if(save.top_screen_state)
	{
		top_screen.state = save.top_screen_state;
	}
	player_status = save.player_status;
	top_screen.setVariableEnvironment(player_status.game_env);
	try { playMusic(save.current_music_id); } catch(e) { /* sounds may not be loaded yet during auto-start */ }
	refreshHealthBar();
	
	// Refresh all debuggers.
	debugRefreshStatus();
	debugRefreshVars();
	debugRefreshCourtRecords();
	debugRefreshScenes();
	debugRefreshFrames();
	
	if(!player_status.current_frame_index && !player_status.next_frame_index)
	{
		// If no data on current frame is found, save comes from jump link.
		if(player_status.next_frame_id)
		{
			// If target frame provided, read it immediately.
			readFrame(getRowIndexById('frames', player_status.next_frame_id));
		}
		else
		{
			// Else start at the beginning.
			readFrame(1);
		}
		delete player_status.next_frame_id;
	}
	else
	{
		// Loading an actual state save with current state.
		if(player_status.last_frame_merged)
		{
			// If merged frame, proceed immediately to next frame.
			readFrame(player_status.next_frame_index);
		}
		else
		{
			// Else check current frame data to detemine proper UI to display.
			// TODO : improve mechanism with actual UI state save ?
			var frame_data = trial_data.frames[player_status.current_frame_index];
			
			switch(frame_data.action_name)
			{
				case 'MultipleChoices':
				case 'AskForEvidence':
				case 'PointArea':
				case 'InputVars':
				case 'CEStatement':
				case 'DialogueMenu':
				case 'DialogueTalk':
				case 'DialoguePresent':
				case 'ExaminationExamine':
				case 'SceneMove':
					// For player input actions, re-run actions to reload action UI.
					runFrameActionAfter(frame_data, computeParameters(frame_data.action_parameters, player_status.game_env));
					break;
				
				default:
					// Else, display proceed button.
					setClass(bottom_screen, 'proceed');
					break;
			}
		}
	}
	
	// Save loaded : remove start overlay if present.
	removeClass(document.getElementById('screens'), 'start');

	EngineEvents.emit('save:loaded', { saveData: save });
}

function loadSaveString(save_string)
{
	loadSaveData(JSON.parse(save_string, function(key, value) {
		if(key == 'game_env')
		{
			// Restore dynamic variable environment from its JSON dump.
			var env = new VariableEnvironment();
			for(var var_name in value)
			{
				env.set(var_name, value[var_name]);
			}
			return env;
		}
		else
		{
			return value;
		}
	}));
}

function refreshSavesList()
{
	var container = document.getElementById('player_saves');
	var btnContainer = document.getElementById('player_saves_buttons');

	emptyNode(container);

	if(window.localStorage)
	{
		var game_saves = JSON.parse(window.localStorage.getItem('game_saves'));

		// --- Save + Load buttons outside the scroll area (always visible) ---
		if (btnContainer) { emptyNode(btnContainer); }
		var btnRow = document.createElement('div');
		btnRow.style.cssText = 'display:flex;gap:4px;margin-bottom:4px;';

		var save_button = document.createElement('button');
		addClass(save_button, 'save_new');
		save_button.textContent = l('save_new') || 'New save';
		save_button.style.cssText = 'flex:1;padding:6px 8px;';
		registerEventHandler(save_button, 'click', function(){
			if(player_status.current_frame_index == 0)
			{
				alert(l('save_error_game_not_started'));
			}
			else if(player_status.proceed_timer && !player_status.proceed_timer_met)
			{
				alert(l('save_error_pending_timer'));
			}
			else if(player_status.proceed_typing && !player_status.proceed_typing_met)
			{
				alert(l('save_error_frame_typing'));
			}
			else
			{
				var gs = JSON.parse(window.localStorage.getItem('game_saves'));
				if(!gs)
				{
					alert(l('save_explain'));
					gs = {};
				}
				if(!gs[trial_information.id])
				{
					gs[trial_information.id] = {};
				}
				var saveStr = getSaveString();
				gs[trial_information.id][(new Date()).getTime()] = saveStr;
				window.localStorage.setItem('game_saves', JSON.stringify(gs));
				EngineEvents.emit('save:created', { saveData: JSON.parse(saveStr) });
				refreshSavesList();
			}
		}, false);
		btnRow.appendChild(save_button);

		var load_button = document.createElement('button');
		load_button.textContent = l('load_latest') || 'Load latest';
		load_button.style.cssText = 'flex:1;padding:6px 8px;';
		registerEventHandler(load_button, 'click', function(){
			var gs = JSON.parse(window.localStorage.getItem('game_saves'));
			if(!gs) return;
			// Find the latest save across ALL sequence parts
			var latestDate = 0;
			var latestPartId = null;
			var latestStr = null;
			var partsToCheck = [trial_information.id];
			if (trial_information.sequence && trial_information.sequence.list) {
				for (var si = 0; si < trial_information.sequence.list.length; si++) {
					partsToCheck.push(trial_information.sequence.list[si].id);
				}
			}
			for (var pi = 0; pi < partsToCheck.length; pi++) {
				var pid = partsToCheck[pi];
				if (!gs[pid]) continue;
				var dates = Object.keys(gs[pid]).map(Number);
				for (var di = 0; di < dates.length; di++) {
					if (dates[di] > latestDate) {
						latestDate = dates[di];
						latestPartId = pid;
						latestStr = gs[pid][String(dates[di])];
					}
				}
			}
			if (!latestStr) return;
			if(player_status.proceed_timer && !player_status.proceed_timer_met)
			{
				alert(l('save_error_pending_timer'));
			}
			else if(player_status.proceed_typing && !player_status.proceed_typing_met)
			{
				alert(l('save_error_frame_typing'));
			}
			else
			{
				if (latestPartId == trial_information.id) {
					loadSaveString(latestStr);
				} else {
					// Redirect to the other part with save data
					var url = new URL(window.location.href);
					url.searchParams.set('trial_id', latestPartId);
					url.searchParams.set('save_data', Base64.encode(latestStr));
					window.location.href = url.toString();
				}
			}
		}, false);
		btnRow.appendChild(load_button);

		(btnContainer || container).appendChild(btnRow);

		// --- Unified save list: merge all parts, sort by date, show headers on part change ---
		// Build a flat array of { date, partId, saveString, title, isCurrent }
		var allSaves = [];
		var partTitles = {};
		partTitles[trial_information.id] = null; // current part = no header

		// Collect current part saves
		if (game_saves && game_saves[trial_information.id]) {
			var curDates = Object.keys(game_saves[trial_information.id]);
			for (var ci = 0; ci < curDates.length; ci++) {
				allSaves.push({ date: Number(curDates[ci]), partId: trial_information.id, saveString: game_saves[trial_information.id][curDates[ci]], isCurrent: true });
			}
		}

		// Collect sequence part saves
		if (trial_information.sequence && trial_information.sequence.list && game_saves) {
			var seq_list = trial_information.sequence.list;
			for (var si = 0; si < seq_list.length; si++) {
				var pid = seq_list[si].id;
				partTitles[pid] = seq_list[si].title || ('Part ' + pid);
				if (pid == trial_information.id || !game_saves[pid]) continue;
				var pDates = Object.keys(game_saves[pid]);
				for (var pi = 0; pi < pDates.length; pi++) {
					allSaves.push({ date: Number(pDates[pi]), partId: pid, saveString: game_saves[pid][pDates[pi]], isCurrent: false });
				}
			}
		}

		// Sort all saves latest-first
		allSaves.sort(function(a, b) { return b.date - a.date; });

		// Render with part headers on every part change
		var lastPartId = null;
		for (var ai = 0; ai < allSaves.length; ai++) {
			(function(entry) {
				// Show part header when part changes (skip header for current part at the very top)
				if (entry.partId !== lastPartId && !(lastPartId === null && entry.isCurrent)) {
					var title = entry.isCurrent ? (trial_information.title || 'Current part') : (partTitles[entry.partId] || ('Part ' + entry.partId));
					var divider = document.createElement('div');
					divider.style.cssText = 'margin-top:6px;padding-top:3px;border-top:1px solid #444;font-size:0.85em;color:#aaa;';
					divider.textContent = title;
					container.appendChild(divider);
				}
				lastPartId = entry.partId;

				// Delete button (only for current part saves)
				if (entry.isCurrent) {
					var del = document.createElement('button');
					registerEventHandler(del, 'click', function() {
						delete game_saves[entry.partId][String(entry.date)];
						if (Object.keys(game_saves[entry.partId]).length === 0) delete game_saves[entry.partId];
						window.localStorage.setItem('game_saves', JSON.stringify(game_saves));
						refreshSavesList();
					}, false);
					setNodeTextContents(del, '×');
					container.appendChild(del);
				}

				var save_link = document.createElement('a');
				var url = new URL(window.location.href);
				if (!entry.isCurrent) url.searchParams.set('trial_id', entry.partId);
				url.searchParams.set('save_data', Base64.encode(entry.saveString));
				save_link.href = url.toString();

				registerEventHandler(save_link, 'click', function(event) {
					if (player_status.proceed_timer && !player_status.proceed_timer_met) {
						alert(l('save_error_pending_timer'));
					} else if (player_status.proceed_typing && !player_status.proceed_typing_met) {
						alert(l('save_error_frame_typing'));
					} else {
						if (entry.isCurrent) {
							loadSaveString(entry.saveString);
						} else {
							window.location.href = save_link.href;
						}
					}
					event.preventDefault();
				}, false);
				setNodeTextContents(save_link, (new Date(entry.date)).toLocaleString());
				container.appendChild(save_link);
			})(allSaves[ai]);
		}

		translateNode(container);
	}
}

// Auto-save via postMessage from the launcher (triggered when quitting)
window.addEventListener('message', function(event) {
	if (event.data && event.data.type === 'auto_save') {
		// Only save if the game has started and is in a saveable state
		if (typeof player_status !== 'undefined' && player_status.current_frame_index > 0
			&& !(player_status.proceed_timer && !player_status.proceed_timer_met)
			&& !(player_status.proceed_typing && !player_status.proceed_typing_met))
		{
			try {
				var game_saves = JSON.parse(window.localStorage.getItem('game_saves')) || {};
				if (!game_saves[trial_information.id]) {
					game_saves[trial_information.id] = {};
				}
				var saveStr = getSaveString();
				game_saves[trial_information.id][(new Date()).getTime()] = saveStr;
				window.localStorage.setItem('game_saves', JSON.stringify(game_saves));
				EngineEvents.emit('save:created', { saveData: JSON.parse(saveStr) });
				console.log('[SAVE] Auto-saved on quit');
			} catch (e) {
				console.warn('[SAVE] Auto-save error:', e.message);
			}
		}
	}
});

//END OF MODULE
Modules.complete('player_save');
