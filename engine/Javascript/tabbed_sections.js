"use strict";
/*
Ace Attorney Online - Tabbed sections dynamic handling

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'tabbed_sections',
	dependencies : ['dom_loaded', 'nodes', 'events'],
	init : function() {
		var tab_navs = document.getElementsByClassName('section-tabs');
		for(var i = 0; i < tab_navs.length; i++)
		{
			enableTabbedNav(tab_navs[i]);
		}
	}
}));

//INDEPENDENT INSTRUCTIONS
function enableTabbedNav(tab_nav)
{
	// Fetch the list of corresponding sections
	var sections = new Array();
	var elt = tab_nav.nextSibling;
	while(elt)
	{
		if(elt.nodeType == 1
			&& elt.tagName.toUpperCase() == 'SECTION')
		{
			sections.push(elt);
		}
		elt = elt.nextSibling;
	}
	
	if(sections.length > 1)
	{
		// If more than one section, create a tab for each section.
		for(var i = 0; i < sections.length; i++)
		{
			createSectionTab(tab_nav, sections, sections[i]);
		}
	}
}

function createSectionTab(tab_nav, sections, section)
{
	var title = section.getElementsByTagName('h1')[0];
	var close_link = section.getElementsByClassName('close')[0];
	
	// Extract the title from its section
	title.parentNode.removeChild(title);
	
	// Create a dynamic link opening the section.
	var tab_link = document.createElement('a');
	addClass(tab_link, 'tab_link');
	setNodeTextContents(tab_link, title.textContent.trim());
	registerEventHandler(tab_link, 'click', function(){
		for(var i = 0; i < sections.length; i++)
		{
			removeClass(sections[i], 'open');
			removeClass(sections[i].linkedTab, 'open');
		}
		addClass(section, 'open');
		addClass(section.linkedTab, 'open');
	}, false);
	
	// Create new tab with open link.
	var tab = document.createElement('span');
	addClass(tab, 'tab');
	
	tab.linkedSection = section;
	section.linkedTab = tab;
	
	tab.appendChild(tab_link);
	
	if(close_link)
	{
		// If section can be closed, create a dynamic close link.
		close_link.parentNode.removeChild(close_link);
		tab.appendChild(close_link);
		registerEventHandler(close_link, 'click', function(e) {
			var section_index = sections.indexOf(section);
			sections.splice(section_index, 1);
			if(hasClass(section, 'open'))
			{
				if(section_index > 0)
				{
					addClass(sections[section_index - 1], 'open');
					addClass(sections[section_index - 1].linkedTab, 'open');
				}
				else if(sections.length > 0)
				{
					addClass(sections[0], 'open');
					addClass(sections[0].linkedTab, 'open');
				}
			}
			
			section.parentNode.removeChild(section);
			tab_nav.removeChild(tab);
			
			e.preventDefault();
			return false;
		}, false);
	}
	
	if(hasClass(section, 'open'))
	{
		addClass(tab, 'open');
	}
	
	// Insert new tab in tab navigation
	tab_nav.appendChild(tab);
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('tabbed_sections');
