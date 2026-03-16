/*
Ace Attorney Online - Trial data module (offline version)
Loads trial_data.json and trial_info.json from the case directory via synchronous XHR.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'trial',
	dependencies : ['trial_object_model', 'objects_diff'],
	init : function() {
		// initial_trial_data is extended according to model, and then set to readonly mode.
		if(!initial_trial_data) return;

		extendObjectWithModel(initial_trial_data, trial_object_model);
		Object.freeze(initial_trial_data);

		if('trialdata_diff' in _GET)
		{
			// Handle trial data diff if given : apply it onto the trial data.
			trial_data = patch(initial_trial_data, JSON.parse(_GET['trialdata_diff']));
		}
		else
		{
			// Else, trial_data is defined as a clone of initial_trial_data to keep original as reference.
			trial_data = objClone(initial_trial_data);
		}
	}
}));


//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES

// Load trial_info.json and trial_data.json from the case directory.
// The case directory is served at /case/{trial_id}/ by the localhost server.
var trial_information;
var initial_trial_data;

(function() {
	var trial_id = _GET['trial_id'];
	if(!trial_id) {
		// No trial_id in URL — leave variables undefined
		return;
	}

	var case_base = 'case/' + trial_id + '/';

	// Synchronous XHR to load JSON — matches the synchronous behavior of the PHP version.
	function loadJSON(url) {
		var xhr = new XMLHttpRequest();
		xhr.open('GET', url, false); // synchronous
		xhr.send();
		if(xhr.status === 200) {
			return JSON.parse(xhr.responseText);
		}
		return null;
	}

	var info = loadJSON(case_base + 'trial_info.json');
	if(!info) {
		// Part not available — show user-friendly error instead of blank screen
		document.body.innerHTML = '<div style="padding:2em;text-align:center;color:#e0e0e0;font-family:sans-serif">' +
			'<h2>Part not available</h2>' +
			'<p>Case #' + trial_id + ' is not downloaded.</p>' +
			'<p>Return to the library to download all parts of this sequence.</p></div>';
		return;
	}
	trial_information = info;

	var data = loadJSON(case_base + 'trial_data.json');
	if(data) {
		initial_trial_data = data;
	}
})();

// trial_data variable is null at first, to avoid any modification until properly initialised.
var trial_data = null;


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('trial');
