function includeScript(name, generated, param, callback)
{
	// Build the URL relative to the site root. It will differ between a generated file and a static file.
	var directory;
	var extension;
	
	if(generated)
	{
		directory = './';
		extension = 'js.php';
	}
	else
	{
		directory = cfg['js_dir'];
		extension = 'js';
	}
	
	var file_version = getFileVersion([extension, directory + name]);
	
	var scriptUrl = directory + name + '.' + extension + '?v=' + file_version;
	
	// If any parameter is given, append it to the URL. (it should only be the case for generated files)
	if(param)
	{
		scriptUrl += '&' + param;
	}
	
	// URL will be resolved relative to JS root scope : either window URL or worker main file URL.
	// All workers should root on bridge.js.php so it will be the same scope.
	if(typeof importScripts == 'function')
	{
		// In worker : use importScripts function and callback immediately.
		importScripts(scriptUrl);
		if(callback)
		{
			callback();
		}
	}
	else
	{
		// In window : use DOM inclusion and callback on load.
		var load_script = document.createElement('script');
		load_script.type = 'text/javascript';
		if(callback)
		{
			load_script.onload = callback;
		}
		load_script.src = scriptUrl;
		
		document.getElementsByTagName("head")[0].appendChild(load_script);
	}
}

var Modules = {
	request_list : new Object(), //request status of modules : undefined; 1 (being loaded); 2 (waiting); 3 (operational)
	module_list : new Object(), //description of modules
	depending_on : new Object(), //modules waiting for a given one
	callbacks : new Object(), //callbacks waiting for a given module
	
	//Checks if a module is loaded, and if not launches a request
	request : function(name, callback)
	{
		if(this.request_list[name] && this.request_list[name] == 3) 
		{
			//if module is already loaded
			
			//run the callback immediately
			if(callback)
			{
				callback();
			}
			
			return true;
		}
		else if(this.request_list[name] && (this.request_list[name] == 1 || this.request_list[name] == 2))
		{
			//module is being loaded
			return false;
		}
		else
		{
			//if a new module, mark it as being loaded and include the script
			this.request_list[name] = 1;
			
			if(callback)
			{
				// If callback given, add it to the list of waiting callbacks.
				if(!this.callbacks[name])
				{
					this.callbacks[name] = new Array();
				}
				this.callbacks[name].push(callback);
			}
			
			// Offline mode: all modules (including trial and default_data) are
			// static JS files in the Javascript/ directory.
			includeScript(name);
			
			return false;
		}
	},
	
	//Starts loading a module
	load : function(module_desc)
	{
		//load the module's description
		this.module_list[module_desc.name] = module_desc;
	},
	
	//Ends the loading of the module
	complete : function(name)
	{
		if(this.check_dependencies(name))
		{
			//If all dependencies are ready, initialise the module
			this.module_list[name].init();
			
			this.request_list[name] = 3; //mark module as operational
			
			// Run the registered callbacks
			if(this.callbacks[name])
			{
				for(var i = 0; i < this.callbacks[name].length; i++)
				{
					this.callbacks[name][i]();
				}
			}
			
			// Wake up the modules wich depend on this one
			this.wake_up_depending_on(name);
		}
		else
		{
			this.request_list[name] = 2; //mark module as waiting
		}
	},
	
	
	//check if all dependencies are operational
	check_dependencies : function(name)
	{
		var deps_satisfied = true;
		for(var i = 0; i < this.module_list[name].dependencies.length; i++)
		{
			var dep_name = this.module_list[name].dependencies[i];
			
			deps_satisfied = deps_satisfied && this.request(dep_name);
			
			if(!this.depending_on[dep_name])
			{
				this.depending_on[dep_name] = new Array();
			}
			
			if(this.depending_on[dep_name].indexOf(name) == -1)
			{
				this.depending_on[dep_name].push(name);
			}
		}
		
		return deps_satisfied;
	},
	
	wake_up_depending_on : function(name)
	{
		if(this.depending_on[name])
		{
			var targets = this.depending_on[name];
			
			//loop through all packages depending on this one
			for(var i = 0; i < targets.length; i++)
			{
				//if the package is waiting, wake it up
				if(this.request_list[targets[i]] == 2)
				{
					this.complete(targets[i]);
				}
			}
		}
	}
};

// Specific dependancies for modules that need to manipulate actual elements in the document.
if(typeof window != 'undefined' && typeof document != 'undefined')
{
	//Special "dom_loaded" module that others can require to wait for DOM load
	Modules.request_list['dom_loaded'] = 1;
	Modules.module_list['dom_loaded'] = new Object({
		name : 'dom_loaded',
		dependencies : [],
		init : function() {}
	});

	if(document.readyState == 'complete' || document.readyState == 'interactive')
	{
		Modules.complete('dom_loaded');
	}
	else
	{
		document.addEventListener('DOMContentLoaded', function(){
			Modules.complete('dom_loaded');
		}, false);
	}

	//Special "page_loaded" module that others can require to wait for page load
	Modules.request_list['page_loaded'] = 1;
	Modules.module_list['page_loaded'] = new Object({
		name : 'page_loaded',
		dependencies : [],
		init : function() {}
	});

	if(document.readyState == 'complete')
	{
		Modules.complete('page_loaded');
	}
	else
	{
		window.addEventListener('load', function(){
			Modules.complete('page_loaded');
		}, false);
	}
}
