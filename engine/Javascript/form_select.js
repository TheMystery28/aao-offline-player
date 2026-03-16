"use strict";
/*
Ace Attorney Online - Generate elements to select values from the trial data

Select elements provide the following methods :
	- getValue(), returning the current value
	- setValueNoCheck(new_val) setting the current value
	- canBe(test_value) checking whether a value is part of the select
	- getTitle() returning the title of the current entry

A setValue method is defined by the form elements API using canBe and setValueNoCheck.

Values in a select can be of any type (including non-scalar) since they are serialized as JSON strings

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'form_select',
	dependencies : ['trial', 'trial_data', 'default_data', 'language', 'nodes', 'events', 'sound-howler', 'objects'],
	init : function() {
		registerFormElementGenerator('select', createSelect);
		registerFormElementGenerator('radio', createRadio);
	}
}));

//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES
var SELECT_OPTION = 0;
var SELECT_OPTGROUP = 1;
var SELECT_INPUT = 2;

//EXPORTED FUNCTIONS

//Functions to manage a Select list
function createSelect(data_array, selected_value)
{
	//Shallow copy data_array, so the original data_array is unaffected by the splices
	data_array = data_array.slice();
	
	//Remove headers for empty categories and elements set as __HIDDEN__ in the language file.
	var last_header_index = data_array.length;
	for(var i = data_array.length - 1; i >=0 ; i--)
	{
		if(data_array[i].type == SELECT_OPTGROUP)
		{
			if(last_header_index == i + 1)
			{
				data_array.splice(i, 1);
			}
			last_header_index = i;
		}
		else if(data_array[i].lang && l(data_array[i].lang) == '__HIDDEN__')
		{
			data_array.splice(i, 1);
		}
	}
	//If data array is empty, display one choice for null
	if(data_array.length == 0)
	{
		data_array.unshift(new Object({
			type: SELECT_OPTION,
			lang: 'none',
			value: null
		}));
	}
	
	// Determine what kind of select is needed depending on the data array, and build the select.
	var containsPreview = false;
	var containsInput = false;
	for(var i = 0; (!containsPreview || !containsInput) && i < data_array.length; i++)
	{
		if(data_array[i].type == SELECT_INPUT)
		{
			containsInput = true;
		}
		if(data_array[i].type == SELECT_OPTION && data_array[i].previewGen)
		{
			containsPreview = true;
		}
	}
	
	var fields_area = null;
	if(containsInput)
	{
		fields_area = document.createElement('span');
	}
	
	var select;
	if(containsPreview)
	{
		select = createComplexSelect(data_array, selected_value, fields_area);
	}
	else
	{
		select = createSimpleSelect(data_array, selected_value, fields_area);
	}
	
	if(!fields_area)
	{
		// If no embedded input fields, the select is enough.
		return select;
	}
	else
	{
		// Else, build a wrapper element for the select.
		var element = document.createElement('span');
		addClass(element, 'select-wrapper');
		element.appendChild(select);
		element.appendChild(fields_area);
		
		element.getValue = select.getValue.bind(select);
		element.setValueNoCheck = select.setValueNoCheck.bind(select);
		element.canBe = select.canBe.bind(select);
		
		registerEventHandler(select, 'change', function(e) {
			e.stopPropagation();
			e.preventDefault();
			triggerEvent(element, 'change');
		}, false);
		
		return element;
	}
}

function createSimpleSelect(data_array, selected_value, fields_area)
{
	var select = document.createElement('select');
	select.input_fields = new Object();
	select.displayed_input_field = null;
	
	var current_optgroup = select;
	for(var i = 0; i < data_array.length; i++)
	{
		var name = ('name' in data_array[i]) ? data_array[i].name : l(data_array[i].lang);
		
		switch(data_array[i].type)
		{
			case SELECT_OPTGROUP:
				var optgroup = document.createElement('optgroup');
				optgroup.label = name;
				select.appendChild(optgroup);
				current_optgroup = optgroup;
				break;
			
			case SELECT_OPTION:
				var option = document.createElement('option');
				option.value = JSON.stringify(data_array[i].value);
				setNodeTextContents(option, name);
				if(objCompare(data_array[i].value, selected_value))
				{
					option.selected = true;
				}
				current_optgroup.appendChild(option);
				break;
			
			case SELECT_INPUT:
				var option = document.createElement('option');
				option.value = 'INPUT/' + data_array[i].input_type + '/' + JSON.stringify(data_array[i].input_parameters);
				setNodeTextContents(option, name);
				
				var input = createFormElement(data_array[i].input_type, undefined, data_array[i].input_parameters);
				if(select.selectedIndex < 0 && input.canBe(selected_value))
				{
					option.selected = true;
				}
				
				select.input_fields[option.value] = input;
				
				current_optgroup.appendChild(option);
				break;
		}
	}
	
	select.getValue = function() {
		if(this.value.match(/^INPUT/))
		{
			return this.input_fields[this.value].getValue();
		}
		else
		{
			return JSON.parse(this.value);
		}
	};
	
	select.setValueNoCheck = function(new_value) {
		var last_input_index = -1;
		
		for(var i = 0; i < this.options.length; i++)
		{
			if(this.options[i].value.match(/^INPUT/))
			{
				last_input_index = i;
				var input = this.input_fields[this.options[i].value];
				if(input.canBe(new_value))
				{
					this.options[i].selected = true;
					input.setValue(new_value);
					return;
				}
			}
			else
			{
				var option_val = JSON.parse(this.options[i].value);
				if(objCompare(option_val, new_value))
				{
					this.options[i].selected = true;
					return;
				}
			}
		}
		
		if(last_input_index >= 0)
		{
			// If nothing matched, but there are inputs, set it on the last input.
			this.input_fields[this.options[last_input_index].value].setValue(new_value);
			this.options[last_input_index].selected = true;
		}
	};
	
	select.canBe = function(test_value)
	{
		var real_value = this.getValue();
		this.setValueNoCheck(test_value);
		var ok = objCompare(this.getValue(), test_value); 
		this.setValueNoCheck(real_value);
		return ok;
	};
	
	if(Object.keys(select.input_fields).length > 0)
	{
		// If there are input fields, display them as appropriate.
		registerEventHandler(select, 'change', function(){
			if(this.displayed_input_field)
			{
				fields_area.removeChild(this.displayed_input_field);
				this.displayed_input_field = null;
			}
			
			if(this.value.match(/^INPUT/))
			{
				this.displayed_input_field = this.input_fields[this.value];
				fields_area.appendChild(this.displayed_input_field);
			}
		}, false);
		
		if(select.value.match(/^INPUT/))
		{
			select.displayed_input_field = select.input_fields[select.value];
			fields_area.appendChild(select.displayed_input_field);
		}
		
		for(var field_id in select.input_fields)
		{
			select.input_fields[field_id].field_id = field_id;
			registerEventHandler(select.input_fields[field_id], 'change', function(e) {
				e.stopPropagation();
				e.preventDefault();
				if(this.field_id == select.value)
				{
					triggerEvent(select, 'change');
				}
			}, false);
		}
	}
	
	return select;
}

function createComplexSelect(data_array, selected_value, fields_area)
{
	var element = document.createElement('div');
	addClass(element, 'select');
	element.setAttribute('tabindex', 0);
	
	var select_list = document.createElement('div');
	addClass(select_list, 'select-list');
	element.appendChild(select_list);
	element.list = select_list;
	select_list.setAttribute('tabindex', -1); //Focusable but not reachable via tab
	
	registerEventHandler(element, 'keydown', function(e){
		switch(e.keyCode)
		{
			case 40:
				//Down arrow: increase selected index
				if(element.selectedIndex < element.options.length - 1)
				{
					element.setSelectedIndex(element.selectedIndex + 1);
				}
				e.preventDefault();
				e.stopPropagation();
				break;
			
			case 38:
				//Up arrow: decrease selected index
				if(element.selectedIndex > 0)
				{
					element.setSelectedIndex(element.selectedIndex - 1);
				}
				e.preventDefault();
				e.stopPropagation();
				break;
			
			case 32:
				//Space: focus select list
				select_list.focus();
				e.preventDefault();
				e.stopPropagation();
				break;
		}
	}, false);
	
	registerEventHandler(select_list, 'focus', function(){
		//Set the keyboard navigation index
		element.setKbdSelectedIndex(element.selectedIndex);
		
		//Set select list position on screen to a more convenient one than the default
		setPanelFixedPosition(select_list, element);
	}, false);
	
	registerEventHandler(select_list, 'blur', function(){
		//Reset display to the normal flow
		restorePanelPosition(select_list);
	}, false);
	
	//Keyboard navigation on select list
	registerEventHandler(select_list, 'keydown', function(e){
		switch(e.keyCode)
		{
			case 40:
				//Down arrow: increase selected index
				if(element.kbdSelectedIndex < element.options.length - 1)
				{
					element.setKbdSelectedIndex(element.kbdSelectedIndex + 1);
				}
				e.preventDefault();
				e.stopPropagation();
				break;
			
			case 38:
				//Up arrow: decrease selected index
				if(element.kbdSelectedIndex > 0)
				{
					element.setKbdSelectedIndex(element.kbdSelectedIndex - 1);
				}
				e.preventDefault();
				e.stopPropagation();
				break;
			
			case 13:
				//Enter: change value
				element.setSelectedIndex(element.kbdSelectedIndex);
				e.preventDefault();
				e.stopPropagation();
				break;
		}
	}, true);
	
	element.setSelectedIndex = function(index)
	{
		if(element.selectedIndex != index)
		{
			//Change only if didn't choose the already selected entry.
			var previous_opt = element.options[element.selectedIndex];
			removeClass(previous_opt, 'selected');
			if(previous_opt.input)
			{
				previous_opt.input.style.display = 'none';
			}
			
			var new_opt = element.options[index];
			addClass(new_opt, 'selected');
			element.selectedIndex = index;
			if(new_opt.input)
			{
				new_opt.input.style.display = '';
			}
			
			triggerEvent(element, 'change');
		}
		element.focus(); //Blur the select list; focus the select
	};
	
	element.setKbdSelectedIndex = function(index)
	{
		var previous_opt = element.options[element.kbdSelectedIndex];
		var new_opt = element.options[index];
		
		if(previous_opt)
		{
			//Close category
			if(previous_opt.parentNode.previousSibling)
			{
				addClass(previous_opt.parentNode.previousSibling, 'closed');
			}
			//Change entry class
			removeClass(previous_opt, 'kbd-selected');
		}
		
		if(new_opt.parentNode.previousSibling)
		{
			removeClass(new_opt.parentNode.previousSibling, 'closed');
		}
		addClass(new_opt, 'kbd-selected');
		select_list.scrollTop = new_opt.offsetTop; //Make sure the element is visible
		element.kbdSelectedIndex = index;
	};
	
	element.getValue = function()
	{
		return JSON.parse(this.options[this.selectedIndex].getAttribute('value'));
	};
	
	element.setValueNoCheck = function(new_val)
	{
		var last_input_index = -1;
		
		for(var i = 0; i < this.options.length; i++)
		{
			if(this.options[i].input)
			{
				last_input_index = i;
				
				if(this.options[i].input.canBe(new_val))
				{
					this.options[i].input.setValue(new_val);
					this.options[i].setAttribute('value', JSON.stringify(new_val));
					this.setSelectedIndex(i);
					return;
				}
			}
			else
			{
				var option_val = JSON.parse(this.options[i].getAttribute('value'));
				if(objCompare(new_val, option_val, false))
				{
					this.setSelectedIndex(i);
					return;
				}
			}
		}
		
		if(last_input_index >= 0)
		{
			// If nothing matched, but there are inputs, set it on the last input.
			this.options[last_input_index].input.setValue(new_val);
			this.options[last_input_index].setAttribute('value', JSON.stringify(new_val));
			this.setSelectedIndex(last_input_index);
		}
	};
	
	//Return true if the given test value is acceptable for the form element
	element.canBe = function(test_value)
	{
		var real_value = element.getValue();
		element.setValueNoCheck(test_value);
		var ok = objCompare(element.getValue(), test_value, false);
		element.setValueNoCheck(real_value);
		return ok;
	};
	
	element.getTitle = function()
	{
		return this.options[this.selectedIndex].name;
	};
	
	fillComplexSelect(element, data_array, selected_value, fields_area);
	
	return element;
}

function fillComplexSelect(select, data_array, selected_value, fields_area)
{
	var select_list = select.list;
	emptyNode(select_list);
	
	var selected_val = selected_value ? JSON.stringify(selected_value) : null;
	
	var header = null;
	var container = document.createElement('ul');
	select_list.appendChild(container);
	select.options = new Array();
	select.selectedIndex = -1;
	
	var option_index = 0;
	for(var i = 0; i < data_array.length; i++)
	{
		var name = ('name' in data_array[i]) ? data_array[i].name : l(data_array[i].lang);
		
		switch(data_array[i].type)
		{
			case SELECT_OPTION :
				var option_value = JSON.stringify(data_array[i].value);
				var selected = objCompare(selected_value, data_array[i].value);
				var new_option = document.createElement('li');
				
				var new_option_preview = document.createElement('span');
				addClass(new_option_preview, 'select-preview');
				new_option.appendChild(new_option_preview);
				
				new_option.name = name;
				var new_option_name = document.createElement('span');
				addClass(new_option_name, 'select-name');
				setNodeTextContents(new_option_name, name);
				new_option.appendChild(new_option_name);
				
				new_option.setAttribute('value', option_value);
				new_option.index = option_index;
				if(selected)
				{
					select.selectedIndex = option_index;
					if(header)
					{
						removeClass(header, 'closed'); //Selected element in there : open the section
					}
				}
				
				new_option.list_opened = false; //Was the list opened during the last mousedown event ?
				registerEventHandler(new_option, 'mousedown', function(e) {
					if(select_list != document.activeElement)
					{
						new_option.list_opened = false;
					}
					else
					{
						new_option.list_opened = true;
					}
				}, false);
				
				registerEventHandler(new_option, 'click', function(e) {
					if(select_list == document.activeElement && new_option.list_opened)
					{
						//If list is focused and really opened click selects an option
						select.setSelectedIndex(this.index);
					}
				}, false);
				
				container.appendChild(new_option);
				select.options.push(new_option);
				
				option_index++;
				
				//New option is created - fill preview if needed
				if(data_array[i].previewGen)
				{
					data_array[i].previewGen(new_option_preview);
				}
				
				break;
			
			case SELECT_OPTGROUP :
				var new_header = document.createElement('h6');
				setNodeTextContents(new_header, name);
				addClass(new_header, 'closed'); //Mark this header's section as closed until the selected element is found in there
				registerEventHandler(new_header, 'click', function() {
					toggleClass(this, 'closed', !hasClass(this, 'closed'), false);
				}, false);
				select_list.appendChild(new_header);
				header = new_header;
				
				var new_optgroup = document.createElement('ul');
				select_list.appendChild(new_optgroup);
				container = new_optgroup;
				break;
			
			case SELECT_INPUT :
				
				var new_option = document.createElement('li');
				
				var new_option_input = document.createElement('span');
				addClass(new_option_input, 'select-input');
				var input = createFormElement(data_array[i].input_type, undefined, data_array[i].input_parameters);
				registerEventHandler(input, 'change', function(){
					new_option.setAttribute('value', JSON.stringify(input.getValue()));
					triggerEvent(select, 'change');
				}, false);
				input.style.display = 'none';
				fields_area.appendChild(input);
				
				new_option.appendChild(new_option_input);
				
				new_option.input = input;
				
				new_option.name = name;
				var new_option_name = document.createElement('span');
				addClass(new_option_name, 'select-name');
				setNodeTextContents(new_option_name, name);
				new_option.appendChild(new_option_name);
				
				new_option.index = option_index;
				if(select.selectedIndex < 0 && input.canBe(selected_value))
				{
					//If selected value hasn't matched yet and can match this input, do it.
					select.selectedIndex = option_index;
					input.setValue(selected_value);
					new_option.setAttribute('value', JSON.stringify(selected_value));
					if(header)
					{
						removeClass(header, 'closed'); //Selected element in there : open the section
					}
				}
				
				if(!input.canBe(selected_value))
				{
					//If input can't take given value, use its default value
					new_option.setAttribute('value', JSON.stringify(input.getValue()));
				}
				
				new_option.list_opened = false; //Was the list opened during the last mousedown event ?
				registerEventHandler(new_option, 'mousedown', function(e) {
					if(select_list != document.activeElement)
					{
						new_option.list_opened = false;
					}
					else
					{
						new_option.list_opened = true;
					}
				}, false);
				
				registerEventHandler(new_option, 'click', function(e) {
					if(select_list == document.activeElement && new_option.list_opened)
					{
						//If list is focused and really opened click selects an option
						select.setSelectedIndex(this.index);
					}
				}, false);
				
				container.appendChild(new_option);
				select.options.push(new_option);
				
				option_index++;
				
				break;
		}
	}
	
	if(select.selectedIndex < 0) {select.selectedIndex = 0;}
	addClass(select.options[select.selectedIndex], 'selected');
	if(select.options[select.selectedIndex].input)
	{
		select.options[select.selectedIndex].input.style.display = '';
	}
	
	return select;
}

//Functions to manage a radio button list
function createRadio()
{
	var element = document.createElement('form');
	element.name = 'radio_form_' + new Date().getTime() + '_' + Math.random();
	addClass(element, 'radio_form');
	
	element.getValue = function()
	{
		var elt_list = element.elements['radio_field'];
		var elt_list_length = elt_list.length;
		for(var i = 0; i < elt_list_length; i++)
		{
			if(elt_list[i].checked)
			{
				return JSON.parse(elt_list[i].value);
			}
		}
	};
	
	element.setValueNoCheck = function(new_val)
	{
		var new_value = JSON.stringify(new_val);
		
		var elt_list = element.elements['radio_field'];
		var elt_list_length = elt_list.length;
		
		for(var i = 0; i < elt_list_length; i++)
		{
			if(elt_list[i].value == new_value)
			{
				elt_list[i].checked = true;
			}
			else
			{
				elt_list[i].checked = false;
			}
		}
	};
	
	//Return true if the given test value is acceptable for the form element
	element.canBe = function(test_value)
	{
		var real_value = element.getValue();
		element.setValueNoCheck(test_value);
		var ok = objCompare(element.getValue(), test_value, true);
		element.setValueNoCheck(real_value);
		return ok;
	};
	
	element.fill = fillRadio.bind(element, element);
	
	return element;
}

function fillRadio(radio_field, data_array, selected_value)
{
	for(var i = 0; i < data_array.length; i++)
	{
		var name = data_array[i].name ? data_array[i].name : l(data_array[i].lang);
		
		switch(data_array[i].type)
		{
			case SELECT_OPTION :
				var option_value = JSON.stringify(data_array[i].value);
				var selected = objCompare(selected_value, data_array[i].value);
				
				var option_button = document.createElement('input');
				option_button.type = 'radio';
				option_button.name = 'radio_field';
				option_button.value = option_value;
				
				if(selected)
				{
					option_button.checked = true;
				}
				else
				{
					option_button.checked = false;
				}
				
				var new_option = createLabel(option_button, name);
					
				radio_field.appendChild(new_option);
				break;
			
			case SELECT_OPTGROUP :
				var new_optgroup = document.createElement('h6');
				setNodeTextContents(new_optgroup, name);
				radio_field.appendChild(new_optgroup);
				break;
		}
	}
	
	return radio_field;
}

// Sorting options list by translated name, and including language prefixes
function languageSortAndClassifyOptionList(option_list)
{
	function setCategoryAndName(option)
	{
		if(option.category) // If option has category, the function was already run once
		{
			return;
		}
		else
		{
			// No category set, need to check the option title
			
			if(option.lang)
			{
				var lang_string = l(option.lang);
				var delimiter_index = lang_string.indexOf('|');
				if(delimiter_index == -1)
				{
					option.category = '';
					option.name = lang_string;
				}
				else
				{
					option.category = lang_string.substr(0, delimiter_index);
					option.name = lang_string.substr(delimiter_index + 1);
				}
			}
			else
			{
				option.category = '';
			}
		}
	}
	
	// Sort the array by category and name
	option_list.sort(function(a, b){
		setCategoryAndName(a);
		setCategoryAndName(b);
		
		if(a.category < b.category)
		{
			return -1;
		}
		else if(a.category > b.category)
		{
			return 1;
		}
		else
		{
			if(a.name < b.name)
			{
				return -1;
			}
			else if(a.name > b.name)
			{
				return 1;
			}
			else
			{
				return 0;
			}
		}
	});
	
	// Go through the options list and add a category title before each category
	var previous_category = null;
	for(var i = option_list.length - 1; i >= 0; i--)
	{
		if(option_list[i].category != previous_category)
		{
			if(previous_category !== null)
			{
				option_list.splice(i + 1, 0, new Object({
					type: SELECT_OPTGROUP,
					name: previous_category
				}));
			}
			previous_category = option_list[i].category;
		}
	}
	
	if(option_list.length > 0 && option_list[0].category)
	{
		option_list.splice(0, 0, new Object({
			type: SELECT_OPTGROUP,
			name: option_list[0].category
		}));
	}
}

//END OF MODULE
Modules.complete('form_select');
