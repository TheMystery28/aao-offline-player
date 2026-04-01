/**
 * Player module: manages the game iframe, toolbar, and save backup/restore.
 *
 * @param {AppContext} ctx
 */
export function initPlayer(ctx) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;
  var loadLibrary = ctx.loadLibrary;
  var loadGlobalPluginsPanel = ctx.loadGlobalPluginsPanel;
  var writeGameSaves = ctx.writeGameSaves;
  var nextBridgeId = ctx.nextBridgeId;

  // DOM refs (owned exclusively by the player module)
  var launcher = document.getElementById("launcher");
  var playerContainer = document.getElementById("player-container");
  var gameFrame = document.getElementById("game-frame");
  var backBtn = document.getElementById("back-btn");
  var playerTitle = document.getElementById("player-title");
  var settingsAutoSave = document.getElementById("settings-autosave");

  // Update toolbar title when iframe navigates (added once, not per showPlayer call)
  gameFrame.addEventListener("load", function () {
    if (playerContainer.classList.contains("hidden")) return;
    try {
      var iframeDoc = gameFrame.contentDocument || gameFrame.contentWindow.document;
      var iframeTitle = iframeDoc.title;
      if (iframeTitle && iframeTitle.indexOf(' - Ace Attorney Online') !== -1) {
        iframeTitle = iframeTitle.replace(' - Ace Attorney Online', '');
      }
      if (iframeTitle && iframeTitle !== 'Ace Attorney Online - Trial Player') {
        playerTitle.textContent = iframeTitle;
      }
    } catch (e) { /* cross-origin */ }
  });

  // --- Player ---

  function showPlayer(title, url, author) {
    console.log("[PLAYER] showPlayer title=" + title + " url=" + url);
    if (author) {
      playerTitle.textContent = title + " — " + author;
    } else {
      playerTitle.textContent = title;
    }
    gameFrame.src = url;
    launcher.classList.add("hidden");
    playerContainer.classList.remove("hidden");

    // Push history state so Android back button returns to launcher instead of blank screen
    history.pushState({ player: true }, "", "");

    // Debug: capture resource load errors from the iframe (one-time)
    gameFrame.addEventListener("load", function onFrameLoad() {
      gameFrame.removeEventListener("load", onFrameLoad);
      try {
        var iframeDoc = gameFrame.contentDocument || gameFrame.contentWindow.document;
        console.log("[PLAYER] Iframe loaded. baseURI=" + iframeDoc.baseURI);
        console.log("[PLAYER] Iframe location=" + gameFrame.contentWindow.location.href);

        // List all <script> tags loaded in the iframe
        var scripts = iframeDoc.querySelectorAll("script");
        console.log("[PLAYER] Iframe scripts loaded: " + scripts.length);
        for (var s = 0; s < scripts.length; s++) {
          console.log("[PLAYER]   script[" + s + "] src=" + (scripts[s].src || "(inline)"));
        }

        // Listen for resource load failures (images, audio, scripts)
        iframeDoc.addEventListener("error", function (e) {
          var el = e.target;
          var src = el.src || el.currentSrc || el.href || "(unknown)";
          var tag = el.tagName || "?";
          var id = el.id || "";
          var cls = el.className || "";
          console.error(
            "[IFRAME RESOURCE ERROR] <" + tag + "> src=" + src +
            (id ? " id=" + id : "") +
            (cls ? " class=" + cls : "") +
            " parentTag=" + (el.parentElement ? el.parentElement.tagName : "none")
          );
        }, true); // capture phase to catch all errors

        // Also intercept successful loads for images (so we see what works)
        iframeDoc.addEventListener("load", function (e) {
          var el = e.target;
          if (el.tagName === "IMG" || el.tagName === "AUDIO" || el.tagName === "SOURCE") {
            var src = el.src || el.currentSrc || "(unknown)";
            console.log("[IFRAME RESOURCE OK] <" + el.tagName + "> src=" + src);
          }
        }, true);

        console.log("[PLAYER] Iframe error+load listeners attached");
      } catch (ex) {
        console.warn("[PLAYER] Cannot attach iframe listeners (cross-origin?): " + ex.message);
      }
    });
  }

  function showLauncher() {
    // Exit fullscreen when returning to launcher — library should never be fullscreen
    try {
      if (window.__TAURI__ && window.__TAURI__.window) {
        window.__TAURI__.window.getCurrentWindow().setFullscreen(false);
      }
      // Sync engine config so fullscreen checkbox updates
      if (gameFrame.contentWindow) {
        gameFrame.contentWindow.postMessage({ type: 'aao-set-config', path: 'display.fullscreen', value: false }, '*');
      }
    } catch (e) {}

    // Close iframe and back up saves. If we received save data directly from the
    // engine (via auto_save_complete), write it to disk immediately. Otherwise
    // fall back to reading via the bridge iframe.
    function finishQuit(savesDataString) {
      gameFrame.src = "about:blank";
      playerContainer.classList.add("hidden");
      launcher.classList.remove("hidden");
      statusMsg.textContent = "";
      loadGlobalPluginsPanel();

      if (savesDataString) {
        try {
          var parsed = JSON.parse(savesDataString);
          invoke("backup_saves", { saves: parsed }).then(function () {
            console.log("[SAVE] Auto-save backed up directly to disk");
          }).catch(function (e) {
            console.warn("[SAVE] Direct backup failed:", e);
            backupSavesToFile();
          });
        } catch (e) {
          console.warn("[SAVE] Failed to parse direct save data:", e);
          backupSavesToFile();
        }
      } else {
        backupSavesToFile();
      }
    }

    // Auto-save before quitting (if enabled in settings).
    // Uses event-driven handshake: engine posts auto_save_complete with save data,
    // so we don't rely on localStorage flush timing (critical on Android).
    if (!settingsAutoSave || settingsAutoSave.checked) {
      if (gameFrame.contentWindow && gameFrame.src !== "about:blank") {
        var saveTimeout;
        var onSaveComplete = function (event) {
          if (event.data && event.data.type === "auto_save_complete") {
            window.removeEventListener("message", onSaveComplete);
            clearTimeout(saveTimeout);
            finishQuit(event.data.data);
          }
        };
        window.addEventListener("message", onSaveComplete);
        try {
          gameFrame.contentWindow.postMessage({ type: "auto_save" }, "*");
        } catch (e) {
          console.warn("[PLAYER] Auto-save postMessage failed:", e.message);
          window.removeEventListener("message", onSaveComplete);
          finishQuit(null);
          return;
        }
        // Fallback timeout if engine doesn't respond (1s is safe)
        saveTimeout = setTimeout(function () {
          window.removeEventListener("message", onSaveComplete);
          console.warn("[PLAYER] Auto-save timed out");
          finishQuit(null);
        }, 1000);
      } else {
        finishQuit(null);
      }
    } else {
      finishQuit(null);
    }
  }

  backBtn.addEventListener("click", showLauncher);

  // Toolbar acts as a window title bar:
  // - In fullscreen: dragging exits fullscreen then starts window drag
  // - Not in fullscreen: dragging moves the window
  var toolbarEl = document.getElementById("player-toolbar");
  toolbarEl.addEventListener("mousedown", function(e) {
    if (e.target === backBtn || e.buttons !== 1) return;
    try {
      if (!window.__TAURI__ || !window.__TAURI__.window) return;
      var win = window.__TAURI__.window.getCurrentWindow();
      win.isFullscreen().then(function(isFs) {
        if (isFs) {
          // Exit fullscreen first, then start dragging
          win.setFullscreen(false).then(function() {
            var frame = document.getElementById("game-frame");
            if (frame && frame.contentWindow) {
              frame.contentWindow.postMessage({ type: 'aao-set-config', path: 'display.fullscreen', value: false }, '*');
            }
            win.startDragging();
          });
        } else {
          win.startDragging();
        }
      });
    } catch (err) {}
  });

  // Listen for messages from the engine iframe
  window.addEventListener("message", function(e) {
    if (!e.data || !e.data.type) return;
    if (e.data.type === 'aao-header-visibility') {
      if (e.data.hidden) {
        playerTitle.style.fontFamily = 'Georgia, serif';
      } else {
        playerTitle.style.fontFamily = '';
      }
    } else if (e.data.type === 'aao-title-update') {
      // Engine loaded a new case — update toolbar title
      var text = e.data.title || '';
      if (e.data.author) text += ' — ' + e.data.author;
      if (text) playerTitle.textContent = text;
    } else if (e.data.type === 'aao-fullscreen') {
      // Toggle Tauri window fullscreen via __TAURI__ global
      try {
        if (window.__TAURI__ && window.__TAURI__.window) {
          window.__TAURI__.window.getCurrentWindow().setFullscreen(!!e.data.fullscreen);
        }
      } catch (err) {
        console.warn('[MAIN] Failed to toggle fullscreen:', err);
      }
    } else if (e.data.type === 'aao-attach-code') {
      // Persist plugin code from the player's in-game Attach Code to Rust backend
      var caseIds = e.data.caseId ? [e.data.caseId] : [];
      invoke("attach_plugin_code", {
        code: e.data.code,
        filename: e.data.filename,
        targetCaseIds: caseIds
      }).then(function() {
        if (gameFrame.contentWindow) {
          gameFrame.contentWindow.postMessage({
            type: 'aao-attach-code-result', success: true
          }, '*');
        }
      }).catch(function(err) {
        if (gameFrame.contentWindow) {
          gameFrame.contentWindow.postMessage({
            type: 'aao-attach-code-result', success: false, error: String(err)
          }, '*');
        }
      });
    }
  });

  // Android system back button: return to launcher if player is visible
  window.addEventListener("popstate", function () {
    if (!playerContainer.classList.contains("hidden")) {
      showLauncher();
      loadLibrary();
    }
  });

  // --- Save Backup/Restore ---

  // Back up saves from localStorage to a file (survives app updates/reinstalls)
  function backupSavesToFile() {
    invoke("get_server_url").then(function (serverUrl) {
      var bridgeId = nextBridgeId();
      var iframe = document.createElement("iframe");
      iframe.style.display = "none";
      var done = false;

      function onMsg(event) {
        if (done || !event.data || event.data.type !== "game_saves") return;
        if (event.data.bridgeId && event.data.bridgeId !== bridgeId) return;
        done = true;
        window.removeEventListener("message", onMsg);
        if (iframe.parentNode) document.body.removeChild(iframe);

        var raw = event.data.data;
        if (raw) {
          var parsed = JSON.parse(raw);
          invoke("backup_saves", { saves: parsed }).then(function () {
            console.log("[SAVE] Backed up saves to file");
          }).catch(function (e) {
            console.warn("[SAVE] Backup failed:", e);
          });
        }
      }

      window.addEventListener("message", onMsg);
      setTimeout(function () {
        if (!done) {
          done = true;
          window.removeEventListener("message", onMsg);
          if (iframe.parentNode) document.body.removeChild(iframe);
        }
      }, 3000);
      iframe.src = serverUrl + "/localstorage_bridge.html?id=" + bridgeId;
      document.body.appendChild(iframe);
    });
  }

  // Restore saves from backup file to localStorage (runs once on startup)
  function restoreSavesFromBackup() {
    invoke("load_saves_backup").then(function (saves) {
      if (!saves) {
        console.log("[SAVE] No saves backup to restore");
        return;
      }
      console.log("[SAVE] Restoring saves from backup...");
      writeGameSaves(saves).then(function (result) {
        if (result && result.success) {
          console.log("[SAVE] Restored " + result.merged + " saves from backup");
        } else {
          console.warn("[SAVE] Restore failed:", result && result.error);
        }
      });
    }).catch(function (e) {
      console.warn("[SAVE] Could not load backup:", e);
    });
  }

  // Restore saves on startup
  restoreSavesFromBackup();

  // Catch manual save/delete events from the engine and back up to disk immediately.
  // This bypasses the localStorage flush race on Android — saves go straight to Rust.
  window.addEventListener("message", function (event) {
    if (event.data && event.data.type === "save_data_changed" && event.data.data) {
      try {
        var parsed = JSON.parse(event.data.data);
        invoke("backup_saves", { saves: parsed }).then(function () {
          console.log("[SAVE] Save change backed up to disk");
        });
      } catch (e) {
        console.error("[SAVE] Failed to parse save data:", e);
      }
    }
  });

  return { showPlayer: showPlayer, showLauncher: showLauncher };
}
