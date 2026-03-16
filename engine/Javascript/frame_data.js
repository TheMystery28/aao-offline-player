"use strict";
/*
Ace Attorney Online - Functions to collect data about a given frame

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'frame_data',
	dependencies : ['trial_data'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS

//EXPORTED VARIABLES

//EXPORTED FUNCTIONS
function getObjectDescriptor(obj_desc, subdir)
{
	if(obj_desc.image || obj_desc.image === '')
	{
		//if image, fetch the uri
		return new Object({
			uri: (obj_desc.external ? '' : cfg.picture_dir + (subdir ? cfg[subdir] : '') ) + obj_desc.image + ( (obj_desc.external || obj_desc.image.match(/\.gif$/)) ? '' : '.jpg')
		});
	}
	else
	{
		//if colour, just pass the descriptor
		return obj_desc;
	}
}

function getDefaultSpriteUrl(base, sprite_id, status)
{
	return cfg.picture_dir + cfg[status + '_subdir'] + base + '/' + sprite_id + '.gif';
}

function getPoseDesc(character_data)
{
	if(character_data.sprite_id > 0) //Positive IDs are for custom sprites
	{
		var array = getRowById('profiles', character_data.profile_id).custom_sprites;
		return array[getIndexById(array, character_data.sprite_id)];
	}
	else if(character_data.sprite_id < 0) //Negative IDs are for default sprites
	{
		var sprite_abs_id = - character_data.sprite_id;
		
		var character_base;
	
		if(character_data.profile_id > 0)
		{
			//User created profile
			character_base = getRowById('profiles', character_data.profile_id).base;
		}
		else if(character_data.profile_id == 0)
		{
			//Default judge profile
			character_base = 'Juge';
		}
		
		var startup;
		var startup_duration;
		if(default_profiles_startup[character_base + '/' + sprite_abs_id])
		{
			startup = getDefaultSpriteUrl(character_base, sprite_abs_id, 'startup');
			startup_duration = default_profiles_startup[character_base + '/' + sprite_abs_id];
		}
		else
		{
			startup = '';
			startup_duration = 0;
		}
		
		return new Object({
			talking: getDefaultSpriteUrl(character_base, sprite_abs_id, 'talking'),
			still: getDefaultSpriteUrl(character_base, sprite_abs_id, 'still'),
			startup: startup,
			startup_duration: startup_duration
		});
	}
	else //Null ID means no sprite
	{
		return new Object({
			talking: '',
			still: '',
			startup: '',
			startup_duration: 0
		});
	}
}

function getCharacterDescriptor(character_data, status)
{
	return new Object({
		uri: getPoseDesc(character_data)[status]
	});
}

function getPopupDescriptor(popup_info)
{
	var popup_data;
	
	if(!isNaN(popup_info.popup_id))
	{
		popup_data = getRowById('popups', popup_info.popup_id);
	}
	else
	{
		popup_data = new Object({
			external: false,
			path: popup_info.popup_id
		});
	}
	
	return new Object({
		uri: getPopupUrl(popup_data)
	});
}

function getCharacterIndexById(char_id, characters)
{
	for(var i = 0; i < characters.length; i++)
	{
		if(characters[i].profile_id == char_id)
		{
			return i;
		}
	}
	
	return -1;
}

function getSpeakerName(frame_data, only_by_id)
{
	if(!only_by_id && frame_data.speaker_use_name)
	{
		return frame_data.speaker_name;
	}
	else if(frame_data.speaker_id > 0)
	{
		return getRowById('profiles', frame_data.speaker_id).short_name;
	}
	else
	{
		switch(frame_data.speaker_id)
		{
			case PROFILE_JUDGE :
				return l('profile_judge');
				break;
			
			case PROFILE_UNKNOWN : 
				return '???';
				break;
			
			default :
				return '';
				break;
		}
	}
}

function getPlace(place_id)
{
	if(place_id > 0)
	{
		//Fetch custom place
		return getRowById('places', place_id);
	}
	else if(place_id < 0)
	{
		//Fetch default place
		return default_places[place_id];
	}
	else if(place_id !== 0)
	{
		//Generate place from default background
		var dummy_place = createDataRow('places');
		dummy_place.background = new Object({
			external: 0,
			image: place_id,
			hidden: 0
		});
		return dummy_place;
	}
}

function getPosition(position_id, place_data)
{
	if(position_id <= 0)
	{
		//Fetch default position
		return default_positions[position_id];
	}
	else
	{
		//Fetch custom position
		return place_data.positions[position_id];
	}
}

function getProfile(profile_id)
{
	if(profile_id <= 0)
	{
		// Fetch default profile
		return default_profiles[profile_id];
	}
	else
	{
		// Fetch trial profile
		return getRowById('profiles', profile_id);
	}
}

function getProfileIconUrl(profile_data)
{
	if(profile_data.icon)
	{
		return profile_data.icon;
	}
	else if(profile_data.base)
	{
		return cfg['picture_dir'] + cfg['icon_subdir'] + profile_data.base + '.png';
	}
	else
	{
		return cfg['picture_dir'] + cfg['icon_subdir'] + 'Inconnu.png';
	}
}

function getEvidenceIconUrl(evidence_data)
{
	if(evidence_data.icon_external)
	{
		return evidence_data.icon;
	}
	else if(evidence_data.icon)
	{
		return cfg['picture_dir'] + cfg['evidence_subdir'] + evidence_data.icon + '.png';
	}
	else
	{
		return cfg['picture_dir'] + cfg['icon_subdir'] + 'Inconnu.png';
	}
}

function getPopupUrl(popup_data)
{
	if(popup_data.external)
	{
		return popup_data.path;
	}
	else
	{
		return cfg.picture_dir + cfg.popups_subdir + popup_data.path + '.gif';
	}
}

function getMusicUrl(music)
{
	if(music.external)
	{
		return music.path;
	}
	else
	{
		return cfg['music_dir'] + music.path + '.mp3';
	}
}

function getSoundUrl(sound)
{
	if(sound.external)
	{
		return sound.path;
	}
	else
	{
		return cfg['sounds_dir'] + sound.path + '.mp3';
	}
}

function getActionDescription(action_name)
{
	for(var category in action_descriptions)
	{
		if(action_name in action_descriptions[category])
		{
			return action_descriptions[category][action_name];
		}
	}
	
	return null;
}

function getVoiceId(frame_data)
{
	if(frame_data.speaker_voice != VOICE_AUTO)
	{
		return frame_data.speaker_voice;
	}
	else
	{
		var profile = getProfile(frame_data.speaker_id);
		if(profile)
		{
			return profile.voice;
		}
		else
		{
			return null;
		}
	}
}

function getVoiceUrl(voice_id, ext)
{
	return cfg['voices_dir'] + 'voice_singleblip_' + (- voice_id) + '.' + ext;
}

function getVoiceUrls(voice_id)
{
	return [ getVoiceUrl(voice_id, 'opus'), getVoiceUrl(voice_id, 'wav'), getVoiceUrl(voice_id, 'mp3') ];
}

function getVoiceDelay(voice_id) {
	switch(voice_id) {
		case VOICE_MALE:
			return VOICE_MALE_DELAY;

		case VOICE_FEMALE:
			return VOICE_FEMALE_DELAY;

		case VOICE_TYPEWRITER:
			return VOICE_TYPEWRITER_DELAY;

		default:
			return -1;
	}
}

//END OF MODULE
Modules.complete('frame_data');
