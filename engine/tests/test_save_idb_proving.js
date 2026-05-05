"use strict";
/**
 * Structural tests verifying IndexedDB is used for game saves.
 * Uses source-code inspection (synchronous XHR).
 */

function testSaveIdbHelperExists() {
	TestHarness.suite('Save IDB: GameSavesDB Helper');

	TestHarness.assert(
		typeof GameSavesDB !== 'undefined',
		'GameSavesDB helper is defined in engine scope'
	);
	if (typeof GameSavesDB !== 'undefined') {
		TestHarness.assertType(GameSavesDB.get, 'function', 'GameSavesDB.get is a function');
		TestHarness.assertType(GameSavesDB.set, 'function', 'GameSavesDB.set is a function');
	}
}

function testSaveIdbPlayerSaveUsesIdb() {
	TestHarness.suite('Save IDB: player_save.js Storage Backend');

	var xhr = new XMLHttpRequest();
	xhr.open('GET', 'Javascript/player_save.js', false);
	xhr.send();
	if (xhr.status !== 200 && xhr.status !== 0) {
		TestHarness.assert(false, 'Could not load player_save.js (status ' + xhr.status + ')');
		return;
	}
	var src = xhr.responseText;

	TestHarness.assert(
		src.indexOf('aao_saves') !== -1,
		'player_save.js uses IndexedDB database "aao_saves"'
	);
	TestHarness.assert(
		src.indexOf('GameSavesDB.set(') !== -1,
		'player_save.js calls GameSavesDB.set() to write saves'
	);
	TestHarness.assert(
		src.indexOf('GameSavesDB.get(') !== -1,
		'player_save.js calls GameSavesDB.get() to read saves'
	);

	var saveStart = src.indexOf("registerEventHandler(save_button, 'click'");
	var saveEnd   = src.indexOf("btnRow.appendChild(save_button)");
	var saveSection = (saveStart !== -1 && saveEnd !== -1) ? src.substring(saveStart, saveEnd) : '';
	TestHarness.assert(
		saveSection.indexOf("localStorage.setItem") === -1,
		'Save button handler does not use localStorage.setItem'
	);

	var autoStart = src.indexOf("event.data.type === 'auto_save'");
	var autoEnd   = src.indexOf("Modules.complete('player_save')");
	var autoSection = (autoStart !== -1 && autoEnd !== -1) ? src.substring(autoStart, autoEnd) : '';
	TestHarness.assert(
		autoSection.indexOf("localStorage.setItem") === -1,
		'Auto-save handler does not use localStorage.setItem'
	);
}

function testSaveIdbBridgeUsesIdb() {
	TestHarness.suite('Save IDB: localstorage_bridge.html Storage Backend');

	var xhr = new XMLHttpRequest();
	xhr.open('GET', 'localstorage_bridge.html', false);
	xhr.send();
	if (xhr.status !== 200 && xhr.status !== 0) {
		TestHarness.assert(false, 'Could not load localstorage_bridge.html (status ' + xhr.status + ')');
		return;
	}
	var src = xhr.responseText;

	TestHarness.assert(
		src.indexOf('indexedDB.open') !== -1,
		'localstorage_bridge.html uses indexedDB.open'
	);
	TestHarness.assert(
		src.indexOf('aao_saves') !== -1,
		'localstorage_bridge.html references "aao_saves" IndexedDB database'
	);
	TestHarness.assert(
		src.indexOf('localStorage.setItem') === -1,
		'localstorage_bridge.html does not use localStorage.setItem'
	);
	TestHarness.assert(
		src.indexOf('localStorage.getItem') === -1,
		'localstorage_bridge.html does not use localStorage.getItem'
	);
}
