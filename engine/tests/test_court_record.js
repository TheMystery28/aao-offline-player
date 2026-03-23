"use strict";
/**
 * Court record regression tests (EXHAUSTIVE).
 */
function testCourtRecord() {
	TestHarness.suite('Court Record');

	// Function existence
	TestHarness.assertType(generateCrElement, 'function', 'generateCrElement is a function');
	TestHarness.assertType(populateCrElementSummary, 'function', 'populateCrElementSummary is a function');
	TestHarness.assertType(setCrElementHidden, 'function', 'setCrElementHidden is a function');
	TestHarness.assertType(refreshCrElements, 'function', 'refreshCrElements is a function');
	if (typeof selectCrElement === 'function') {
		TestHarness.assertType(selectCrElement, 'function', 'selectCrElement is a function');
	}

	// Tests that require trial_data (court record is populated only when a case is loaded)
	if (typeof trial_data !== 'undefined' && trial_data) {
		// generateCrElement creates a div element
		if (trial_data.evidence && trial_data.evidence.length > 1) {
			var crEl = generateCrElement('evidence', trial_data.evidence[1].id);
			TestHarness.assertEqual(crEl.tagName, 'DIV', 'generateCrElement creates a div element');

			// generateCrElement result has summary child
			var hasSummary = false;
			var hasDetails = false;
			for (var i = 0; i < crEl.children.length; i++) {
				if (crEl.children[i].classList.contains('summary')) hasSummary = true;
				if (crEl.children[i].classList.contains('details')) hasDetails = true;
			}
			TestHarness.assert(hasSummary, 'generateCrElement result has summary child');
			TestHarness.assert(hasDetails, 'generateCrElement result has details child');
		}

		// setCrElementHidden tests
		if (trial_data.profiles && trial_data.profiles.length > 1) {
			var profileId = trial_data.profiles[1].id;
			var originalHidden = trial_data.profiles[1].hidden;

			setCrElementHidden('profiles', profileId, true);
			TestHarness.assert(trial_data.profiles[1].hidden === true, 'setCrElementHidden(profiles, id, true) hides the profile');

			setCrElementHidden('profiles', profileId, false);
			TestHarness.assert(trial_data.profiles[1].hidden === false, 'setCrElementHidden(profiles, id, false) unhides the profile');

			// Restore original state
			setCrElementHidden('profiles', profileId, originalHidden);
		}

		if (trial_data.evidence && trial_data.evidence.length > 1) {
			var evidenceId = trial_data.evidence[1].id;
			var origHidden = trial_data.evidence[1].hidden;

			setCrElementHidden('evidence', evidenceId, true);
			TestHarness.assert(trial_data.evidence[1].hidden === true, 'setCrElementHidden(evidence, id, true) hides the evidence');

			// Restore
			setCrElementHidden('evidence', evidenceId, origHidden);
		}

		// refreshCrElements updates DOM to match trial_data hidden states
		var refreshOk = true;
		try { refreshCrElements(); } catch (e) { refreshOk = false; }
		TestHarness.assert(refreshOk, 'refreshCrElements updates DOM without error');
	}

	// DOM element existence (these should exist from our test_runner.html mirror)
	TestHarness.assert(document.getElementById('cr_evidence_list') !== null, 'Evidence list container #cr_evidence_list exists');
	TestHarness.assert(document.getElementById('cr_profiles_list') !== null, 'Profiles list container #cr_profiles_list exists');
	TestHarness.assert(document.getElementById('cr_evidence') !== null, 'Court record has evidence section #cr_evidence');
	TestHarness.assert(document.getElementById('cr_profiles') !== null, 'Court record has profiles section #cr_profiles');
	TestHarness.assert(document.getElementById('cr_item_check') !== null, 'Court record has item check section #cr_item_check');

	// Switch buttons exist
	TestHarness.assert(document.getElementById('cr_evidence_switch') !== null, '#cr_evidence_switch exists');
	TestHarness.assert(document.getElementById('cr_profiles_switch') !== null, '#cr_profiles_switch exists');

	// Clicking #cr_profiles_switch sets courtrecord class to 'profiles'
	var courtrecordEl = document.getElementById('courtrecord');
	var profilesSwitch = document.getElementById('cr_profiles_switch');
	if (courtrecordEl && profilesSwitch) {
		profilesSwitch.click();
		TestHarness.assertEqual(courtrecordEl.className, 'profiles', 'Clicking #cr_profiles_switch sets courtrecord class to profiles');
	}

	// Clicking #cr_evidence_switch sets courtrecord class to 'evidence'
	var evidenceSwitch = document.getElementById('cr_evidence_switch');
	if (courtrecordEl && evidenceSwitch) {
		evidenceSwitch.click();
		TestHarness.assertEqual(courtrecordEl.className, 'evidence', 'Clicking #cr_evidence_switch sets courtrecord class to evidence');
	}

	// Check back button removes 'cr-check' class from content
	var contentEl = document.getElementById('content');
	var checkBack = document.getElementById('cr-item-check-back');
	if (contentEl && checkBack) {
		contentEl.classList.add('cr-check');
		checkBack.click();
		TestHarness.assert(
			!contentEl.classList.contains('cr-check'),
			'#cr-item-check-back removes cr-check class from content'
		);
	}

	// Evidence display element exists
	TestHarness.assert(document.getElementById('evidence-display') !== null, '#evidence-display exists');
}
