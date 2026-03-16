"use strict";
/*
Ace Attorney Online - Loading bars

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'loading_bar',
	dependencies : ['nodes', 'events'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS


//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function LoadingBar()
{
	var self = this;
	
	self.element = document.createElement('div');
	addClass(self.element, 'loading-bar');
	
	self.loadedProgress = document.createElement('span');
	addClass(self.loadedProgress, 'loading-bar-loaded');
	self.element.appendChild(self.loadedProgress);
	
	self.failedProgress = document.createElement('span');
	addClass(self.failedProgress, 'loading-bar-failed');
	self.element.appendChild(self.failedProgress);
	
	var current_target = 0;
	var current_loaded = 0;
	var current_failed = 0;
	var current_timeout = null;
	var rafPending = false;

	// Batch DOM updates into requestAnimationFrame to avoid 400+ style mutations
	// during loading. Counter updates (addOne/loadedOne/failedOne) are immediate,
	// but actual style changes are batched to ~60 updates/sec.
	self.updateDisplay = function()
	{
		if (!rafPending)
		{
			rafPending = true;
			requestAnimationFrame(function()
			{
				rafPending = false;
				var loadProgressValue = (current_target > 0 ? current_loaded / current_target : 1);

				self.loadedProgress.style.width = loadProgressValue * 100 + '%';
				self.failedProgress.style.width = (current_target > 0 ? current_failed / current_target : 0) * 100 + '%';

				if(loadProgressValue == 1)
				{
					window.clearTimeout(current_timeout);
					current_timeout = window.setTimeout(function(){
						// Recheck with latest values in case more items were added
						var recheck = (current_target > 0 ? current_loaded / current_target : 1);
						if(recheck == 1)
						{
							// If everything loaded properly and nothing new to load, trigger a loadComplete event
							triggerEvent(self.element, 'loadComplete');
						}
					}, 100);
				}
			});
		}
	};


	// Add an object to load
	self.addOne = function()
	{
		current_target++;
		self.updateDisplay();
	};


	// Note one object as loaded
	self.loadedOne = function()
	{
		current_loaded++;
		self.updateDisplay();
	};


	// Note one object as failed to load
	self.failedOne = function(resource_id)
	{
		current_failed++;
		if (typeof resource_id !== "undefined") console.log("Failed to load: " + resource_id);
		self.updateDisplay();
	};

	self.updateDisplay(); // Trigger display update at least once, and load complete event even if nothing to load
}

//END OF MODULE
Modules.complete('loading_bar');
