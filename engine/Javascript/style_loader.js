"use strict";
/*
 * Ace Attorney Online - CSS style loader
 *
 * Simplified: CSS is loaded via standard <link> tags in PHP.
 * The vendor-prefix and data-URI conversion (StyleFixer) has been removed
 * because all CSS properties used (transition, transform, linear-gradient,
 * @keyframes, flex) have been unprefixed in browsers since 2014-2015.
 *
 * This module remains as a stub so the 7 modules that list 'style_loader'
 * in their dependencies continue to resolve correctly.
 */

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'style_loader',
	dependencies : [],
	init : function(){
		// Remove loading mask — CSS is already loaded via <link> tags
		var loading_mask = document.getElementById('loading-mask');
		if(loading_mask && loading_mask.parentNode)
		{
			loading_mask.parentNode.removeChild(loading_mask);
		}
	}
}));

//EXPORTED VARIABLES
var style_loading_bar = null;

//EXPORTED FUNCTIONS

/**
 * Include a CSS stylesheet by name.
 * No-op: CSS is now loaded via static <link> tags in PHP.
 * Kept for backward compatibility.
 */
function includeStyle(name, generated, param)
{
	// No-op — CSS loaded via <link> tags
}

//END OF MODULE
Modules.complete('style_loader');
