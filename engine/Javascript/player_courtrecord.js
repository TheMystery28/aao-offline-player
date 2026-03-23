"use strict";
/*
Ace Attorney Online - Player court records manager

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'player_courtrecord',
	dependencies : ['engine_events', 'trial', 'trial_data', 'nodes', 'language', 'page_loaded', 'player_debug', 'expression_engine'],
	init : function(){
		if(trial_data)
		{
			// If a trial is actually loaded...
			var evidence = document.getElementById('cr_evidence_list');
			for(var i = 1; i < trial_data.evidence.length; i++)
			{
				evidence.appendChild(generateCrElement('evidence', trial_data.evidence[i].id));
			}
			
			var profiles = document.getElementById('cr_profiles_list');
			for(var i = 1; i < trial_data.profiles.length; i++)
			{
				profiles.appendChild(generateCrElement('profiles', trial_data.profiles[i].id));
			}
			
			register_custom_function('evidence_is_revealed', function(type, id){
				var real_type = (type == 'preuve' || type == 'evidence') ? 'evidence' : 'profiles';
				return !getRowById(real_type, id).hidden;
			});
		}
	}
}));

//INDEPENDENT INSTRUCTIONS
function generateCrElement(type, id)
{
	var elt = getRowById(type, id);
	
	var elt_container = document.createElement('div');
	elt_container.id = 'cr_' + type + '_' + id;
	addClass(elt_container, 'evidence_display');
	
	var summary = document.createElement('div');
	addClass(summary, 'summary');
	populateCrElementSummary(summary, type, elt);
	registerEventHandler(summary, 'click', selectCrElement.bind(undefined, type, id), false);
	elt_container.appendChild(summary);
	
	var details = document.createElement('div');
	details.id = 'screen-details-' + type + '_' + id;
	addClass(details, 'details thin-bars');
	
		var description = document.createElement('div');
		setNodeTextContents(description, elt.description);
		addClass(description, 'description');
		details.appendChild(description);
		
		var bottom_bar = document.createElement('div');
		addClass(bottom_bar, 'buttonbar-bottom');
		details.appendChild(bottom_bar);
			
			if(elt.check_button_data && elt.check_button_data.length > 0)
			{
				var check = document.createElement('a');
				addClass(check, 'bs-button bottomright');
				check.setAttribute('data-locale-content', 'check');
				check.href = 'javascript: checkCrElement("' + type + '", ' + id + ')';
				bottom_bar.appendChild(check);
			}
		
		translateNode(details);
	
	elt_container.appendChild(details);
	
	elt_container.hidden = elt.hidden;
	
	return elt_container;
}

function checkCrElement(type, id)
{
	var current_page = 0;
	function openPage(page_index)
	{
		removeClass(pagination.childNodes[current_page], 'open');
		addClass(pagination.childNodes[page_index], 'open');
		removeClass(contents.childNodes[current_page], 'open');
		addClass(contents.childNodes[page_index], 'open');
		
		current_page = page_index;
	}
	
	var elt = getRowById(type, id);
	
	if(elt.check_button_data && elt.check_button_data.length > 0)
	{
		var pagination = document.getElementById('cr-item-check-pagination');
		var contents = document.getElementById('cr-item-check-contents');
		
		emptyNode(pagination);
		emptyNode(contents);
		
		for(var i = 0; i < elt.check_button_data.length; i++)
		{
			var data = elt.check_button_data[i];
			
			var pagination_link = document.createElement('a');
			var contents_div = document.createElement('div');
			
			registerEventHandler(pagination_link, 'click', (function(i){
				openPage(i);
			}).bind(undefined, i), false);
			addClass(contents_div, data.type);
			
			switch(data.type)
			{
				case 'text':
					// Pagination
					setNodeTextContents(pagination_link, data.content.substr(0, 75));
					
					// Contents
					setNodeTextContents(contents_div, data.content);
					break;
				
				case 'image':
					// Pagination
					var thumbnail = new Image();
					thumbnail.src = data.content;
					pagination_link.appendChild(thumbnail);
					
					// Contents
					var img = new Image();
					img.src = data.content;
					contents_div.appendChild(img);
					break;
				
				case 'sound':
					// Pagination
					var img = new Image();
					img.src = 'img/sound.png';
					pagination_link.appendChild(img);
					
					// Contents
					contents_div.appendChild(createSoundPlayer(data.content, 'cr_' + type + '_' + id + '_' + i));
					break;
			}
			
			pagination.appendChild(pagination_link);
			contents.appendChild(contents_div);
		}
		
		// Open the first page
		openPage(0);
		
		// Display the panel
		addClass(document.getElementById('content'), 'cr-check');
	}
}

function selectCrElement(type, id)
{
	var content = document.getElementById('content');
	
	// Select the element only if in select mode and it matches the type lock
	if(hasClass(content, 'cr-select') &&
		(hasClass(content, 'cr-all') || hasClass(content, 'cr-' + type)))
	{
		var elt = getRowById(type, id);
		
		var container = document.getElementById('evidence-display');
		emptyNode(container);
		
		var summary = document.createElement('div');
		addClass(summary, 'summary');
		populateCrElementSummary(summary, type, elt);
		container.appendChild(summary);
		
		var description = document.createElement('div');
		setNodeTextContents(description, elt.description);
		addClass(description, 'description');
		container.appendChild(description);
		
		container.setAttribute('data-selected-elt', type + '_' + id);

		EngineEvents.emit('courtrecord:select', { type: type, id: id });
	}
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function refreshCrElements()
{
	for(var type in {'evidence': 0, 'profiles': 0})
	{
		for(var i = 1 ; i < trial_data[type].length; i++)
		{
			document.getElementById('cr_' + type + '_' + trial_data[type][i].id).hidden = trial_data[type][i].hidden;
		}
	}
}

function setCrElementHidden(type, id, hidden)
{
	var elt = getRowById(type, id);

	if(elt)
	{
		elt.hidden = hidden;
		document.getElementById('cr_' + type + '_' + id).hidden = hidden;

		EngineEvents.emit('courtrecord:visibility', { type: type, id: id, hidden: hidden });
		debugRefreshCourtRecords();
	}
}

function populateCrElementSummary(container, type, elt)
{
	emptyNode(container);
	
	// Create structure
	var img = new Image();
	container.appendChild(img);
	var name = document.createElement('span');
	addClass(name, 'name');
	container.appendChild(name);
	var metadata = document.createElement('span');
	addClass(metadata, 'civil_status');
	container.appendChild(metadata);
	
	// Populate with element data
	if(type == 'evidence')
	{
		img.src = getEvidenceIconUrl(elt);
		setNodeTextContents(name, elt.name);
		setNodeTextContents(metadata, elt.metadata);
	}
	else
	{
		img.src = getProfileIconUrl(elt);
		setNodeTextContents(name, elt.long_name);
		setNodeTextContents(metadata, elt.civil_status);
	}
}

function populateCrElementDetails(container, type, id)
{
	emptyNode(container);
	
	var elt = getRowById(type, id);
	
	var summary = document.createElement('div');
	addClass(summary, 'summary');
	populateCrElementSummary(summary, type, elt);
	
	var description = document.createElement('div');
	addClass(description, 'description');
	setNodeTextContents(description, elt.description);
	
	// Display
	container.appendChild(summary);
	container.appendChild(description);
}

// Scale evidence/profile cards proportionally to fill available width.
// Cards are 268px wide at base. Calculate zoom = containerWidth / 268.
(function() {
	var CARD_BASE_WIDTH = 273; // 268px card + 5px margin
	function updateCardZoom() {
		var lists = document.querySelectorAll('.evidence-list');
		for (var i = 0; i < lists.length; i++) {
			var w = lists[i].clientWidth;
			if (w > 0 && w !== CARD_BASE_WIDTH) {
				lists[i].style.setProperty('--card-zoom', (w / CARD_BASE_WIDTH).toFixed(3));
			}
		}
	}
	// Run on load and on resize/orientation change
	updateCardZoom();
	window.addEventListener('resize', updateCardZoom);
	// Recalculate when switching between evidence/profiles tabs,
	// because the hidden tab has clientWidth=0 during initial calculation.
	var switches = document.querySelectorAll('.courtrecord-switchview');
	for (var i = 0; i < switches.length; i++) {
		switches[i].addEventListener('click', function() {
			setTimeout(updateCardZoom, 50); // wait for display:block to apply
		});
	}
})();

//END OF MODULE
Modules.complete('player_courtrecord');
