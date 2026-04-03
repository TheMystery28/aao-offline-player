"use strict";
/*
Ace Attorney Online - Player sound loader

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'player_sound',
	dependencies : ['engine_events', 'trial', 'frame_data', 'sound-howler', 'loading_bar', 'language'],
	init : function()
	{
		if(trial_data)
		{
			// If there is data to preload...
			
			// Set preload object
			var loading_screen = document.getElementById('screen-loading');
			var sounds_loading_label = document.createElement('p');
			sounds_loading_label.setAttribute('data-locale-content', 'loading_sounds');
			loading_screen.appendChild(sounds_loading_label);
			var sounds_loading = new LoadingBar();
			loading_screen.appendChild(sounds_loading.element);
			translateNode(sounds_loading_label);
			
			// Resume audio context on first user interaction (required by mobile browsers).
			// Without this, audio playback fails silently on Android WebView because
			// the AudioContext starts in 'suspended' state and requires a user gesture.
			if (typeof Howler !== 'undefined' && Howler.ctx && Howler.ctx.state === 'suspended') {
				var resumeAudio = function() {
					Howler.ctx.resume();
					document.removeEventListener('click', resumeAudio, true);
					document.removeEventListener('touchstart', resumeAudio, true);
				};
				document.addEventListener('click', resumeAudio, true);
				document.addEventListener('touchstart', resumeAudio, true);
			}

			// Suspend/resume audio when app goes to background/foreground.
			// On Android, the OS cleans up audio socket connections while backgrounded.
			// If Chromium's renderer tries to write to the dead socket on resume,
			// it crashes with "ASR: No room in socket buffer: Broken pipe",
			// which kills the entire iframe renderer process.
			// Fix: suspend the AudioContext before backgrounding so nothing writes
			// to the socket. Resume it when the app comes back.
			document.addEventListener('visibilitychange', function() {
				if (typeof Howler === 'undefined' || !Howler.ctx) return;
				if (document.hidden) {
					Howler.ctx.suspend();
				} else {
					Howler.ctx.resume();
				}
			});

			// Register all music files with lazy loading (streams on-demand).
			// The visibilitychange handler above suspends/resumes AudioContext
			// to prevent the Android "Broken pipe" crash during background/foreground transitions.
			for(var i = 1; i < trial_data.music.length; i++)
			{
				sounds_loading.addOne();
				var url = getMusicUrl(trial_data.music[i]);
				var music_id = 'music_' + trial_data.music[i].id;
				SoundHowler.registerSound(music_id, {
					url: url,
					html5: true,
					preload: false,
					loop: {
						start: trial_data.music[i].loop_start
					},
					volume: trial_data.music[i].volume
				});
				sounds_loading.loadedOne();
			}
			
			// Load all sound files
			for(var i = 1; i < trial_data.sounds.length; i++)
			{
				sounds_loading.addOne();
				var url = getSoundUrl(trial_data.sounds[i])
				var sound_id = 'sound_' + trial_data.sounds[i].id;
				SoundHowler.registerSound(sound_id, {
					url: url,
					onload: sounds_loading.loadedOne,
					onloaderror: sounds_loading.failedOne.bind(sounds_loading, `Sound ID #${trial_data.sounds[i].id}: ${url}`),
					volume: trial_data.sounds[i].volume
				});
			}
			
			// Load only voices actually used in frames or by profiles
			var usedVoices = {};
			for(var fi = 1; fi < trial_data.frames.length; fi++)
			{
				var f = trial_data.frames[fi];
				if(!f) continue;
				// Explicit voice override (not VOICE_AUTO=-4)
				if(f.speaker_voice && f.speaker_voice < 0 && f.speaker_voice !== -4)
				{
					usedVoices[-f.speaker_voice] = true;
				}
			}
			// Also check profile default voices (used when frame has VOICE_AUTO)
			for(var pi = 1; pi < trial_data.profiles.length; pi++)
			{
				var p = trial_data.profiles[pi];
				if(p && p.voice && p.voice < 0 && p.voice !== -4)
				{
					usedVoices[-p.voice] = true;
				}
			}
			for(var i = 1; i <= 3; i++)
			{
				if(!usedVoices[i]) continue;
				sounds_loading.addOne();
				var url = getVoiceUrls(-i)
				var voice_id = 'voice_-' + i;
				SoundHowler.registerSound(voice_id, {
					urls: url,
					loop: false,
					onload: sounds_loading.loadedOne,
					onloaderror: sounds_loading.failedOne.bind(sounds_loading, `Voice ID #${i}: ${url}`),
					volume: 70
				});
			}
		}
	}
}));

//INDEPENDENT INSTRUCTIONS
var current_music_id;
var _musicPositionCache = 0;
var _musicPositionRAF = null;

// Heartbeat: cache the music position every animation frame while playing.
// Needed because Howler's stop() resets <audio>.currentTime to 0, making
// howl.seek() useless for recovery after silent audio death on Android.
function _trackMusicPosition()
{
	if(current_music_id && current_music_id != MUSIC_STOP)
	{
		var howl = SoundHowler.getSoundById('music_' + current_music_id);
		if(howl && howl.playing())
		{
			var pos = howl.seek();
			if(typeof pos === 'number' && pos > 0) _musicPositionCache = pos;
		}
	}
	_musicPositionRAF = requestAnimationFrame(_trackMusicPosition);
}

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function playSound(sound_id)
{
	SoundHowler.playSound('sound_' + sound_id);
	EngineEvents.emit('sound:play', { soundId: sound_id });
}

function playMusic(music_id)
{
	var howler_id = 'music_' + music_id;
	var needsRestart = (current_music_id != music_id);
	var recoveryPosition = 0;

	// Liveness check: on Android WebView, the <audio> element may have
	// silently died (rejected play() Promise, OS audio session kill).
	// Detect this by checking howl.playing() and recover from cached position.
	if(!needsRestart && current_music_id != MUSIC_STOP)
	{
		var howl = SoundHowler.getSoundById(howler_id);
		if(howl && !howl.playing())
		{
			needsRestart = true;
			recoveryPosition = _musicPositionCache;
		}
	}

	if(needsRestart)
	{
		stopMusic();
		// Reset the volume, if a fade changed it.
		SoundHowler.setSoundVolume(howler_id, getRowById('music', music_id).volume);
		var playId = SoundHowler.playSound(howler_id);

		// If recovering a dead track, seamlessly seek to where it died
		if(recoveryPosition > 0 && typeof playId === 'number')
		{
			var activeHowl = SoundHowler.getSoundById(howler_id);
			if(activeHowl) activeHowl.seek(recoveryPosition, playId);
		}

		current_music_id = music_id;
		_musicPositionCache = recoveryPosition;
		EngineEvents.emit('music:play', { musicId: music_id });

		// Start position tracking if not already running
		if(!_musicPositionRAF) _trackMusicPosition();
	}
}

function crossfadeMusic(to_music_id, same_position, to_volume, duration)
{
	if(current_music_id == to_music_id)
	{
		// All we need is to adjust the volume.
		fadeMusic(to_volume, duration);
	}

	else if(current_music_id == MUSIC_STOP)
	{
		// Fade into having music.
		current_music_id = to_music_id;
		SoundHowler.setSoundVolume('music_' + to_music_id, 0);
		SoundHowler.playSound('music_' + to_music_id);
		fadeMusic(to_volume, duration);
	}

	else
	{
		// Fade from track to another.
		var prev_music_id = current_music_id;

		var current_music_obj = SoundHowler.getSoundById('music_' + current_music_id);
		var to_music_obj = SoundHowler.getSoundById('music_' + to_music_id);

		if(!to_music_obj.playing()) 
		{
			to_music_obj.volume(0);
		}

		SoundHowler.fadeSound('music_' + prev_music_id, duration, 0, function()
		{
			SoundHowler.stopSound('music_' + prev_music_id);
		});

		if(same_position) 
		{
			var newPosition = current_music_obj.seek() % to_music_obj.duration();
			var playFromLoop = (to_music_obj._sprite.loop) && (newPosition * 1000 >= to_music_obj._sprite.loop[0]);

			SoundHowler.playSound('music_' + to_music_id, playFromLoop);
			to_music_obj.seek(newPosition);
		}
		else {
			SoundHowler.playSound('music_' + to_music_id);
		}

		var base_volume = getRowById('music', to_music_id).volume;
		var end_volume = base_volume * (to_volume / 100);
		SoundHowler.fadeSound('music_' + to_music_id, duration, end_volume);

		current_music_id = to_music_id;
	}
}

function fadeMusic(to_volume, duration, callback)
{
	if(current_music_id && current_music_id != MUSIC_STOP)
	{
		var base_volume = getRowById('music', current_music_id).volume;
		var end_volume = base_volume * (to_volume / 100);
		SoundHowler.fadeSound('music_' + current_music_id, duration, end_volume, callback);
	}
}

function stopMusic()
{
	SoundHowler.stopSound('music_' + current_music_id);
	current_music_id = MUSIC_STOP;
	_musicPositionCache = 0;
	if(_musicPositionRAF) { cancelAnimationFrame(_musicPositionRAF); _musicPositionRAF = null; }
	EngineEvents.emit('music:stop', {});
}

function stopNonMusicSounds()
{
	for(var i = 0; i < SoundHowler.registeredSounds.length; i++)
	{
		var sid = SoundHowler.registeredSounds[i].id;
		if(sid.indexOf('music_') !== 0)
		{
			SoundHowler.registeredSounds[i].howl.stop();
		}
	}
}

// All "sound player" functions are needed for a minimalist music player.
// Currently, this is needed for audio evidence in the Court Record.
function updateSoundPlayerProgress(sound) {
	position_bar.max = sound.duration();
	position_bar.value = sound.seek();
}

function createSoundPlayer(url, sound_id)
{
	var player = document.createElement('div');
	addClass(player, 'sound_player');
	
	var play_button = document.createElement('button');
	setNodeTextContents(play_button, '▶');
	player.appendChild(play_button);
	
	var pause_button = document.createElement('button');
	setNodeTextContents(pause_button, '▮▮');
	player.appendChild(pause_button);
	
	var position_bar = document.createElement('progress');
	player.appendChild(position_bar);
	
	var sound = SoundHowler.getSoundById(sound_id) || SoundHowler.registerSound(sound_id, {
		url: url
	});
	
	if(sound.seek() > 0)
	{
		updateSoundPlayerProgress(sound);
	}
	else
	{
		position_bar.max = 1;
		position_bar.value = 0;
	}

	var playAndUpdatePositionBar = function(sound) {
		sound.play();
		if(!updateInterval) {
			updateInterval = setInterval(function() {
				updateSoundPlayerProgress(sound);
				if(!sound.playing())
				{
					clearInterval(updateInterval);
					updateInterval = null;
				}
			}, 100);
		}
	}

	// Every 100 ms, update the current audio position displayed on the player.
	registerEventHandler(play_button, 'click', playAndUpdatePositionBar.bind(sound), false);
	registerEventHandler(pause_button, 'click', sound.pause, false);
	registerEventHandler(position_bar, 'click', function(e) {
		var bar_screen_pos = this.getBoundingClientRect();
		var new_ratio_position = (e.screenX - bar_screen_pos.left) / this.clientWidth;
		var new_position = Math.floor(new_ratio_position * position_bar.max);

		sound.seek(new_position);
		playAndUpdatePositionBar(sound);
	}, false);
	
	return player;
}

//END OF MODULE
Modules.complete('player_sound');
