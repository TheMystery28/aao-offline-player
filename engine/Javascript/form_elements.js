"use strict";
/*
Ace Attorney Online - Generate form elements

A form element provides the following interface methods :
	- getValue(), returning the current value
	- setValue(new_val) setting the current value
	- canBe(test_value) check whether the field can accept a given value
	- getName(), returning the name of the field
	- setName(new_name), setting the name of the field

When created, a form element must already have a valid default value, so getValue() can be called right away.

A form element provides the following events :
	- change : the value of the element has been changed, and can be read by getValue();
	- focus : the form element has taken focus
	- blur : the form element has lost focus

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'form_elements',
	dependencies : ['form_select', 'events', 'objects', 'nodes'],
	init : function() {
		registerFormElementGenerator('checkbox', createCheckBox);
		registerFormElementGenerator('boolean', createStringCheckBox);
		
		registerFormElementGenerator('float', createFloatInput.bind(undefined, false));
		registerFormElementGenerator('posfloat', createFloatInput.bind(undefined, true));
		
		registerFormElementGenerator('integer', createIntegerInput.bind(undefined, false));
		registerFormElementGenerator('natural', createIntegerInput.bind(undefined, true));
		registerFormElementGenerator('percent', createIntegerInput.bind(undefined, true, 100));
		
		registerFormElementGenerator('string', createStringInput);
		registerFormElementGenerator('word', createStringInput); // TODO : enforce only one word
		registerFormElementGenerator('uri', createUriInput);
		registerFormElementGenerator('image_uri', createImageUriInput);
		registerFormElementGenerator('sound_uri', createSoundUriInput);

		registerFormElementGenerator('colour', createColourInput);
		
		registerFormElementGenerator('expression', createExpressionInput);
		
		registerFormElementGenerator('text', createTextArea);
	}
}));

//INDEPENDENT INSTRUCTIONS
var form_element_generators = new Object();

function createCheckBox()
{
	var element = document.createElement('input');
	element.type = 'checkbox';
	
	element.canBe = function(test_value)
	{
		return test_value === true || test_value === false || test_value === 0 || test_value === 1;
	};
	
	element.getValue = function()
	{
		return this.checked;
	};
	
	element.setValueNoCheck = function(new_val)
	{
		this.checked = new_val;
	};
	
	return element;
}

// String checkbox uses strings ("true", "false") instead of real boolean values
function createStringCheckBox()
{
	var element = document.createElement('input');
	element.type = 'checkbox';
	
	element.canBe = function(test_value)
	{
		return test_value === "true" || test_value === "false";
	};
	
	element.getValue = function()
	{
		return this.checked ? "true" : "false";
	};
	
	element.setValueNoCheck = function(new_val)
	{
		this.checked = (new_val === "true");
	};
	
	return element;
}

function createFloatInput(nonnegative)
{
	var element = document.createElement('input');
	element.type = 'text';
	element.value = 0;
	
	element.canBe = function(test_value)
	{
		if(isNaN(test_value))
		{
			return false;
		}
		else if(nonnegative)
		{
			return !!test_value.toString().match(/^[\d]*\.?[\d]*$/);
		}
		else
		{
			return !!test_value.toString().match(/^-?[\d]*\.?[\d]*$/);
		}
	};

	registerEventHandler(element, 'keyup', function(e)
	{
		if(!element.canBe(element.value))
		{
			element.value = element.previousValue;
			e.preventDefault();
			return false;
		}
		else
		{
			element.previousValue = element.value;
		}
	}, false);
	
	element.getValue = function()
	{
		return this.value;
	};
	
	element.setValueNoCheck = function(new_val)
	{
		this.value = new_val;
		this.previousValue = new_val;
	};
	
	return element;
}

function createIntegerInput(natural, max)
{
	var element = document.createElement('input');
	element.type = 'text';
	element.value = 0;
	
	element.canBe = function(test_value)
	{
		if(isNaN(test_value))
		{
			return false;
		}
		if(typeof(max) !== undefined && parseInt(test_value) > max)
		{
			return false;
		}
		else if(natural)
		{
			return !!test_value.toString().match(/^[\d]*$/);
		}
		else
		{
			return !!test_value.toString().match(/^-?[\d]*$/);
		}
	};
	
	element.previousValue = '';
	
	registerEventHandler(element, 'keyup', function(e)
	{
		if(!element.canBe(element.value))
		{
			element.value = element.previousValue;
			e.preventDefault();
			return false;
		}
		else
		{
			element.previousValue = element.value;
		}
	}, false);
	
	element.getValue = function()
	{
		var intVal = parseInt(this.value);
		return isNaN(intVal) ? 0 : intVal;
	};
	
	element.setValueNoCheck = function(new_val)
	{
		this.value = new_val;
		this.previousValue = new_val;
	};
	
	return element;
}

function createStringInput()
{
	var element = document.createElement('input');
	element.type = 'text';
	element.value = '';
	
	element.canBe = is_scalar;
	
	element.getValue = function()
	{
		return this.value;
	};
	
	element.setValueNoCheck = function(new_val)
	{
		this.value = new_val;
	};
	
	return element;
}

function createTextArea()
{
	var element = document.createElement('textarea');
	element.value = '';
	
	element.canBe = is_scalar;
	
	element.getValue = function()
	{
		return this.value;
	};
	
	element.setValueNoCheck = function(new_val)
	{
		this.value = new_val;
	};
	
	return element;
}

function createExpressionInput()
{
	var element = createStringInput();
	
	function checkSyntax()
	{
		//check if the expression syntax is correct
		var expr = element.getValue();
		
		try
		{
			evaluate_expression(expr, global_env);
			removeClass(element, 'bad_value');
		}
		catch(e)
		{
			var message = e.message || e;
			toggleClass(element, 'bad_value', message.indexOf('BadExpression:') === 0);
		}
	}
	
	element.setValueNoCheck = function(new_val)
	{
		this.value = new_val;
		checkSyntax();
	};
	
	return element;
}

// Returns an object with info about the URI, or null if cannot parse correctly
function parseUri(uri)
{
	// Parse top level according to http://tools.ietf.org/html/rfc3986
	var scheme_regexp = '[a-zA-Z][a-zA-Z0-9+.-]*';
	var unreserved_chars = 'a-zA-Z0-9_~.-';
	var sub_delims_chars = '!$&\'()*+,;=';
	var pct_encoded_regexp = '%[0-9A-Fa-f]{2}';
	var pchar_regexp = '(?:[:@ ' + sub_delims_chars + unreserved_chars + ']|' + pct_encoded_regexp + ')'; // Space isn't accepted as per the spec, but UAs accept them by automatically urlencoding them
	var basic_uri_regexp = '^(' + scheme_regexp + '):(.*?)(?:\\?((?:' + pchar_regexp + ' | [?/])*))?(?:#((?:' + pchar_regexp + ' | [?/])*))?$';
	
	var basicUriReg = new RegExp(basic_uri_regexp);
	
	var parsedUri = uri.match(basicUriReg);
	if(parsedUri)
	{
		var scheme = parsedUri[1];
		var hier_part = parsedUri[2];
		var query = parsedUri[3];
		var fragment = parsedUri[4];
		
		var result = new Object({
			scheme: scheme,
			query: query,
			fragment: fragment
		});
		
		switch(scheme)
		{
			case 'http':
			case 'https':
			case 'ftp':
				// Parse URL hier_part more or less according to http://tools.ietf.org/html/rfc3986
				// (Only allow absolute URL with authority defined, starting with //)
				
				var reg_name_regexp = '(?:[' + sub_delims_chars + unreserved_chars + ']|' + pct_encoded_regexp + ')*';
				var file_url_hier_part_regexp = '^//(?:([\\w.]+:)*@)?(' + reg_name_regexp + ')?(?::(\\d*))?((?:/' + pchar_regexp + '*)*)$';
				var fileUrlReg = new RegExp(file_url_hier_part_regexp);
				
				var parsedFileUrl = hier_part.match(fileUrlReg);
				
				if(parsedFileUrl)
				{
					var userinfo = parsedFileUrl[1];
					var host = parsedFileUrl[2];
					var port = parsedFileUrl[3];
					var path = parsedFileUrl[4];
					
					var path_elements = path.split('/');
					var file_name = path_elements[path_elements.length - 1];
					var file_ext = file_name.substr(file_name.lastIndexOf('.') + 1);
					
					result.userinfo = userinfo;
					result.host = host;
					result.port = port;
					result.path = path;
					result.file_name = file_name;
					result.file_ext = file_ext;
				}
				else
				{
					return null; // Can not parse URL correctly
				}
				
				break;
			
			case 'data':
				// Parse Data URI hier_part (more or less) according to http://tools.ietf.org/html/rfc2397
				
				var data_uri_hier_part_regexp = '^((\\w+)/(\\w+))?(;\\w+=[^;]*)*(;base64)?,(.*)$';
				var dataUriReg = new RegExp(data_uri_hier_part_regexp);
				
				var parsedDataUri = hier_part.match(dataUriReg);
				if(parsedDataUri)
				{
					var mime = parsedDataUri[1];
					var mime_type = parsedDataUri[2];
					var mime_subtype = parsedDataUri[3];
					var params = parsedDataUri[4];
					var base64 = parsedDataUri[5] ? true : false;
					var data = parsedDataUri[6];
					
					result.mime = mime;
					result.params = params;
					result.base64 = base64;
					result.data = data;
				}
				else
				{
					return null; // Can not parse URI correctly
				}
				
				break;
		}
		
		return result;
	}
	else
	{
		return null;
	}
}

function createUriInput()
{
	var element = createStringInput();
	
	function checkSyntax()
	{
		//check if the URL syntax is correct
		var uri = element.getValue();
		
		toggleClass(element, 'bad_value', !parseUri(uri));
	}
	
	registerEventHandler(element, 'change', checkSyntax, false);
	
	return element;
}

function createImageUriInput()
{
	var element = createStringInput();
	
	function checkSyntax()
	{
		//check if the URL syntax is correct
		var uri = element.getValue();
		
		var parsed_uri = parseUri(uri);
		toggleClass(element, 'bad_value', !(parsed_uri && (
			(parsed_uri.file_ext && (['jpg', 'jpeg', 'png', 'gif'].indexOf(parsed_uri.file_ext) !== -1)) || 
			(parsed_uri.mime && (['image/jpeg', 'image/png', 'image/gif'].indexOf(parsed_uri.mime) !== -1))
		)));
	}
	
	registerEventHandler(element, 'change', checkSyntax, false);
	
	return element;
}

function createSoundUriInput()
{
	var element = createStringInput();
	
	function checkSyntax()
	{
		//check if the URL syntax is correct
		var uri = element.getValue();
		
		var parsed_uri = parseUri(uri);
		toggleClass(element, 'bad_value', !(parsed_uri && (
			(parsed_uri.file_ext && (['mp3'].indexOf(parsed_uri.file_ext) !== -1)) || 
			(parsed_uri.mime && (['audio/mpeg', 'audio/mp3'].indexOf(parsed_uri.mime) !== -1))
		)));
	}
	
	registerEventHandler(element, 'change', checkSyntax, false);
	
	return element;
}

function createColourInput() 
{
	var element = document.createElement('input');
	element.type = "color";
	element.defaultValue = "#000000";
	
	element.canBe = function(input) {
		// check if input is a hex code using regex
		var regex = new RegExp("^#(?:[0-9a-fA-F]{3}){1,2}$");
		return regex.test(input);
	}
	
	element.getValue = function() {
		return this.value;
	}
	
	element.setValueNoCheck = function(new_val) {
		this.value = new_val;
	}
	
	return element;
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function registerFormElementGenerator(type, generator)
{
	form_element_generators[type] = generator;
}

function populateFormElement(element, initial_value)
{
	element.setValue = function(new_val)
	{
		if(this.canBe(new_val))
		{
			this.setValueNoCheck(new_val);
			triggerEvent(element, 'change');
			return true;
		}
		else
		{
			debugger;
			return false;
		}
	};
	
	element.setName = function(new_name)
	{
		element.name = new_name;
	};
	element.getName = function()
	{
		return element.name;
	};
	
	if(typeof initial_value != 'undefined')
	{
		element.setValueNoCheck(initial_value);
	}
}

function createFormElement(type, initial_value, parameters)
{
	var element = form_element_generators[type](parameters);
	
	populateFormElement(element, initial_value);
	
	return element;
}

function createFormCustomSelect(values, initial_value)
{
	var element = createSelect(values);
	
	populateFormElement(element, initial_value);
	
	return element;
}

//Creating a label containing a form element
// If raw_string is defined, it overrides language_string
function createLabel(form_element, language_string, raw_string)
{
	var label = document.createElement('label');
	
	var label_title = document.createElement('span');
	if(raw_string)
	{
		setNodeTextContents(label_title, raw_string);
	}
	else
	{
		label_title.setAttribute('data-locale-content', language_string);
		translateNode(label_title);
	}
	
	var label_element = document.createElement('span');
	addClass(label_element, 'form_element_container');
	label_element.appendChild(form_element);
	
	//Set the IDs of the elements
	form_element.id = 'form_elt_' + (++createLabel.newId);
	label.htmlFor = 'form_elt_' + createLabel.newId;
	
	//Fix the click event on the label to work with custom form controls as well
	registerEventHandler(label, 'click', function(e){
		if(!nodeContains(form_element, e.target))
		{
			form_element.focus();
		}
	}, false);
	
	// Set class and insert order depending on element type
	switch(form_element.getAttribute('type'))
	{
		case 'checkbox':
		case 'radio':
			addClass(label, 'checkbox_label');
			label.appendChild(label_element);
			label.appendChild(label_title);
			break;
			
		default:	
			addClass(label, 'regular_label');
			label.appendChild(label_title);
			label.appendChild(label_element);
			break;
	}
	
	return label;
}
createLabel.newId = 0;

//END OF MODULE
Modules.complete('form_elements');
