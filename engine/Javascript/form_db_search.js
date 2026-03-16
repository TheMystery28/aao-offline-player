"use strict";
/*
Ace Attorney Online - Database search field
TODO : integrate in AAO form elements framework.
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'form_db_search',
	dependencies : ['events', 'nodes'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS
function createDbSearchField(search_key, field_name, parameters)
{
	var element = document.createElement('span');
	addClass(element, 'db-search-field');
	
	// TODO : move XHR to a specific module.
	element.xhr = new XMLHttpRequest();
	
	element.search_box = document.createElement('input');
	element.search_box.type = 'text';
	element.appendChild(element.search_box);
	
	element.result_box = document.createElement('ul');
	element.result_box.answers = [];
	element.result_box.selected_index = -1;
	element.appendChild(element.result_box);
	
	element.value_input = document.createElement('input');
	element.value_input.type = 'text';
	element.value_input.name = field_name;
	element.value_input.style.display = 'none';
	element.appendChild(element.value_input);
	
	element.result_box.clear = function() {
		emptyNode(this);
		this.answers = [];
		this.selected_index = -1;
	};
	
	element.result_box.refreshSelectedValue = function(updateSearchBox) {
		var selected_answer = this.answers[this.selected_index];
		
		if(updateSearchBox)
		{
			element.search_box.value = selected_answer.label;
		}
		element.value_input.value = selected_answer.value;
		
		for(var i = 0; i < this.childNodes.length; i++)
		{
			removeClass(this.childNodes[i], 'kbd-selected');
		}
		
		addClass(this.childNodes[this.selected_index], 'kbd-selected');
	};
	
	registerEventHandler(element.search_box, 'keyup', handleKeyPressOnDbSearchField.bind(undefined, element, search_key, parameters), false);
	
	registerEventHandler(element.search_box, 'blur', element.result_box.clear.bind(element.result_box), false);
	
	return element;
}

function handleKeyPressOnDbSearchField(field, search_key, parameters, event)
{
	var search_box = field.search_box;
	var result_box = field.result_box;
	var value_input = field.value_input;
	
	switch(event.keyCode)
	{
		case 13:
			// Enter key : accept search result
			result_box.clear();
			
			if(result_box.answers.length > 0)
			{
				result_box.refreshSelectedValue(true);
			}
			break;
		
		case 40:
			//Down arrow: increase selected index
			if(result_box.answers.length > 0 && result_box.selected_index < result_box.answers.length - 1)
			{
				result_box.selected_index++;
				
				result_box.refreshSelectedValue(true);
			}
			break;
		
		case 38:
			//Up arrow: decrease selected index
			if(result_box.answers.length > 0 && result_box.selected_index > 0)
			{
				result_box.selected_index--;
				
				result_box.refreshSelectedValue(true);
			}
			
			break;
		
		default:
			searchAndDisplay(field, search_key, parameters);
			break;
	}
	
	event.preventDefault();
	event.stopPropagation();
}

function searchAndDisplay(field, search_key, parameters)
{
	var search_box = field.search_box;
	var result_box = field.result_box;
	var value_input = field.value_input;
	
	var parameters_text = '';
	if(parameters)
	{
		for(var name in parameters)
		{
			parameters_text += '&parameters[' + name + ']=' + parameters[name];
		}
	}
	
	var search_text = search_box.value;
	
	result_box.clear();
	
	if(search_text.length > 0)
	{
		// Cancel any ongoing search
		field.xhr.abort();
		
		field.xhr.onreadystatechange = function() {
			if(field.xhr.readyState == 4 && field.xhr.status == 200)
			{
				result_box.answers = JSON.parse(field.xhr.responseText);
				
				if(result_box.answers.length > 0)
				{
					for(var i = 0; i < result_box.answers.length; i++)
					{
						var value = result_box.answers[i].value;
						var label = result_box.answers[i].label;
						
						var result_item = document.createElement('li');
						result_item.value = value;
						setNodeTextContents(result_item, label);
						result_box.appendChild(result_item);
						
						registerEventHandler(result_item, 'mousedown', (function(index) {
							result_box.selected_index = index;
							result_box.refreshSelectedValue(true);
						}).bind(undefined, i), false);
					}
					
					result_box.selected_index = 0;
					result_box.refreshSelectedValue(false);
				}
				else
				{
					value_input.value = '';
				}
			}
		};
		field.xhr.open('GET', 'db_search.php?search_key=' + search_key + parameters_text + '&search_text=' + encodeURIComponent(search_text), true);
		field.xhr.send(null);
	}
	else
	{
		value_input.value = '';
	}
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('form_db_search');
