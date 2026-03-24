"use strict";
/**
 * Test runner entry point.
 * Waits for the player module to finish loading, rebuilds any DOM elements
 * destroyed by player_init()'s no-trial-data path, then executes all test
 * suites and renders the report.
 */
(function() {
	function runSafe(name, fn) {
		try {
			fn();
		} catch (e) {
			console.error('[TEST RUNNER] Suite "' + name + '" threw: ' + (e.stack || e));
			TestHarness.suite(name);
			TestHarness.assert(false, 'SUITE CRASHED: ' + e);
		}
	}

	function rebuildDOM() {
		var content = document.getElementById('content');
		if (!content) return;

		// If #content was emptied by player_init (no trial data), rebuild
		if (content.children.length > 0) return;

		console.warn('[TEST RUNNER] #content was emptied by player_init. Rebuilding DOM...');

		var section = document.createElement('section');
		content.appendChild(section);

		var screens = document.createElement('div');
		screens.id = 'screens';
		screens.className = 'start';
		section.appendChild(screens);

		var screenBottom = document.createElement('div');
		screenBottom.id = 'screen-bottom';
		screens.appendChild(screenBottom);

		var buttonIds = [
			{ id: 'proceed', tag: 'a', cls: 'bs-button center' },
			{ id: 'start', tag: 'a', cls: 'bs-button center' },
			{ id: 'skip', tag: 'a', cls: 'bs-button center' },
			{ id: 'statement-backwards', tag: 'a', cls: 'bs-button left' },
			{ id: 'statement-forwards', tag: 'a', cls: 'bs-button right' },
			{ id: 'statement-skip-forwards', tag: 'a', cls: 'bs-button right' },
			{ id: 'back', tag: 'a', cls: 'bs-button bottomleft' },
			{ id: 'press', tag: 'a', cls: 'bs-button topleft' },
			{ id: 'present-center', tag: 'a', cls: 'bs-button topmiddle' },
			{ id: 'present-topright', tag: 'a', cls: 'bs-button topright' }
		];
		for (var i = 0; i < buttonIds.length; i++) {
			var btn = document.createElement(buttonIds[i].tag);
			btn.id = buttonIds[i].id;
			btn.className = buttonIds[i].cls;
			screenBottom.appendChild(btn);
		}

		var screenTop = document.createElement('div');
		screenTop.id = 'screen-top';
		screens.appendChild(screenTop);

		var evidDisplay = document.createElement('div');
		evidDisplay.id = 'evidence-display';
		evidDisplay.className = 'evidence_display';
		screenBottom.appendChild(evidDisplay);

		var courtrecord = document.createElement('div');
		courtrecord.id = 'courtrecord';
		courtrecord.className = 'evidence';
		section.appendChild(courtrecord);

		var crEvidence = document.createElement('section');
		crEvidence.id = 'cr_evidence';
		courtrecord.appendChild(crEvidence);
		var crEvidenceList = document.createElement('div');
		crEvidenceList.id = 'cr_evidence_list';
		crEvidenceList.className = 'evidence-list';
		crEvidence.appendChild(crEvidenceList);

		var crProfiles = document.createElement('section');
		crProfiles.id = 'cr_profiles';
		courtrecord.appendChild(crProfiles);
		var crProfilesList = document.createElement('div');
		crProfilesList.id = 'cr_profiles_list';
		crProfilesList.className = 'evidence-list';
		crProfiles.appendChild(crProfilesList);

		var crItemCheck = document.createElement('aside');
		crItemCheck.id = 'cr_item_check';
		courtrecord.appendChild(crItemCheck);

		var crSwitchProfiles = document.createElement('a');
		crSwitchProfiles.id = 'cr_profiles_switch';
		crEvidence.appendChild(crSwitchProfiles);

		var crSwitchEvidence = document.createElement('a');
		crSwitchEvidence.id = 'cr_evidence_switch';
		crProfiles.appendChild(crSwitchEvidence);

		var crCheckBack = document.createElement('a');
		crCheckBack.id = 'cr-item-check-back';
		crCheckBack.className = 'bs-button bottomleft';
		crItemCheck.appendChild(crCheckBack);

		var playerParams = document.createElement('div');
		playerParams.id = 'player-parametres';
		section.appendChild(playerParams);

		var savesDetails = document.createElement('details');
		savesDetails.open = true;
		var savesSummary = document.createElement('summary');
		savesSummary.setAttribute('data-locale-content', 'player_saves');
		savesDetails.appendChild(savesSummary);
		var savesSectionContent = document.createElement('div');
		savesSectionContent.className = 'settings-section-content';
		savesDetails.appendChild(savesSectionContent);
		var playerSaves = document.createElement('div');
		playerSaves.id = 'player_saves';
		savesSectionContent.appendChild(playerSaves);
		playerParams.appendChild(savesDetails);

		var settingsDetails = document.createElement('details');
		settingsDetails.open = true;
		var settingsSummary = document.createElement('summary');
		settingsSummary.setAttribute('data-locale-content', 'player_settings');
		settingsDetails.appendChild(settingsSummary);
		var settingsSectionContent = document.createElement('div');
		settingsSectionContent.className = 'settings-section-content';
		settingsDetails.appendChild(settingsSectionContent);
		var playerSettings = document.createElement('div');
		playerSettings.id = 'player_settings';
		settingsSectionContent.appendChild(playerSettings);
		playerParams.appendChild(settingsDetails);

		// Re-register court record switch handlers
		crSwitchProfiles.addEventListener('click', function() {
			courtrecord.className = 'profiles';
		});
		crSwitchEvidence.addEventListener('click', function() {
			courtrecord.className = 'evidence';
		});
		crCheckBack.addEventListener('click', function() {
			var c = document.getElementById('content');
			if (c) c.classList.remove('cr-check');
		});
	}

	function executeTests() {
		console.log('[TEST RUNNER] Running test suites...');

		// Rebuild DOM if player_init destroyed it
		rebuildDOM();

		var suites = [
			['Modules', typeof testModules === 'function' ? testModules : null],
			['Events', typeof testEvents === 'function' ? testEvents : null],
			['Nodes', typeof testNodes === 'function' ? testNodes : null],
			['Frame Data', typeof testFrameData === 'function' ? testFrameData : null],
			['Player', typeof testPlayer === 'function' ? testPlayer : null],
			['Court Record', typeof testCourtRecord === 'function' ? testCourtRecord : null],
			['Controls', typeof testControls === 'function' ? testControls : null],
			['Save', typeof testSave === 'function' ? testSave : null],
			['Sound', typeof testSound === 'function' ? testSound : null],
			['Display', typeof testDisplay === 'function' ? testDisplay : null],
			['Expression Engine', typeof testExpressionEngine === 'function' ? testExpressionEngine : null],
			['Language', typeof testLanguage === 'function' ? testLanguage : null],
			['Objects', typeof testObjects === 'function' ? testObjects : null],
		['EngineEvents', typeof testEngineEvents === 'function' ? testEngineEvents : null],
		['EngineConfig', typeof testEngineConfig === 'function' ? testEngineConfig : null],
		['InputManager', typeof testInputManager === 'function' ? testInputManager : null],
		['ThemeManager', typeof testThemeManager === 'function' ? testThemeManager : null],
		['SettingsPanel', typeof testSettingsPanel === 'function' ? testSettingsPanel : null],
		['EngineConfig Migration', typeof testEngineConfigMigration === 'function' ? testEngineConfigMigration : null]
		];

		for (var i = 0; i < suites.length; i++) {
			if (suites[i][1]) {
				runSafe(suites[i][0], suites[i][1]);
			}
		}

		var allPassed = TestHarness.report();
		console.log('[TEST RUNNER] ' + (allPassed ? 'ALL TESTS PASSED' : 'SOME TESTS FAILED'));
	}

	// Wait for the player module to reach status 3 (operational).
	// We can't use Modules.request('player', callback) because the <head> already
	// called Modules.request('player') without a callback, and the module system
	// silently drops callbacks when the module is already in loading state (1 or 2).
	// So we poll instead.
	var pollInterval = setInterval(function() {
		if (Modules.request_list['player'] === 3) {
			clearInterval(pollInterval);
			console.log('[TEST RUNNER] player module loaded, preparing test environment...');

			// player_init() runs asynchronously (after language files load via XHR).
			// Give it time to finish (it empties #content when no trial data),
			// then rebuild the DOM and run tests.
			setTimeout(function() {
				executeTests();
			}, 500);
		}
	}, 50);
})();
