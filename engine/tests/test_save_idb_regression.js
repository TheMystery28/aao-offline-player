"use strict";
/**
 * Regression tests for save system behavior after the IndexedDB migration.
 */

function testSaveIdbApiExists() {
	TestHarness.suite('Save IDB: API');

	TestHarness.assertType(getSaveData,      'function', 'getSaveData is a function');
	TestHarness.assertType(getSaveString,    'function', 'getSaveString is a function');
	TestHarness.assertType(loadSaveData,     'function', 'loadSaveData is a function');
	TestHarness.assertType(loadSaveString,   'function', 'loadSaveString is a function');
	TestHarness.assertType(refreshSavesList, 'function', 'refreshSavesList is a function');
}

function testSaveIdbRoundtrip() {
	TestHarness.suite('Save IDB: Roundtrip Integrity');

	if (typeof trial_data === 'undefined' || !trial_data) {
		TestHarness.assert(true, 'Skipped (no trial data loaded)');
		return;
	}

	var saveData = getSaveData();
	TestHarness.assertEqual(saveData.trial_id, trial_information.id, 'getSaveData.trial_id matches trial_information.id');
	TestHarness.assertType(saveData.save_date,             'number', 'getSaveData.save_date is a number');
	TestHarness.assertType(saveData.player_status,         'object', 'getSaveData.player_status is an object');
	TestHarness.assertType(saveData.trial_data_diffs,      'object', 'getSaveData.trial_data_diffs is an object');
	TestHarness.assertType(saveData.trial_data_base_dates, 'object', 'getSaveData.trial_data_base_dates is an object');
	TestHarness.assert('current_music_id' in saveData,            'getSaveData contains current_music_id');

	var saveString = getSaveString();
	var parseOk = true;
	try { JSON.parse(saveString); } catch(e) { parseOk = false; }
	TestHarness.assert(parseOk, 'getSaveString returns valid JSON');

	var before = getSaveData();
	loadSaveString(getSaveString());
	var after = getSaveData();
	TestHarness.assertEqual(after.player_status.current_frame_id, before.player_status.current_frame_id, 'Roundtrip preserves current_frame_id');
	TestHarness.assertEqual(after.player_status.health,           before.player_status.health,           'Roundtrip preserves health');
	TestHarness.assertEqual(after.current_music_id,               before.current_music_id,               'Roundtrip preserves current_music_id');
}

function testSaveIdbGuardBehaviors() {
	TestHarness.suite('Save IDB: Guard Behaviors');

	var xhr = new XMLHttpRequest();
	xhr.open('GET', 'Javascript/player_save.js', false);
	xhr.send();
	if (xhr.status !== 200 && xhr.status !== 0) {
		TestHarness.assert(false, 'Could not load player_save.js (status ' + xhr.status + ')');
		return;
	}
	var src = xhr.responseText;

	var saveStart = src.indexOf("registerEventHandler(save_button, 'click'");
	var saveEnd   = src.indexOf("btnRow.appendChild(save_button)");
	var saveSection = (saveStart !== -1 && saveEnd !== -1) ? src.substring(saveStart, saveEnd) : '';

	TestHarness.assert(saveStart !== -1, 'Save button handler present in source');
	TestHarness.assert(saveSection.indexOf('save_error_pending_timer') !== -1, 'Save button: timer guard enforced');
	TestHarness.assert(saveSection.indexOf('save_error_frame_typing')  !== -1, 'Save button: typing guard enforced');

	var autoStart   = src.indexOf("event.data.type === 'auto_save'");
	var autoEnd     = src.indexOf("Modules.complete('player_save')");
	var autoSection = (autoStart !== -1 && autoEnd !== -1) ? src.substring(autoStart, autoEnd) : '';

	TestHarness.assert(autoStart !== -1, 'Auto-save postMessage handler present in source');
	TestHarness.assert(autoSection.indexOf('proceed_timer')       !== -1, 'Auto-save: timer guard enforced');
	TestHarness.assert(autoSection.indexOf('proceed_typing')      !== -1, 'Auto-save: typing guard enforced');
	TestHarness.assert(autoSection.indexOf('auto_save_complete')  !== -1, 'Auto-save posts auto_save_complete to launcher');

	var loadStart = src.indexOf('function loadSaveData');
	var loadEnd   = src.indexOf('function loadSaveString');
	var loadBody  = (loadStart !== -1 && loadEnd !== -1) ? src.substring(loadStart, loadEnd) : '';
	TestHarness.assert(loadBody.indexOf('stopNonMusicSounds') !== -1, 'loadSaveData calls stopNonMusicSounds');

	TestHarness.assert(src.indexOf("EngineEvents.emit('save:created'") !== -1, "save:created event still emitted on save");
	TestHarness.assert(src.indexOf('save_data_changed')                !== -1, 'save_data_changed postMessage still sent to launcher');
}

function testSaveIdbBridgeProtocol() {
	TestHarness.suite('Save IDB: Bridge Protocol');

	var xhr = new XMLHttpRequest();
	xhr.open('GET', 'localstorage_bridge.html', false);
	xhr.send();
	if (xhr.status !== 200 && xhr.status !== 0) {
		TestHarness.assert(false, 'Could not load localstorage_bridge.html (status ' + xhr.status + ')');
		return;
	}
	var src = xhr.responseText;

	TestHarness.assert(src.indexOf('"game_saves"')       !== -1 || src.indexOf("'game_saves'")       !== -1, 'Bridge sends game_saves message');
	TestHarness.assert(src.indexOf('"write_saves"')      !== -1 || src.indexOf("'write_saves'")      !== -1, 'Bridge handles write_saves message');
	TestHarness.assert(src.indexOf('"write_saves_result"') !== -1 || src.indexOf("'write_saves_result'") !== -1, 'Bridge posts write_saves_result');
	TestHarness.assert(src.indexOf('bridgeId') !== -1, 'Bridge uses bridgeId for routing');
	TestHarness.assert(src.indexOf('merged')   !== -1, 'Bridge result includes merged count');
}

function testSaveIdbRefreshDoesNotThrow() {
	TestHarness.suite('Save IDB: refreshSavesList');

	var container = document.getElementById('player_saves');
	if (!container) {
		TestHarness.assert(true, 'Skipped: #player_saves not in DOM');
		return;
	}
	var threw = false;
	try { refreshSavesList(); } catch(e) { threw = true; }
	TestHarness.assert(!threw, 'refreshSavesList does not throw synchronously');
}
