/*
Ace Attorney Online - Load and use the howler.js sound library

*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'sound-howler',
	dependencies : [],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS
includeScript('howler.js/howler.min', false, '', function(){
	Howler.autoSuspend = false;
	Howler.autoUnlock = true;
	Modules.complete('sound-howler');
});

window.SoundHowler = new SoundHowler();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function SoundHowler() {
	var self = this;
	self.registeredSounds = [];

	// PRIVATE METHODS
	self.getSoundIndexById = function(id)
	{
		for(var i = 0; i < self.registeredSounds.length; i++)
		{
			if(self.registeredSounds[i].id == id)
			{
				return i;
			}
		}

		return MUSIC_STOP;
	};

	// PUBLIC METHODS
	self.registerSound = function(id, args) {
		// if sound already exists, return the existing sound
		var existing_sound = self.getSoundById(id);
		if(existing_sound) return existing_sound;

		var srcs = [];
		if(args.urls) {
			srcs = args.urls;
		}
		if(args.url){
			srcs.push(args.url);
		}

		var loop, onload_new;
		var useHtml5 = !!args.html5;

		// To get a loop starting at a specific time, we need sprites.
		// However, sprites are NOT supported in HTML5 Audio mode.
		// For HTML5 mode, we use seek-based looping via the onend callback.
		if (args.loop && args.loop.start) {
			if (useHtml5) {
				// HTML5 Audio: seek-based loop (sprites don't work in HTML5 mode)
				var loopStartSec = args.loop.start / 1000;
				onload_new = function() {
					this._loopStart = loopStartSec;
					if (args.onload) args.onload(this);
				};
				loop = false; // We manage looping manually via onend
			} else {
				// Web Audio: sprite-based loop (original behavior)
				onload_new = function() {
					this._sprite = {
						intro: [0, args.loop.start],
						loop: [args.loop.start, this._duration * 1000 - args.loop.start, true]
					}
					args.onload(this);
				}
				loop = false;
			}
		} else {
			onload_new = args.onload;
			loop = !!args.loop;
		}

		// Define the Howl. Other arguments could be passed, if applicable.
		var newHowl = new Howl({
			src: srcs,
			volume: args.volume ? Math.min(1.0, args.volume / 100) : 1.0,
			loop: loop,
			html5: useHtml5,
			onload: onload_new ? onload_new : undefined,
			onloaderror: args.onloaderror ? args.onloaderror : undefined,
			onplay: args.onplay ? args.onplay : undefined,
			preload: (args.preload !== undefined) ? args.preload : true
		});

		// For HTML5 Audio with loop_start: set up seek-based looping.
		// When the track ends, spawn a new playback node and seek it to loop_start.
		// IMPORTANT: play() must come BEFORE seek() to avoid a race condition where
		// seek() mutates the dying node while _ended() is still cleaning it up,
		// causing two concurrent <audio> elements on Android WebView.
		if (useHtml5 && args.loop && args.loop.start) {
			var loopStartSec = args.loop.start / 1000;
			newHowl.on('end', function() {
				var newId = newHowl.play();
				if (typeof newId === 'number') {
					newHowl.seek(loopStartSec, newId);

					// WORKAROUND for Howler v2.2.4 _playLock queue leak:
					// play() sets _playLock = true while the HTML5 <audio>
					// play() Promise is pending. The seek() above gets queued
					// instead of executing. When the Promise resolves, Howler
					// releases the lock but does NOT call _loadQueue() (only
					// does so when internal=true). Force a drain on the next
					// tick — by then the Promise microtask has resolved and
					// _playLock is false, so the queued seek executes normally.
					// _loadQueue() with no event arg executes the first queued
					// action. For a dedicated music Howl this is always our seek.
					setTimeout(function() {
						if (newHowl && newHowl._queue && newHowl._queue.length > 0) {
							newHowl._loadQueue();
						}
					}, 0);
				}
			});
		}

		var newSound = new Object({
			id: id,
			howl: newHowl
		});

		self.registeredSounds.push(newSound);

		return newSound.howl;
	};

	self.playSound = function(id, from_loop)
	{
		var sound = self.getSoundById(id);
		if (sound._sprite.intro) {
			if(from_loop) {
				return sound.play("loop");
			}
			else {
				sound._onend = [
				{ fn:
				  function(x) {
					  sound.play("loop");
					  sound._onend = [];
				  }
				}
				]
				return sound.play("intro");
			}
		} else if (sound !== null) return sound.play();
	};

	self.pauseSound = function(id)
	{
		var sound = self.getSoundById(id);
		if (sound !== null) sound.pause();
	};

	self.stopSound = function(id)
	{
		var sound = self.getSoundById(id);
		if (sound !== null) sound.stop();
	};

	self.setSoundVolume = function(id, volume)
	{
		var sound = self.getSoundById(id);
		// Remember to cap volume at 1.0. HTML5 won't allow anything more, and Howler hangs
		if (sound !== null) sound.volume(Math.min(1.0, volume / 100));
	};

	self.setSoundPlaybackRate = function(id, rate)
	{
		var sound = self.getSoundById(id);
		if (sound !== null) sound.rate(rate);
	};

	self.unloadSound = function(id)
	{
		var sound_index = self.getSoundIndexById(id);
		if(sound_index != MUSIC_STOP)
		{
			// kill and then remove the sound
			self.registeredSounds[sound_index].howl.unload();
			self.registeredSounds.splice(sound_index, 1);
		}
	};

	self.fadeSound = function(id, duration, to_volume, endFadeCallback)
	{
		var sound = self.getSoundById(id);
		if (sound !== null) {
			var from_volume = sound.volume();
			to_volume = Math.min(1.0, to_volume / 100);

			if(endFadeCallback !== null)
			{
			   sound.once("fade", endFadeCallback);
			}
			sound.fade(from_volume, to_volume, duration);
		}
	};

	self.getSoundById = function(id)
	{
		for(var sound in self.registeredSounds)
		{
			if(self.registeredSounds[sound].id == id)
			{
				return self.registeredSounds[sound].howl;
			}
		}

		return null;
	};
}

//END OF MODULE

