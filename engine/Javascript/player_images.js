"use strict";
/*
Ace Attorney Online - Player image loader

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'player_images',
	dependencies : ['trial', 'default_data', 'frame_data', 'events', 'page_loaded', 'loading_bar'],
	init : function()
	{
		if(trial_data)
		{
			// If there is data to preload...
			// Set preload object
			var loading_screen = document.getElementById('screen-loading');
			var images_loading_label = document.createElement('p');
			images_loading_label.setAttribute('data-locale-content', 'loading_images');
			loading_screen.appendChild(images_loading_label);
			images_loading = new LoadingBar();
			loading_screen.appendChild(images_loading.element);
			translateNode(images_loading_label);

			// Scan frames to find which default sprites and places are actually used.
			// This avoids preloading hundreds of unused sprites that cause 404s
			// (especially for imported aaoffline cases where only used assets exist).
			var usedDefaultSprites = {}; // "profileId:spriteAbsId" → true
			var usedDefaultPlaces = {}; // placeId → true
			var usedVoiceIds = {}; // voiceAbsId → true

			for(var i = 1; i < trial_data.frames.length; i++)
			{
				var frame = trial_data.frames[i];
				if(!frame) continue;

				// Characters with default sprites (negative sprite_id)
				var characters = frame.characters || [];
				for(var c = 0; c < characters.length; c++)
				{
					if(characters[c] && characters[c].profile_id && characters[c].sprite_id < 0)
					{
						usedDefaultSprites[characters[c].profile_id + ':' + (-characters[c].sprite_id)] = true;
					}
				}

				// Default places (negative place ID)
				if(frame.place && frame.place < 0)
				{
					usedDefaultPlaces[frame.place] = true;
				}

				// Explicit voice override (not VOICE_AUTO)
				if(frame.speaker_voice && frame.speaker_voice < 0 && frame.speaker_voice !== -4) // -4 = VOICE_AUTO
				{
					usedVoiceIds[-frame.speaker_voice] = true;
				}
			}

			// Also scan examination places (examinations can set default places)
			if(trial_data.scenes)
			{
				for(var si = 1; si < trial_data.scenes.length; si++)
				{
					var scene = trial_data.scenes[si];
					if(!scene || !scene.examinations) continue;
					for(var ei = 0; ei < scene.examinations.length; ei++)
					{
						var exam = scene.examinations[ei];
						if(exam && exam.place && exam.place < 0)
						{
							usedDefaultPlaces[exam.place] = true;
						}
					}
				}
			}

			// Collect voices from profiles (VOICE_AUTO frames use the profile's voice)
			for(var i = 1; i < trial_data.profiles.length; i++)
			{
				var p = trial_data.profiles[i];
				if(p && p.voice && p.voice < 0 && p.voice !== -4)
				{
					usedVoiceIds[-p.voice] = true;
				}
			}

			// Load all evidence icons
			for(var i = 1; i < trial_data.evidence.length; i++)
			{
				preloadImage(getEvidenceIconUrl(trial_data.evidence[i]), `Evidence icon #${trial_data.evidence[i].id}`);
			}

			// Load all profile images
			for(var i = 1; i < trial_data.profiles.length; i++)
			{
				var profile = trial_data.profiles[i];
				preloadImage(getProfileIconUrl(profile), `Profile icon ID #${profile.id}`); // Profile icon

				for(var j = 0; j < profile.custom_sprites.length; j++) // Custom sprites
				{
					let imgOriginInfo = `Character ID #${profile.id}, pose "${profile.custom_sprites[j].name}"`;

					if(profile.custom_sprites[j].talking)
					{
						preloadImage(profile.custom_sprites[j].talking, `${imgOriginInfo} talking`);
					}
					if(profile.custom_sprites[j].still)
					{
						preloadImage(profile.custom_sprites[j].still, `${imgOriginInfo} still`);
					}
					if(profile.custom_sprites[j].startup)
					{
						preloadImage(profile.custom_sprites[j].startup, `${imgOriginInfo} startup`);
					}
				}

				// Only preload default sprites actually used in frames
				for(var j = 1; j <= default_profiles_nb[profile.base]; j++)
				{
					if(!usedDefaultSprites[profile.id + ':' + j]) continue;

					let imgOriginInfo = `Default sprite "${profile.base}", index #${j}`;

					preloadImage(getDefaultSpriteUrl(profile.base, j, 'talking'), `${imgOriginInfo} talking`);
					preloadImage(getDefaultSpriteUrl(profile.base, j, 'still'), `${imgOriginInfo} still`);
					if(default_profiles_startup[profile.base + '/' + j])
					{
						preloadImage(getDefaultSpriteUrl(profile.base, j, 'startup'), `${imgOriginInfo} startup`);
					}
				}
			}

			// Load all place images
			for(var i = 1; i < trial_data.places.length; i++) // Custom places
			{
				preloadPlaceImages(trial_data.places[i], `Custom place ID #${trial_data.places[i].id}`);
			}

			// Only preload default places actually used in frames
			for(var i in default_places)
			{
				if(usedDefaultPlaces[default_places[i].id])
				{
					preloadPlaceImages(default_places[i], `Default place ID #${default_places[i].id}`);
				}
			}

			// Load all popup images
			for(var i = 1; i < trial_data.popups.length; i++) // Custom places
			{
				preloadImage(getPopupUrl(trial_data.popups[i]), `Popup image ID #${trial_data.popups[i].id}`);
			}
		}
	}
}));

//INDEPENDENT INSTRUCTIONS
var images_loading;
var nb_images_to_load = 0;
var nb_images_loaded = 0;
var nb_images_failed = 0;

function preloadImage(uri, imgOriginInfo)
{
	images_loading.addOne();

	var img = new Image();
	registerEventHandler(img, 'load', images_loading.loadedOne, false);
	registerEventHandler(img, 'error', images_loading.failedOne.bind(images_loading, `Image ${imgOriginInfo} ${uri}`), false);

	// Just setting img.src triggers the browser to fetch and cache the image.
	// No need to append to the DOM — avoids 200+ unnecessary DOM insertions.
	img.src = uri;
}

function preloadPlaceImages(place, imgOriginInfo)
{
	var background = getObjectDescriptor(place.background, 'bg_subdir');
	if(background.uri) // Place background if it's a picture
	{
		preloadImage(background.uri, `${imgOriginInfo}'s background`);
	}

	for(var j = 0; j < place.background_objects.length; j++) // Background objects
	{
		preloadImage(getObjectDescriptor(place.background_objects[j]).uri, `${imgOriginInfo}, Background object #${place.background_objects[j].id}`);
	}

	for(var j = 0; j < place.foreground_objects.length; j++) // Foreground objects
	{
		preloadImage(getObjectDescriptor(place.foreground_objects[j]).uri, `${imgOriginInfo}, Foreground object #${place.foreground_objects[j].id}`);
	}
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS


//END OF MODULE
Modules.complete('player_images');
