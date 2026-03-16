"use strict";
/*
Ace Attorney Online - Language management module

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'language',
	dependencies : ['nodes'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS



//EXPORTED VARIABLES
var lang = new Object();

var Languages = {
	request_list : new Object(), //request status of language files : undefined; 1 (being loaded); 2 (loaded)
	trigger_list : new Object(), //callback functions for language completion
	languages : ['en'], // Languages from which to load all strings (first is main; following used as fallback)
	
	setMainLanguage : function(language)
	{
		this.languages.unshift(language);
	},
	
	request : function(name, callback)
	{
		if(this.request_list[name] == 2) //language already loaded : run callback
		{
			callback();
		}
		else
		{
			if(!this.request_list[name]) //new language : mark it and start loading
			{
				this.request_list[name] = 1;
				this.trigger_list[name] = new Array();
				
				includeLanguage(name, this.languages);
			}
			this.trigger_list[name].push(callback); //insert new callback
		}
	},
	
	complete : function(name)
	{
		this.request_list[name] = 2; //mark as loaded
		
		//run all callbacks
		for(var i = 0; i < this.trigger_list[name].length; i++)
		{
			this.trigger_list[name][i]();
		}
	},
	
	requestFiles : function(names, callback)
	{
		// Clone the given names array to avoid any possible interference.
		var to_load = names.slice(0);
		
		for(var i = 0; i < to_load.length; i++)
		{
			this.request(to_load[i], (function(name){
				var name_index = to_load.indexOf(name);
				if(name_index >= 0)
				{
					to_load.splice(name_index, 1);
				}
				
				if(to_load.length == 0)
				{
					callback();
				}
			}).bind(undefined, to_load[i]));
		}
	} 
};

//EXPORTED FUNCTIONS
function includeLanguage(name, language_list)
{
	var file_version = getFileVersion(['lang', language_list[0], name]);
	
	if(file_version != null)
	{
		// If file exists, load it through ajax
		var xhr = new XMLHttpRequest();
		xhr.onreadystatechange = function(){
			if(xhr.readyState == 4)
			{
				if(xhr.status == 200)
				{
					// When file is loaded, merge it with current language data
					var new_lang = JSON.parse(xhr.responseText);
					for(var key in new_lang)
					{
						lang[key] = new_lang[key];
					}
					
					// And mark it as loaded
					Languages.complete(name);
				}
				else
				{
					if(language_list.length > 1)
					{
						// If not loaded but other languages available : fall back
						includeLanguage(name, language_list.slice(1));
					}
					else
					{
						throw "Language file not found : " + name;
					}
				}
			}
		};
		
		xhr.open('GET', cfg['lang_dir'] + language_list[0] + '/' + name + '.js?v=' + file_version, true);
		xhr.send(null);
	}
	else
	{
		// Language file cannot be found in current language
		
		if(language_list.length > 1)
		{
			// But other languages available : fall back
			includeLanguage(name, language_list.slice(1));
		}
		else
		{
			throw "Language file not found : " + name;
		}
	}
}

// Load a raw language string
function l(str)
{
	if(lang[str])
	{
		return lang[str];
	}
	else
	{
		//console.warn('Missing language line : ' + str);
		return str;
	}
}

// Load a language string and perform locale variable replacement
function lr(str, localevars)
{
	return l(str).replace(/<(\w+)>/g, function(match, name)
	{
		return localevars[name] || name;
	});
}

// Translate a node and all of its children
function translateNode(node)
{
	function getTranslation(node, string_name)
	{
		return l(string_name).replace(/<(\w+)>/g, function(match, name)
		{
			return node.getAttribute('data-localevar-'+name) || name;
		});
	}
	
	var string_name;
	
	if(string_name = node.getAttribute('data-locale-content'))
	{
		//Translate the content of the element
		setNodeTextContents(node, getTranslation(node, string_name));
	}
	
	if(string_name = node.getAttribute('data-locale-value'))
	{
		//Translate value of the element
		node.value = getTranslation(node, string_name);
	}
	
	if(string_name = node.getAttribute('data-locale-placeholder'))
	{
		//Translate placeholder of the element
		node.placeholder = getTranslation(node, string_name);
	}
	
	if(string_name = node.getAttribute('data-locale-before'))
	{
		//Translate content to be included before in CSS
		node.setAttribute('data-include-before', getTranslation(node, string_name));
	}
	
	if(node.childNodes.length > 0)
	{
		for(var i = 0; i < node.childNodes.length; i++)
		{
			if(node.childNodes[i].nodeType == 1)
			{
				translateNode(node.childNodes[i]);
			}
		}
	}
}

//END OF MODULE
Modules.complete('language');
