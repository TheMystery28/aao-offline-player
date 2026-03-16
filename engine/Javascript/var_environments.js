"use strict";
/*
Ace Attorney Online - Variable environments

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'var_environments',
	dependencies : [],
	init : function() 
	{
		global_env = new VariableEnvironment();
		global_env.set('TRUE', true);
		global_env.set('FALSE', false);
	}
}));

//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES
var global_env;

//EXPORTED FUNCTIONS
function VariableEnvironment(next_env)
{
	this.vars = new Object();
	this.next = next_env;
	
	this.get = function(var_name)
	{
		var current_env = this;
		while(current_env)
		{
			if(var_name in current_env.vars)
			{
				return current_env.vars[var_name];
			}
			current_env = current_env.next;
		}
		
		return 0;
	};
	
	this.isSet = function(var_name)
	{
		var current_env = this;
		while(current_env)
		{
			if(var_name in current_env.vars)
			{
				return true;
			}
			current_env = current_env.next;
		}
		
		return false;
	};
	
	this.set = function(var_name, value)
	{
		this.vars[var_name] = value;
	};
	
	this.toJSON = function()
	{
		var flat_vars = {};
		
		var current_env = this;
		while(current_env)
		{
			for(var var_name in current_env.vars)
			{
				if(!(var_name in flat_vars))
				{
					flat_vars[var_name] = current_env.vars[var_name];
				}
			}
			
			current_env = current_env.next;
		}
		
		return flat_vars;
	};
}

//END OF MODULE
Modules.complete('var_environments');
