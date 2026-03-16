"use strict";
/*
Ace Attorney Online - Offline Player Bridge
Static JS equivalent of bridge.js.php for offline use.
*/

// Offline configuration — paths relative to the localhost server root
var cfg = {
	"site_name": "Ace Attorney Online",
	"picture_dir": "defaults/images/",
		"icon_subdir": "chars/",
		"talking_subdir": "chars/",
		"still_subdir": "charsStill/",
		"startup_subdir": "charsStartup/",
		"evidence_subdir": "evidence/",
		"bg_subdir": "backgrounds/",
		"defaultplaces_subdir": "defaultplaces/",
		"popups_subdir": "popups/",
		"locks_subdir": "psycheLocks/",
	"music_dir": "defaults/music/",
	"sounds_dir": "defaults/sounds/",
	"voices_dir": "defaults/voices/",
	"js_dir": "Javascript/",
	"css_dir": "CSS/",
	"lang_dir": "Languages/"
};

// No CSRF token needed offline
var csrf_token = "";

// File versions — not needed offline.
// Set to false (not an object) so getFileVersion() in common.js
// returns new Date() for any file, indicating "assume file exists".
// This prevents the language loader from thinking files don't exist.
var file_versions = false;

// Offline user language — read from URL ?lang= parameter, overridden by trial language if available
var user_language = (function() {
	var match = window.location.search.match(/[?&]lang=([^&]+)/);
	return match ? decodeURIComponent(match[1]) : "en";
})();

// Debug: log cfg paths and capture resource errors in the player iframe
console.log("[BRIDGE] cfg.picture_dir=" + cfg.picture_dir);
console.log("[BRIDGE] cfg.voices_dir=" + cfg.voices_dir);
console.log("[BRIDGE] cfg.music_dir=" + cfg.music_dir);
console.log("[BRIDGE] cfg.sounds_dir=" + cfg.sounds_dir);
console.log("[BRIDGE] document.baseURI=" + document.baseURI);
console.log("[BRIDGE] location.href=" + location.href);

// Capture ALL resource load errors in this document
document.addEventListener("error", function(e) {
    var el = e.target;
    if (el.tagName === "IMG" || el.tagName === "AUDIO" || el.tagName === "SOURCE" || el.tagName === "SCRIPT") {
        var src = el.src || el.currentSrc || el.href || "(unknown)";
        console.error("[BRIDGE RESOURCE ERROR] <" + el.tagName + "> src=" + src);
    }
}, true);
