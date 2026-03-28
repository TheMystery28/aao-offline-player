const { invoke, Channel } = window.__TAURI__.core;

/**
 * Parse a case ID from user input.
 * Accepts: numeric ID, or full/partial AAO URL containing trial_id=N or id_proces=N.
 */
function parseCaseId(input) {
  var trimmed = input.trim();
  // Pure numeric
  var num = parseInt(trimmed, 10);
  if (!isNaN(num) && num > 0 && String(num) === trimmed) {
    return num;
  }
  // URL patterns: trial_id=N or id_proces=N
  var match = trimmed.match(/(?:trial_id|id_proces)=(\d+)/);
  if (match) {
    return parseInt(match[1], 10);
  }
  return null;
}

window.addEventListener("DOMContentLoaded", function () {
  var launcher = document.getElementById("launcher");
  var playerContainer = document.getElementById("player-container");
  var gameFrame = document.getElementById("game-frame");
  var backBtn = document.getElementById("back-btn");
  var playerTitle = document.getElementById("player-title");
  var statusMsg = document.getElementById("status-msg");
  var caseList = document.getElementById("case-list");
  var emptyLibrary = document.getElementById("empty-library");
  var libraryLoading = document.getElementById("library-loading");

  // Download UI
  var downloadBtn = document.getElementById("download-btn");
  var caseIdInput = document.getElementById("case-id-input");
  var downloadResult = document.getElementById("download-result");
  var progressContainer = document.getElementById("progress-container");
  var progressPhase = document.getElementById("progress-phase");
  var progressBarInner = document.getElementById("progress-bar-inner");
  var progressText = document.getElementById("progress-text");
  var cancelDownloadBtn = document.getElementById("cancel-download-btn");

  // Helper: apply or remove spoiler blur based on the setting
  function applySpoilerBlur() {
    var blurEl = document.getElementById("settings-blur-spoilers");
    if (blurEl && blurEl.checked) {
      progressText.classList.add("spoiler-blur");
    } else {
      progressText.classList.remove("spoiler-blur");
    }
  }
  function removeSpoilerBlur() {
    progressText.classList.remove("spoiler-blur");
  }

  // Track known case IDs for duplicate detection
  var knownCaseIds = [];
  var downloadInProgress = false;
  var downloadQueue = [];

  function processQueue() {
    if (downloadInProgress || downloadQueue.length === 0) return;
    var next = downloadQueue.shift();
    statusMsg.textContent = "Starting queued download...";
    if (next.type === "single") {
      startDownload(next.caseId);
    } else if (next.type === "sequence") {
      startSequenceDownload(next.caseIds, next.sequenceTitle);
    }
  }

  // Cancel button handler
  cancelDownloadBtn.addEventListener("click", function () {
    invoke("cancel_download").then(function () {
      progressPhase.textContent = "Cancelling...";
      cancelDownloadBtn.classList.add("hidden");
    });
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

    // Update toolbar title when iframe navigates to a new case (e.g., loading save from another sequence part)
    gameFrame.addEventListener("load", function() {
      try {
        var iframeDoc = gameFrame.contentDocument || gameFrame.contentWindow.document;
        var iframeTitle = iframeDoc.title;
        // Strip the " - Ace Attorney Online" suffix if present
        if (iframeTitle && iframeTitle.indexOf(' - Ace Attorney Online') !== -1) {
          iframeTitle = iframeTitle.replace(' - Ace Attorney Online', '');
        }
        if (iframeTitle && iframeTitle !== 'Ace Attorney Online - Trial Player') {
          playerTitle.textContent = iframeTitle;
        }
      } catch (e) { /* cross-origin */ }
    });

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

    // Auto-save before quitting (if enabled in settings).
    // The iframe is cross-origin (localhost vs tauri.localhost), so use postMessage.
    if (!settingsAutoSave || settingsAutoSave.checked) {
      try {
        if (gameFrame.contentWindow && gameFrame.src !== "about:blank") {
          gameFrame.contentWindow.postMessage({ type: "auto_save" }, "*");
        }
      } catch (e) {
        console.warn("[PLAYER] Auto-save failed:", e.message);
      }
    }
    // Small delay to let the save complete, then close and back up saves
    setTimeout(function () {
      gameFrame.src = "about:blank";
      playerContainer.classList.add("hidden");
      launcher.classList.remove("hidden");
      statusMsg.textContent = "";
      // Back up all saves to file after leaving the player
      backupSavesToFile();
    }, 100);
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
    findLastSequenceSave([{ id: 0 }]); // dummy call just to get server URL — actually, let's read all saves directly
    invoke("get_server_url").then(function (serverUrl) {
      var bridgeId = "backup_" + (++bridgeIdCounter);
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

  // --- Library ---

  var librarySearch = document.getElementById("library-search");
  var librarySort = document.getElementById("library-sort");
  var cachedCases = [];
  var cachedCollections = [];

  function loadLibrary() {
    console.log("[LIBRARY] loadLibrary called");
    libraryLoading.classList.remove("hidden");
    emptyLibrary.classList.add("hidden");
    Promise.all([
      invoke("list_cases"),
      invoke("list_collections").catch(function () { return []; })
    ])
      .then(function (results) {
        cachedCases = results[0];
        cachedCollections = results[1];
        console.log("[LIBRARY] list_cases returned " + cachedCases.length + " cases, " +
          cachedCollections.length + " collections");
        knownCaseIds = cachedCases.map(function (c) { return c.case_id; });
        applySearchAndSort();
        loadStorageInfo();
        loadGlobalPluginsPanel();
      })
      .catch(function (e) {
        console.error("[LIBRARY] Failed to load library:", e);
        libraryLoading.textContent = "Failed to load library.";
      });
  }

  function applySearchAndSort() {
    var query = (librarySearch.value || "").trim().toLowerCase();
    var sortBy = librarySort.value;

    var filtered = cachedCases;
    if (query) {
      filtered = cachedCases.filter(function (c) {
        return c.title.toLowerCase().indexOf(query) !== -1 ||
               c.author.toLowerCase().indexOf(query) !== -1 ||
               String(c.case_id).indexOf(query) !== -1;
      });
    }

    var sorted = filtered.slice();
    if (sortBy === "name-asc") {
      sorted.sort(function (a, b) { return a.title.localeCompare(b.title); });
    } else if (sortBy === "name-desc") {
      sorted.sort(function (a, b) { return b.title.localeCompare(a.title); });
    } else if (sortBy === "date-new") {
      sorted.sort(function (a, b) { return (b.download_date || "").localeCompare(a.download_date || ""); });
    } else if (sortBy === "date-old") {
      sorted.sort(function (a, b) { return (a.download_date || "").localeCompare(b.download_date || ""); });
    } else if (sortBy === "size-big") {
      sorted.sort(function (a, b) { return b.assets.total_size_bytes - a.assets.total_size_bytes; });
    } else if (sortBy === "size-small") {
      sorted.sort(function (a, b) { return a.assets.total_size_bytes - b.assets.total_size_bytes; });
    }

    renderCaseList(sorted, cachedCollections, query);
  }

  var searchDebounceTimer = null;
  librarySearch.addEventListener("input", function () {
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(applySearchAndSort, 200);
  });
  librarySort.addEventListener("change", applySearchAndSort);

  function renderCaseList(cases, collections, searchQuery) {
    libraryLoading.classList.add("hidden");
    // Remove old cards, groups, and collection groups
    caseList.querySelectorAll(".case-card, .sequence-group, .collection-group").forEach(function (c) { c.remove(); });

    collections = collections || [];

    if (cases.length === 0 && collections.length === 0) {
      emptyLibrary.classList.remove("hidden");
      return;
    }

    emptyLibrary.classList.add("hidden");

    // Build lookup maps
    var casesById = {};
    for (var ci = 0; ci < cases.length; ci++) {
      casesById[cases[ci].case_id] = cases[ci];
    }

    // Group cases by sequence title
    var sequenceGroups = {}; // title → { list: [...], cases: [...] }
    var standalone = [];

    for (var i = 0; i < cases.length; i++) {
      var c = cases[i];
      var seq = c.sequence;
      if (seq && seq.title && seq.list && seq.list.length > 1) {
        var key = seq.title;
        if (!sequenceGroups[key]) {
          sequenceGroups[key] = { list: seq.list, cases: [] };
        }
        sequenceGroups[key].cases.push(c);
      } else {
        standalone.push(c);
      }
    }

    // Sort sequence groups
    var groupKeys = Object.keys(sequenceGroups);
    for (var g = 0; g < groupKeys.length; g++) {
      var groupTitle = groupKeys[g];
      var group = sequenceGroups[groupTitle];
      var listIds = group.list.map(function (p) { return p.id; });
      group.cases.sort(function (a, b) {
        return listIds.indexOf(a.case_id) - listIds.indexOf(b.case_id);
      });
    }

    // Determine which items are claimed by a collection
    var claimedCaseIds = {};
    var claimedSequenceTitles = {};
    for (var col = 0; col < collections.length; col++) {
      var items = collections[col].items || [];
      for (var it = 0; it < items.length; it++) {
        if (items[it].type === "case") {
          claimedCaseIds[items[it].case_id] = true;
        } else if (items[it].type === "sequence") {
          claimedSequenceTitles[items[it].title] = true;
        }
      }
    }

    // Render collections first
    for (var co = 0; co < collections.length; co++) {
      appendCollectionGroup(collections[co], casesById, sequenceGroups, searchQuery);
    }

    // Render remaining uncollected sequences
    for (var gs = 0; gs < groupKeys.length; gs++) {
      if (!claimedSequenceTitles[groupKeys[gs]]) {
        appendSequenceGroup(groupKeys[gs], sequenceGroups[groupKeys[gs]].list, sequenceGroups[groupKeys[gs]].cases, searchQuery);
      }
    }

    // Render remaining uncollected standalone cases
    for (var s = 0; s < standalone.length; s++) {
      if (!claimedCaseIds[standalone[s].case_id]) {
        appendCaseCard(standalone[s]);
      }
    }
  }

  function appendSequenceGroup(sequenceTitle, sequenceList, downloadedCases, searchQuery) {
    var totalParts = sequenceList.length;
    var downloadedCount = downloadedCases.length;
    var totalSize = 0;
    var downloadedIds = [];
    for (var i = 0; i < downloadedCases.length; i++) {
      totalSize += downloadedCases[i].assets.total_size_bytes;
      downloadedIds.push(downloadedCases[i].case_id);
    }
    var missingIds = [];
    for (var j = 0; j < sequenceList.length; j++) {
      if (downloadedIds.indexOf(sequenceList[j].id) === -1) {
        missingIds.push(sequenceList[j].id);
      }
    }

    var group = document.createElement("div");
    group.className = "sequence-group";

    // Header
    var header = document.createElement("div");
    header.className = "sequence-header";
    header.innerHTML =
      '<span class="sequence-header-toggle">&#9660;</span> ' +
      '<strong>' + escapeHtml(sequenceTitle) + '</strong>' +
      '<span class="sequence-meta">' +
        downloadedCount + '/' + totalParts + ' parts' +
        ' &middot; ' + formatBytes(totalSize) +
      '</span>';

    var seqPluginsBtn = document.createElement("button");
    seqPluginsBtn.className = "small-btn header-plugins-btn";
    seqPluginsBtn.textContent = "Plugins";
    seqPluginsBtn.title = "Configure plugin params for this sequence";
    seqPluginsBtn.addEventListener("click", (function (title) {
      return function (e) {
        e.stopPropagation();
        invoke("list_global_plugins").then(function (manifest) {
          var scripts = (manifest && manifest.scripts) || [];
          if (scripts.length === 0) {
            statusMsg.textContent = "No global plugins installed. Open the Plugins panel to add one.";
            pluginsPanel.classList.remove("hidden");
            pluginsToggle.classList.add("open");
            loadGlobalPluginsPanel();
            pluginsToggle.scrollIntoView({ behavior: "smooth" });
            return;
          }
          showScopedPluginModal("sequence", title, 'Sequence "' + title + '"');
        });
      };
    })(sequenceTitle));
    header.appendChild(seqPluginsBtn);

    var partsContainer = document.createElement("div");
    partsContainer.className = "sequence-parts";

    header.addEventListener("click", function () {
      var isOpen = !partsContainer.classList.contains("hidden");
      if (isOpen) {
        partsContainer.classList.add("hidden");
        header.querySelector(".sequence-header-toggle").innerHTML = "&#9654;";
      } else {
        partsContainer.classList.remove("hidden");
        header.querySelector(".sequence-header-toggle").innerHTML = "&#9660;";
      }
    });

    // Part rows
    var renderedParts = 0;
    for (var k = 0; k < sequenceList.length; k++) {
      var partInfo = sequenceList[k];

      // When searching, skip parts that don't match the query
      if (searchQuery) {
        var partTitle = (partInfo.title || "").toLowerCase();
        var partId = String(partInfo.id);
        if (partTitle.indexOf(searchQuery) === -1 && partId.indexOf(searchQuery) === -1) {
          continue;
        }
      }

      var downloaded = null;
      for (var d = 0; d < downloadedCases.length; d++) {
        if (downloadedCases[d].case_id === partInfo.id) {
          downloaded = downloadedCases[d];
          break;
        }
      }
      appendSequencePart(partsContainer, partInfo, k + 1, downloaded);
      renderedParts++;
    }

    // Don't render the group at all if search filtered out all parts
    if (searchQuery && renderedParts === 0) {
      return;
    }

    // Footer actions
    var footer = document.createElement("div");
    footer.className = "sequence-actions";

    // Play from Part 1 button
    if (downloadedCases.length > 0) {
      var firstCase = null;
      for (var f = 0; f < sequenceList.length; f++) {
        for (var fc = 0; fc < downloadedCases.length; fc++) {
          if (downloadedCases[fc].case_id === sequenceList[f].id) {
            firstCase = downloadedCases[fc];
            break;
          }
        }
        if (firstCase) break;
      }
      if (firstCase) {
        var playFirstBtn = document.createElement("button");
        playFirstBtn.className = "play-btn";
        playFirstBtn.innerHTML = "&#9654; Play from Part 1";
        playFirstBtn.addEventListener("click", (function (c) {
          return function () { playCase(c.case_id, c.title); };
        })(firstCase));
        footer.appendChild(playFirstBtn);
      }
    }

    // Continue (play from last save) button
    if (downloadedCases.length > 0) {
      var continueBtn = document.createElement("button");
      continueBtn.className = "play-btn continue-btn";
      continueBtn.innerHTML = "&#9654; Continue";
      continueBtn.title = "Resume from your most recent save across all parts";
      continueBtn.addEventListener("click", (function (seqList, dlCases) {
        return function () {
          statusMsg.textContent = "Checking saves...";
          findLastSequenceSave(seqList).then(function (lastSave) {
            if (!lastSave) {
              statusMsg.textContent = "No saves found for this sequence. Use 'Play from Part 1' to start.";
              return;
            }
            // Find the matching downloaded case for the title
            var matchTitle = "Part " + lastSave.partId;
            for (var mc = 0; mc < dlCases.length; mc++) {
              if (dlCases[mc].case_id === lastSave.partId) {
                matchTitle = dlCases[mc].title;
                break;
              }
            }
            statusMsg.textContent = "Resuming from save in \"" + matchTitle + "\"...";
            invoke("open_game", { caseId: lastSave.partId })
              .then(function (url) {
                // Append save_data to the URL
                var sep = url.indexOf("?") === -1 ? "?" : "&";
                var fullUrl = url + sep + "save_data=" + encodeURIComponent(lastSave.saveDataBase64);
                showPlayer(matchTitle, fullUrl);
              })
              .catch(function (e) {
                statusMsg.textContent = "Error: " + e;
              });
          });
        };
      })(sequenceList, downloadedCases));
      footer.appendChild(continueBtn);
    }

    // Download remaining button
    if (missingIds.length > 0) {
      var dlRemBtn = document.createElement("button");
      dlRemBtn.className = "update-btn";
      dlRemBtn.textContent = "Download " + missingIds.length + " remaining";
      dlRemBtn.addEventListener("click", (function (ids, title) {
        return function () {
          if (downloadInProgress) {
            statusMsg.textContent = "A download is already in progress.";
            return;
          }
          startSequenceDownload(ids, title);
        };
      })(missingIds, sequenceTitle));
      footer.appendChild(dlRemBtn);
    }

    // Update All button
    if (downloadedCases.length > 0) {
      var updateAllBtn = document.createElement("button");
      updateAllBtn.className = "update-btn";
      updateAllBtn.textContent = "Update All";
      updateAllBtn.addEventListener("click", (function (cases) {
        return function () {
          if (downloadInProgress) {
            statusMsg.textContent = "A download is already in progress.";
            return;
          }
          showUpdateModal(
            "Update all " + cases.length + " parts?",
            "Script/dialog only",
            "Re-download all assets",
            function (choice) {
              var redownload = (choice === 2);

              // Update each part sequentially
              var idx = 0;
              function updateNext() {
                if (idx >= cases.length) {
                  loadLibrary();
                  statusMsg.textContent = "All " + cases.length + " parts updated.";
                  return;
                }
                var c = cases[idx];
                idx++;
                statusMsg.textContent = "Updating part " + idx + "/" + cases.length + ": " + c.title + "...";
                startUpdate(c.case_id, redownload, function () {
                  updateNext();
                });
              }
              updateNext();
            }
          );
        };
      })(downloadedCases));
      footer.appendChild(updateAllBtn);
    }

    // Export sequence button
    if (downloadedCases.length > 1) {
      var exportSeqBtn = document.createElement("button");
      exportSeqBtn.className = "export-btn";
      exportSeqBtn.textContent = "Export Sequence";
      exportSeqBtn.addEventListener("click", (function (ids, title, list) {
        return function () {
          var safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
          var defaultName = safeName + ".aaocase";
          statusMsg.textContent = "Choosing export location...";
          invoke("pick_export_file", { defaultName: defaultName })
            .then(function (destPath) {
              if (!destPath) {
                statusMsg.textContent = "";
                return;
              }
              progressContainer.classList.remove("hidden");
              progressPhase.textContent = "Exporting sequence...";
              progressBarInner.style.width = "0%";
              progressText.textContent = "";

              var onEvent = new Channel();
              onEvent.onmessage = function (msg) {
                if (msg.event === "progress") {
                  var pct = Math.round((msg.data.completed / msg.data.total) * 100);
                  progressBarInner.style.width = pct + "%";
                  progressText.textContent =
                    msg.data.completed + " / " + msg.data.total + " files (" + pct + "%)";
                } else if (msg.event === "finished") {
                  progressBarInner.style.width = "100%";
                  progressPhase.textContent = "Export complete!";
                  progressText.textContent = formatBytes(msg.data.total_bytes);
                }
              };

              function doSeqExport(saves, includePlugins) {
                invoke("export_sequence", {
                  caseIds: ids,
                  sequenceTitle: title,
                  sequenceList: list,
                  destPath: destPath,
                  saves: saves,
                  includePlugins: includePlugins,
                  onEvent: onEvent
                }).then(function (size) {
                  var msg = 'Exported "' + title + '" (' + formatBytes(size) + ")";
                  if (saves) msg += " with saves";
                  statusMsg.textContent = msg;
                }).catch(function (e) {
                  console.error("[MAIN] export sequence error:", e);
                  statusMsg.textContent = "Export error: " + e;
                  progressContainer.classList.add("hidden");
                });
              }

              // Smart prompts
              var seqHasPlugins = false;
              for (var sp = 0; sp < downloadedCases.length; sp++) {
                if (downloadedCases[sp].has_plugins) { seqHasPlugins = true; break; }
              }

              invoke("read_saves_for_export", { caseIds: ids }).then(function (saves) {
                var hasSaves = saves !== null;
                if (!hasSaves && !seqHasPlugins) {
                  doSeqExport(null, true);
                } else if (hasSaves && !seqHasPlugins) {
                  showConfirmModal("Include saves?", "Include Saves",
                    function () { doSeqExport(saves, true); },
                    function () { doSeqExport(null, true); });
                } else if (!hasSaves && seqHasPlugins) {
                  showConfirmModal("Include plugins?", "Include Plugins",
                    function () { doSeqExport(null, true); },
                    function () { doSeqExport(null, false); });
                } else {
                  showExportOptionsModal(function (incSaves, incPlugins) {
                    doSeqExport(incSaves ? saves : null, incPlugins);
                  });
                }
              });
            });
        };
      })(downloadedIds, sequenceTitle, sequenceList));
      footer.appendChild(exportSeqBtn);
    }

    // Saves & Plugins button for entire sequence
    if (downloadedCases.length > 0) {
      var anyHasPlugins = false;
      for (var hp = 0; hp < downloadedCases.length; hp++) {
        if (downloadedCases[hp].has_plugins) { anyHasPlugins = true; break; }
      }
      var seqSavesBtn = document.createElement("button");
      seqSavesBtn.className = "save-btn";
      seqSavesBtn.textContent = "Saves";
      seqSavesBtn.title = "Saves & plugins for all parts";
      seqSavesBtn.addEventListener("click", (function (ids, title, hasPlug) {
        return function () { showSavesPluginsModal(ids, title, hasPlug); };
      })(downloadedIds, sequenceTitle, anyHasPlugins));
      footer.appendChild(seqSavesBtn);
    }

    // Delete all button
    var delAllBtn = document.createElement("button");
    delAllBtn.className = "delete-btn";
    delAllBtn.textContent = "Delete All";
    delAllBtn.addEventListener("click", (function (cases, title) {
      return function () {
        showConfirmModal(
          'Delete all ' + cases.length + ' parts of "' + title + '"?\nThis cannot be undone.',
          "Delete All",
          function () {
            var deletePromises = cases.map(function (c) {
              return invoke("delete_case", { caseId: c.case_id });
            });
            Promise.all(deletePromises)
              .then(function () { loadLibrary(); })
              .catch(function (e) { statusMsg.textContent = "Error deleting: " + e; });
          }
        );
      };
    })(downloadedCases, sequenceTitle));
    footer.appendChild(delAllBtn);

    group.appendChild(header);
    group.appendChild(partsContainer);
    group.appendChild(footer);
    caseList.appendChild(group);
  }

  function appendSequencePart(container, partInfo, partNum, manifest) {
    var row = document.createElement("div");
    row.className = "sequence-part" + (manifest ? "" : " sequence-part-missing");

    if (manifest) {
      var sizeStr = formatBytes(manifest.assets.total_size_bytes);
      var failedCount = manifest.failed_assets ? manifest.failed_assets.length : 0;
      row.innerHTML =
        '<span class="sequence-part-info">' +
          '<span class="sequence-part-num">' + partNum + '.</span> ' +
          escapeHtml(manifest.title) +
          '<span class="muted"> &middot; ' + manifest.assets.total_downloaded + ' assets (' + sizeStr + ')' +
          (failedCount > 0 ? ' &middot; <span class="case-failed">' + failedCount + ' failed</span>' : '') +
          (manifest.has_plugins ? ' &middot; <span class="case-plugins">Plugins</span>' : '') +
          '</span>' +
        '</span>';

      var actions = document.createElement("span");
      actions.className = "sequence-part-actions";

      var playBtn = document.createElement("button");
      playBtn.className = "play-btn";
      playBtn.innerHTML = "&#9654;";
      playBtn.title = "Play this part";
      playBtn.addEventListener("click", (function (c) {
        return function () { playCase(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(playBtn);

      var updatePartBtn = document.createElement("button");
      updatePartBtn.className = "update-btn";
      updatePartBtn.textContent = "Update";
      updatePartBtn.addEventListener("click", (function (c) {
        return function () { updateCase(c.case_id); };
      })(manifest));
      actions.appendChild(updatePartBtn);

      if (failedCount > 0) {
        var retryPartBtn = document.createElement("button");
        retryPartBtn.className = "retry-btn";
        retryPartBtn.textContent = "Retry (" + failedCount + ")";
        retryPartBtn.title = "Retry failed assets (likely dead links — may not help)";
        retryPartBtn.addEventListener("click", (function (c) {
          return function () { retryCase(c.case_id, c.failed_assets); };
        })(manifest));
        actions.appendChild(retryPartBtn);
      }

      var linkPartBtn = document.createElement("button");
      linkPartBtn.className = "link-btn";
      linkPartBtn.textContent = "Link";
      linkPartBtn.title = "Copy AAO link";
      linkPartBtn.addEventListener("click", (function (id) {
        return function () { copyTrialLink(id); };
      })(manifest.case_id));
      actions.appendChild(linkPartBtn);

      var exportBtn = document.createElement("button");
      exportBtn.className = "export-btn";
      exportBtn.textContent = "Export";
      exportBtn.addEventListener("click", (function (c) {
        return function () { exportCase(c.case_id, c.title, c.has_plugins); };
      })(manifest));
      actions.appendChild(exportBtn);

      var saveBtn = document.createElement("button");
      saveBtn.className = "save-btn";
      saveBtn.textContent = "Saves";
      saveBtn.title = "Saves & plugins";
      saveBtn.addEventListener("click", (function (c) {
        return function () { showSavesPluginsModal([c.case_id], c.title, c.has_plugins); };
      })(manifest));
      actions.appendChild(saveBtn);

      var pluginBtn = document.createElement("button");
      pluginBtn.className = "plugin-btn";
      pluginBtn.textContent = "Plugins";
      pluginBtn.title = "Manage plugins";
      pluginBtn.addEventListener("click", (function (c) {
        return function () { showPluginManagerModal(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(pluginBtn);

      var deleteBtn = document.createElement("button");
      deleteBtn.className = "delete-btn";
      deleteBtn.textContent = "Delete";
      deleteBtn.addEventListener("click", (function (c) {
        return function () { deleteCase(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(deleteBtn);

      row.appendChild(actions);
    } else {
      row.innerHTML =
        '<span class="sequence-part-info">' +
          '<span class="sequence-part-num">' + partNum + '.</span> ' +
          escapeHtml(partInfo.title || ("Case " + partInfo.id)) +
          '<span class="muted"> &middot; not downloaded</span>' +
        '</span>';
    }

    container.appendChild(row);
  }

  function appendCaseCard(c) {
    var card = document.createElement("div");
    card.className = "case-card";
    card.dataset.caseId = c.case_id;

    var sizeStr = formatBytes(c.assets.total_size_bytes);
    var assetCount = c.assets.total_downloaded;
    var dateStr = c.download_date ? formatDate(c.download_date) : "";
    var failedCount = c.failed_assets ? c.failed_assets.length : 0;

    card.innerHTML =
      '<div class="case-info">' +
        "<strong>" + escapeHtml(c.title) + "</strong>" +
        '<p class="case-meta">' +
          "by " + escapeHtml(c.author) +
          " &middot; " + escapeHtml(c.language.toUpperCase()) +
          " &middot; " + assetCount + " assets (" + sizeStr + ")" +
          (failedCount > 0 ? ' &middot; <span class="case-failed" style="cursor:pointer;text-decoration:underline" title="Click for details">' + failedCount + " failed</span>" : "") +
          (dateStr ? ' &middot; <span class="case-date">' + dateStr + "</span>" : "") +
          (c.has_plugins ? ' &middot; <span class="case-plugins">Plugins</span>' : "") +
        "</p>" +
      "</div>" +
      '<div class="case-actions">' +
        '<button class="play-btn">&#9654; Play</button>' +
        '<button class="case-continue-btn play-btn" title="Resume from last save">&#9654; Continue</button>' +
        '<button class="update-btn">Update</button>' +
        (failedCount > 0 ? '<button class="retry-btn" title="Retry only previously failed assets">Retry (' + failedCount + ')</button>' : "") +
        '<button class="link-btn" title="Copy AAO link">Link</button>' +
        '<button class="export-btn">Export</button>' +
        '<button class="save-btn" title="Saves &amp; plugins">Saves</button>' +
        '<button class="plugin-btn" title="Manage plugins">Plugins</button>' +
        '<button class="delete-btn">Delete</button>' +
      "</div>";

    card.querySelector(".play-btn").addEventListener("click", function () {
      playCase(c.case_id, c.title);
    });

    (function (caseId, caseTitle) {
      card.querySelector(".case-continue-btn").addEventListener("click", function () {
        statusMsg.textContent = "Checking saves...";
        findLastSequenceSave([{ id: caseId }]).then(function (lastSave) {
          if (!lastSave) {
            statusMsg.textContent = "No saves found for this case.";
            return;
          }
          statusMsg.textContent = 'Resuming "' + caseTitle + '"...';
          invoke("open_game", { caseId: caseId })
            .then(function (url) {
              var sep = url.indexOf("?") === -1 ? "?" : "&";
              var fullUrl = url + sep + "save_data=" + encodeURIComponent(lastSave.saveDataBase64);
              showPlayer(caseTitle, fullUrl);
            })
            .catch(function (e) { statusMsg.textContent = "Error: " + e; });
        });
      });
    })(c.case_id, c.title);

    card.querySelector(".update-btn").addEventListener("click", function () {
      updateCase(c.case_id);
    });

    var retryBtn = card.querySelector(".retry-btn");
    if (retryBtn) {
      retryBtn.addEventListener("click", function () {
        retryCase(c.case_id, c.failed_assets);
      });
    }

    var failedSpan = card.querySelector(".case-failed");
    if (failedSpan && c.failed_assets) {
      failedSpan.addEventListener("click", (function (fa) {
        return function (e) { e.stopPropagation(); showFailedAssetsModal(fa); };
      })(c.failed_assets));
    }

    card.querySelector(".link-btn").addEventListener("click", function () {
      copyTrialLink(c.case_id);
    });

    card.querySelector(".export-btn").addEventListener("click", function () {
      exportCase(c.case_id, c.title, c.has_plugins);
    });

    card.querySelector(".save-btn").addEventListener("click", function () {
      showSavesPluginsModal([c.case_id], c.title, c.has_plugins);
    });

    card.querySelector(".plugin-btn").addEventListener("click", function () {
      showPluginManagerModal(c.case_id, c.title);
    });

    card.querySelector(".delete-btn").addEventListener("click", function () {
      deleteCase(c.case_id, c.title);
    });

    caseList.appendChild(card);
  }

  // --- Collections ---

  function appendCollectionGroup(collection, allCases, sequenceGroups, searchQuery) {
    var items = collection.items || [];
    var itemCount = items.length;

    // Count total size across all items in the collection
    var totalSize = 0;
    var totalCases = 0;
    for (var i = 0; i < items.length; i++) {
      if (items[i].type === "sequence" && sequenceGroups[items[i].title]) {
        var seqCases = sequenceGroups[items[i].title].cases;
        for (var sc = 0; sc < seqCases.length; sc++) {
          totalSize += seqCases[sc].assets.total_size_bytes;
          totalCases++;
        }
      } else if (items[i].type === "case" && allCases[items[i].case_id]) {
        totalSize += allCases[items[i].case_id].assets.total_size_bytes;
        totalCases++;
      }
    }

    var group = document.createElement("div");
    group.className = "collection-group";

    // Header
    var header = document.createElement("div");
    header.className = "collection-header";
    header.innerHTML =
      '<span class="collection-header-toggle">&#9660;</span> ' +
      '<strong>' + escapeHtml(collection.title) + '</strong>' +
      '<span class="collection-meta">' +
        itemCount + ' item' + (itemCount !== 1 ? 's' : '') +
        ' &middot; ' + totalCases + ' case' + (totalCases !== 1 ? 's' : '') +
        ' &middot; ' + formatBytes(totalSize) +
      '</span>';

    var colPluginsBtn = document.createElement("button");
    colPluginsBtn.className = "small-btn header-plugins-btn";
    colPluginsBtn.textContent = "Plugins";
    colPluginsBtn.title = "Configure plugin params for this collection";
    colPluginsBtn.addEventListener("click", (function (col) {
      return function (e) {
        e.stopPropagation();
        invoke("list_global_plugins").then(function (manifest) {
          var scripts = (manifest && manifest.scripts) || [];
          if (scripts.length === 0) {
            statusMsg.textContent = "No global plugins installed. Open the Plugins panel to add one.";
            pluginsPanel.classList.remove("hidden");
            pluginsToggle.classList.add("open");
            loadGlobalPluginsPanel();
            pluginsToggle.scrollIntoView({ behavior: "smooth" });
            return;
          }
          showScopedPluginModal("collection", col.id, 'Collection "' + col.title + '"');
        });
      };
    })(collection));
    header.appendChild(colPluginsBtn);

    var itemsContainer = document.createElement("div");
    itemsContainer.className = "collection-items";

    header.addEventListener("click", function () {
      var isOpen = !itemsContainer.classList.contains("hidden");
      if (isOpen) {
        itemsContainer.classList.add("hidden");
        header.querySelector(".collection-header-toggle").innerHTML = "&#9654;";
      } else {
        itemsContainer.classList.remove("hidden");
        header.querySelector(".collection-header-toggle").innerHTML = "&#9660;";
      }
    });

    // Render each item in order
    var renderedItems = 0;
    for (var j = 0; j < items.length; j++) {
      var item = items[j];
      if (item.type === "sequence" && sequenceGroups[item.title]) {
        var sg = sequenceGroups[item.title];
        var beforeCount = itemsContainer.children.length;
        appendSequenceGroupInto(itemsContainer, item.title, sg.list, sg.cases, searchQuery);
        if (itemsContainer.children.length > beforeCount) renderedItems++;
      } else if (item.type === "case" && allCases[item.case_id]) {
        // When searching, skip cases that don't match
        if (searchQuery) {
          var caseData = allCases[item.case_id];
          var cTitle = (caseData.title || "").toLowerCase();
          var cAuthor = (caseData.author || "").toLowerCase();
          var cId = String(caseData.case_id);
          if (cTitle.indexOf(searchQuery) === -1 && cAuthor.indexOf(searchQuery) === -1 && cId.indexOf(searchQuery) === -1) {
            continue;
          }
        }
        appendCaseCardInto(itemsContainer, allCases[item.case_id]);
        renderedItems++;
      }
    }

    // Don't render the collection at all if search filtered out all items
    if (searchQuery && renderedItems === 0) {
      return;
    }

    // Footer actions
    var footer = document.createElement("div");
    footer.className = "collection-actions";

    // Play from Part 1 — play the first playable case across all items in order
    var firstPlayable = findFirstPlayableInCollection(items, allCases, sequenceGroups);
    if (firstPlayable) {
      var playFirstBtn = document.createElement("button");
      playFirstBtn.className = "play-btn";
      playFirstBtn.innerHTML = "&#9654; Play from Part 1";
      playFirstBtn.addEventListener("click", (function (c) {
        return function () { playCase(c.case_id, c.title); };
      })(firstPlayable));
      footer.appendChild(playFirstBtn);
    }

    // Continue button — find latest save across all cases in the collection
    var allCollectionCaseIds = getCollectionCaseIds(items, allCases, sequenceGroups);
    if (allCollectionCaseIds.length > 0) {
      var continueBtn = document.createElement("button");
      continueBtn.className = "play-btn continue-btn";
      continueBtn.innerHTML = "&#9654; Continue";
      continueBtn.title = "Resume from your most recent save across all cases in this collection";
      continueBtn.addEventListener("click", (function (caseIds, casesMap) {
        return function () {
          statusMsg.textContent = "Checking saves...";
          // Build a fake sequenceList from caseIds so we can reuse findLastSequenceSave
          var fakeList = caseIds.map(function (id) { return { id: id }; });
          findLastSequenceSave(fakeList).then(function (lastSave) {
            if (!lastSave) {
              statusMsg.textContent = "No saves found in this collection.";
              return;
            }
            var matchTitle = casesMap[lastSave.partId] ? casesMap[lastSave.partId].title : ("Case " + lastSave.partId);
            statusMsg.textContent = 'Resuming from save in "' + matchTitle + '"...';
            invoke("open_game", { caseId: lastSave.partId })
              .then(function (url) {
                var sep = url.indexOf("?") === -1 ? "?" : "&";
                var fullUrl = url + sep + "save_data=" + encodeURIComponent(lastSave.saveDataBase64);
                showPlayer(matchTitle, fullUrl);
              })
              .catch(function (e) { statusMsg.textContent = "Error: " + e; });
          });
        };
      })(allCollectionCaseIds, allCases));
      footer.appendChild(continueBtn);
    }

    // Edit button
    var editBtn = document.createElement("button");
    editBtn.className = "edit-collection-btn";
    editBtn.textContent = "Edit";
    editBtn.addEventListener("click", (function (col) {
      return function () { showEditCollectionModal(col); };
    })(collection));
    footer.appendChild(editBtn);

    // Export Collection button
    var exportColBtn = document.createElement("button");
    exportColBtn.className = "export-btn";
    exportColBtn.textContent = "Export Collection";
    var collHasPlugins = false;
    for (var chp = 0; chp < allCollectionCaseIds.length; chp++) {
      if (allCases[allCollectionCaseIds[chp]] && allCases[allCollectionCaseIds[chp]].has_plugins) {
        collHasPlugins = true; break;
      }
    }
    exportColBtn.addEventListener("click", (function (col, caseIds, hasPlug) {
      return function () { exportCollection(col, caseIds, hasPlug); };
    })(collection, allCollectionCaseIds, collHasPlugins));
    footer.appendChild(exportColBtn);

    // Delete button
    var delBtn = document.createElement("button");
    delBtn.className = "delete-btn";
    delBtn.textContent = "Delete Collection";
    delBtn.addEventListener("click", (function (col) {
      return function () {
        showConfirmModal(
          'Delete collection "' + col.title + '"?\nCases will not be deleted.',
          "Delete",
          function () {
            invoke("delete_collection", { id: col.id })
              .then(function () { loadLibrary(); })
              .catch(function (e) { statusMsg.textContent = "Error: " + e; });
          }
        );
      };
    })(collection));

    footer.appendChild(delBtn);

    group.appendChild(header);
    group.appendChild(itemsContainer);
    group.appendChild(footer);
    caseList.appendChild(group);
  }

  /**
   * Render a sequence group inside a container (used within collection items).
   * Reuses the same logic as appendSequenceGroup but appends to a given container.
   */
  function appendSequenceGroupInto(container, sequenceTitle, sequenceList, downloadedCases, searchQuery) {
    var totalParts = sequenceList.length;
    var downloadedCount = downloadedCases.length;
    var totalSize = 0;
    var downloadedIds = [];
    for (var i = 0; i < downloadedCases.length; i++) {
      totalSize += downloadedCases[i].assets.total_size_bytes;
      downloadedIds.push(downloadedCases[i].case_id);
    }
    var missingIds = [];
    for (var j = 0; j < sequenceList.length; j++) {
      if (downloadedIds.indexOf(sequenceList[j].id) === -1) {
        missingIds.push(sequenceList[j].id);
      }
    }

    var groupEl = document.createElement("div");
    groupEl.className = "sequence-group";

    var header = document.createElement("div");
    header.className = "sequence-header";
    header.innerHTML =
      '<span class="sequence-header-toggle">&#9660;</span> ' +
      '<strong>' + escapeHtml(sequenceTitle) + '</strong>' +
      '<span class="sequence-meta">' +
        downloadedCount + '/' + totalParts + ' parts' +
        ' &middot; ' + formatBytes(totalSize) +
      '</span>';

    var seqInPluginsBtn = document.createElement("button");
    seqInPluginsBtn.className = "small-btn header-plugins-btn";
    seqInPluginsBtn.textContent = "Plugins";
    seqInPluginsBtn.title = "Configure plugin params for this sequence";
    seqInPluginsBtn.addEventListener("click", (function (title) {
      return function (e) {
        e.stopPropagation();
        invoke("list_global_plugins").then(function (manifest) {
          var scripts = (manifest && manifest.scripts) || [];
          if (scripts.length === 0) {
            statusMsg.textContent = "No global plugins installed. Open the Plugins panel to add one.";
            pluginsPanel.classList.remove("hidden");
            pluginsToggle.classList.add("open");
            loadGlobalPluginsPanel();
            pluginsToggle.scrollIntoView({ behavior: "smooth" });
            return;
          }
          showScopedPluginModal("sequence", title, 'Sequence "' + title + '"');
        });
      };
    })(sequenceTitle));
    header.appendChild(seqInPluginsBtn);

    var partsContainer = document.createElement("div");
    partsContainer.className = "sequence-parts";

    header.addEventListener("click", function () {
      var isOpen = !partsContainer.classList.contains("hidden");
      if (isOpen) {
        partsContainer.classList.add("hidden");
        header.querySelector(".sequence-header-toggle").innerHTML = "&#9654;";
      } else {
        partsContainer.classList.remove("hidden");
        header.querySelector(".sequence-header-toggle").innerHTML = "&#9660;";
      }
    });

    var renderedParts = 0;
    for (var k = 0; k < sequenceList.length; k++) {
      var partInfo = sequenceList[k];

      // When searching, skip parts that don't match
      if (searchQuery) {
        var partTitle = (partInfo.title || "").toLowerCase();
        var partId = String(partInfo.id);
        if (partTitle.indexOf(searchQuery) === -1 && partId.indexOf(searchQuery) === -1) {
          continue;
        }
      }

      var downloaded = null;
      for (var d = 0; d < downloadedCases.length; d++) {
        if (downloadedCases[d].case_id === partInfo.id) {
          downloaded = downloadedCases[d];
          break;
        }
      }
      appendSequencePart(partsContainer, partInfo, k + 1, downloaded);
      renderedParts++;
    }

    // Don't render the group if search filtered out all parts
    if (searchQuery && renderedParts === 0) {
      return;
    }

    // Sequence-specific footer
    var seqFooter = document.createElement("div");
    seqFooter.className = "sequence-actions";

    if (downloadedCases.length > 0) {
      var firstCase = null;
      for (var f = 0; f < sequenceList.length; f++) {
        for (var fc = 0; fc < downloadedCases.length; fc++) {
          if (downloadedCases[fc].case_id === sequenceList[f].id) {
            firstCase = downloadedCases[fc];
            break;
          }
        }
        if (firstCase) break;
      }
      if (firstCase) {
        var playBtn = document.createElement("button");
        playBtn.className = "play-btn";
        playBtn.innerHTML = "&#9654; Play from Part 1";
        playBtn.addEventListener("click", (function (c) {
          return function () { playCase(c.case_id, c.title); };
        })(firstCase));
        seqFooter.appendChild(playBtn);
      }
    }

    // Continue (play from last save) button
    if (downloadedCases.length > 0) {
      var continueBtn = document.createElement("button");
      continueBtn.className = "play-btn continue-btn";
      continueBtn.innerHTML = "&#9654; Continue";
      continueBtn.title = "Resume from your most recent save across all parts";
      continueBtn.addEventListener("click", (function (seqList, dlCases) {
        return function () {
          statusMsg.textContent = "Checking saves...";
          findLastSequenceSave(seqList).then(function (lastSave) {
            if (!lastSave) {
              statusMsg.textContent = "No saves found for this sequence. Use 'Play from Part 1' to start.";
              return;
            }
            // Find the matching downloaded case for the title
            var matchTitle = "Part " + lastSave.partId;
            for (var mc = 0; mc < dlCases.length; mc++) {
              if (dlCases[mc].case_id === lastSave.partId) {
                matchTitle = dlCases[mc].title;
                break;
              }
            }
            statusMsg.textContent = "Resuming from save in \"" + matchTitle + "\"...";
            invoke("open_game", { caseId: lastSave.partId })
              .then(function (url) {
                // Append save_data to the URL
                var sep = url.indexOf("?") === -1 ? "?" : "&";
                var fullUrl = url + sep + "save_data=" + encodeURIComponent(lastSave.saveDataBase64);
                showPlayer(matchTitle, fullUrl);
              })
              .catch(function (e) {
                statusMsg.textContent = "Error: " + e;
              });
          });
        };
      })(sequenceList, downloadedCases));
      seqFooter.appendChild(continueBtn);
    }

    if (missingIds.length > 0) {
      var dlBtn = document.createElement("button");
      dlBtn.className = "update-btn";
      dlBtn.textContent = "Download " + missingIds.length + " remaining";
      dlBtn.addEventListener("click", (function (ids, title) {
        return function () {
          if (downloadInProgress) {
            statusMsg.textContent = "A download is already in progress.";
            return;
          }
          startSequenceDownload(ids, title);
        };
      })(missingIds, sequenceTitle));
      seqFooter.appendChild(dlBtn);
    }

    groupEl.appendChild(header);
    groupEl.appendChild(partsContainer);
    groupEl.appendChild(seqFooter);
    container.appendChild(groupEl);
  }

  /**
   * Render a case card inside a container (used within collection items).
   */
  function appendCaseCardInto(container, c) {
    var card = document.createElement("div");
    card.className = "case-card";
    card.dataset.caseId = c.case_id;

    var sizeStr = formatBytes(c.assets.total_size_bytes);
    var assetCount = c.assets.total_downloaded;
    var dateStr = c.download_date ? formatDate(c.download_date) : "";
    var failedCount = c.failed_assets ? c.failed_assets.length : 0;

    card.innerHTML =
      '<div class="case-info">' +
        "<strong>" + escapeHtml(c.title) + "</strong>" +
        '<p class="case-meta">' +
          "by " + escapeHtml(c.author) +
          " &middot; " + escapeHtml(c.language.toUpperCase()) +
          " &middot; " + assetCount + " assets (" + sizeStr + ")" +
          (failedCount > 0 ? ' &middot; <span class="case-failed" style="cursor:pointer;text-decoration:underline" title="Click for details">' + failedCount + " failed</span>" : "") +
          (dateStr ? ' &middot; <span class="case-date">' + dateStr + "</span>" : "") +
          (c.has_plugins ? ' &middot; <span class="case-plugins">Plugins</span>' : "") +
        "</p>" +
      "</div>" +
      '<div class="case-actions">' +
        '<button class="play-btn">&#9654; Play</button>' +
        '<button class="case-continue-btn play-btn" title="Resume from last save">&#9654; Continue</button>' +
        '<button class="update-btn">Update</button>' +
        (failedCount > 0 ? '<button class="retry-btn" title="Retry only previously failed assets">Retry (' + failedCount + ')</button>' : "") +
        '<button class="link-btn" title="Copy AAO link">Link</button>' +
        '<button class="export-btn">Export</button>' +
        '<button class="save-btn" title="Saves &amp; plugins">Saves</button>' +
        '<button class="plugin-btn" title="Manage plugins">Plugins</button>' +
        '<button class="delete-btn">Delete</button>' +
      "</div>";

    card.querySelector(".play-btn").addEventListener("click", function () {
      playCase(c.case_id, c.title);
    });
    (function (caseId, caseTitle) {
      card.querySelector(".case-continue-btn").addEventListener("click", function () {
        statusMsg.textContent = "Checking saves...";
        findLastSequenceSave([{ id: caseId }]).then(function (lastSave) {
          if (!lastSave) {
            statusMsg.textContent = "No saves found for this case.";
            return;
          }
          statusMsg.textContent = 'Resuming "' + caseTitle + '"...';
          invoke("open_game", { caseId: caseId })
            .then(function (url) {
              var sep = url.indexOf("?") === -1 ? "?" : "&";
              var fullUrl = url + sep + "save_data=" + encodeURIComponent(lastSave.saveDataBase64);
              showPlayer(caseTitle, fullUrl);
            })
            .catch(function (e) { statusMsg.textContent = "Error: " + e; });
        });
      });
    })(c.case_id, c.title);
    card.querySelector(".update-btn").addEventListener("click", function () {
      updateCase(c.case_id);
    });
    var retryBtn = card.querySelector(".retry-btn");
    if (retryBtn) {
      retryBtn.addEventListener("click", function () {
        retryCase(c.case_id, c.failed_assets);
      });
    }
    var failedSpan = card.querySelector(".case-failed");
    if (failedSpan && c.failed_assets) {
      failedSpan.addEventListener("click", (function (fa) {
        return function (e) { e.stopPropagation(); showFailedAssetsModal(fa); };
      })(c.failed_assets));
    }
    card.querySelector(".link-btn").addEventListener("click", function () {
      copyTrialLink(c.case_id);
    });
    card.querySelector(".export-btn").addEventListener("click", function () {
      exportCase(c.case_id, c.title, c.has_plugins);
    });
    card.querySelector(".save-btn").addEventListener("click", function () {
      showSavesPluginsModal([c.case_id], c.title, c.has_plugins);
    });
    card.querySelector(".plugin-btn").addEventListener("click", function () {
      showPluginManagerModal(c.case_id, c.title);
    });
    card.querySelector(".delete-btn").addEventListener("click", function () {
      deleteCase(c.case_id, c.title);
    });

    container.appendChild(card);
  }

  function findFirstPlayableInCollection(items, allCases, sequenceGroups) {
    for (var i = 0; i < items.length; i++) {
      var item = items[i];
      if (item.type === "sequence" && sequenceGroups[item.title]) {
        var sg = sequenceGroups[item.title];
        for (var f = 0; f < sg.list.length; f++) {
          for (var fc = 0; fc < sg.cases.length; fc++) {
            if (sg.cases[fc].case_id === sg.list[f].id) {
              return sg.cases[fc];
            }
          }
        }
      } else if (item.type === "case" && allCases[item.case_id]) {
        return allCases[item.case_id];
      }
    }
    return null;
  }

  function getCollectionCaseIds(items, allCases, sequenceGroups) {
    var ids = [];
    for (var i = 0; i < items.length; i++) {
      var item = items[i];
      if (item.type === "sequence" && sequenceGroups[item.title]) {
        var sg = sequenceGroups[item.title];
        for (var c = 0; c < sg.cases.length; c++) {
          ids.push(sg.cases[c].case_id);
        }
      } else if (item.type === "case" && allCases[item.case_id]) {
        ids.push(item.case_id);
      }
    }
    return ids;
  }

  function exportCollection(collection, caseIds, anyHasPlugins) {
    var safeName = collection.title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
    var defaultName = safeName + ".aaocase";
    statusMsg.textContent = "Choosing export location...";
    invoke("pick_export_file", { defaultName: defaultName })
      .then(function (destPath) {
        if (!destPath) {
          statusMsg.textContent = "";
          return;
        }
        progressContainer.classList.remove("hidden");
        progressPhase.textContent = "Exporting collection...";
        progressBarInner.style.width = "0%";
        progressText.textContent = "";

        var onEvent = new Channel();
        onEvent.onmessage = function (msg) {
          if (msg.event === "progress") {
            var pct = Math.round((msg.data.completed / msg.data.total) * 100);
            progressBarInner.style.width = pct + "%";
            progressText.textContent =
              msg.data.completed + " / " + msg.data.total + " files (" + pct + "%)";
          } else if (msg.event === "finished") {
            progressBarInner.style.width = "100%";
            progressPhase.textContent = "Export complete!";
            progressText.textContent = formatBytes(msg.data.total_bytes);
          }
        };

        function doCollExport(saves, includePlugins) {
          invoke("export_collection", {
            collectionId: collection.id,
            destPath: destPath,
            saves: saves,
            includePlugins: includePlugins,
            onEvent: onEvent
          }).then(function (size) {
            var msg = 'Exported collection "' + collection.title + '" (' + formatBytes(size) + ")";
            if (saves) msg += " with saves";
            statusMsg.textContent = msg;
          }).catch(function (e) {
            console.error("[MAIN] export collection error:", e);
            statusMsg.textContent = "Export error: " + e;
            progressContainer.classList.add("hidden");
          });
        }

        // Smart prompts
        invoke("read_saves_for_export", { caseIds: caseIds }).then(function (saves) {
          var hasSaves = saves !== null;
          if (!hasSaves && !anyHasPlugins) {
            doCollExport(null, true);
          } else if (hasSaves && !anyHasPlugins) {
            showConfirmModal("Include saves?", "Include Saves",
              function () { doCollExport(saves, true); },
              function () { doCollExport(null, true); });
          } else if (!hasSaves && anyHasPlugins) {
            showConfirmModal("Include plugins?", "Include Plugins",
              function () { doCollExport(null, true); },
              function () { doCollExport(null, false); });
          } else {
            showExportOptionsModal(function (incSaves, incPlugins) {
              doCollExport(incSaves ? saves : null, incPlugins);
            });
          }
        });
      })
      .catch(function (e) {
        console.error("[MAIN] export collection error:", e);
        statusMsg.textContent = "Export error: " + e;
        progressContainer.classList.add("hidden");
      });
  }

  // --- New Collection Modal ---

  function showNewCollectionModal() {
    // Gather all cases and sequence groups for the picker
    invoke("list_cases").then(function (cases) {
      invoke("list_collections").catch(function () { return []; }).then(function (collections) {
        // Build sequence groups
        var sequenceGroups = {};
        var standalone = [];
        for (var i = 0; i < cases.length; i++) {
          var c = cases[i];
          var seq = c.sequence;
          if (seq && seq.title && seq.list && seq.list.length > 1) {
            if (!sequenceGroups[seq.title]) {
              sequenceGroups[seq.title] = { list: seq.list, cases: [] };
            }
            sequenceGroups[seq.title].cases.push(c);
          } else {
            standalone.push(c);
          }
        }

        // Determine which items are already claimed
        var claimedCaseIds = {};
        var claimedSequenceTitles = {};
        for (var col = 0; col < collections.length; col++) {
          var items = collections[col].items || [];
          for (var it = 0; it < items.length; it++) {
            if (items[it].type === "case") claimedCaseIds[items[it].case_id] = true;
            else if (items[it].type === "sequence") claimedSequenceTitles[items[it].title] = true;
          }
        }

        showCollectionPickerModal(
          "New Collection",
          "",
          sequenceGroups,
          standalone,
          claimedCaseIds,
          claimedSequenceTitles,
          function (title, selectedItems) {
            invoke("create_collection", { title: title, items: selectedItems })
              .then(function () { loadLibrary(); })
              .catch(function (e) { statusMsg.textContent = "Error creating collection: " + e; });
          }
        );
      });
    });
  }

  function showCollectionPickerModal(modalTitle, existingTitle, sequenceGroups, standalone, claimedCaseIds, claimedSequenceTitles, onSave) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    // Title
    var titleField = document.createElement("div");
    titleField.className = "modal-field";
    var titleLabel = document.createElement("label");
    titleLabel.textContent = "Collection Title";
    var titleInput = document.createElement("input");
    titleInput.type = "text";
    titleInput.placeholder = "My Collection";
    titleInput.value = existingTitle;
    titleField.appendChild(titleLabel);
    titleField.appendChild(titleInput);

    // Picker
    var pickerField = document.createElement("div");
    pickerField.className = "modal-field";
    var pickerLabel = document.createElement("label");
    pickerLabel.textContent = "Select Items";
    var picker = document.createElement("div");
    picker.className = "collection-picker";

    // Build picker items
    var groupKeys = Object.keys(sequenceGroups);
    var checkboxes = []; // { checkbox, type, value }

    if (groupKeys.length > 0) {
      var seqLabel = document.createElement("div");
      seqLabel.className = "collection-picker-group-label";
      seqLabel.textContent = "Sequences";
      picker.appendChild(seqLabel);

      for (var g = 0; g < groupKeys.length; g++) {
        (function (seqTitle) {
          if (claimedSequenceTitles[seqTitle]) return;
          var sg = sequenceGroups[seqTitle];
          var row = document.createElement("label");
          row.className = "collection-picker-item";
          var cb = document.createElement("input");
          cb.type = "checkbox";
          var label = document.createElement("span");
          label.textContent = seqTitle;
          var meta = document.createElement("span");
          meta.className = "picker-item-meta";
          meta.textContent = sg.cases.length + "/" + sg.list.length + " parts";
          row.appendChild(cb);
          row.appendChild(label);
          row.appendChild(meta);
          picker.appendChild(row);
          checkboxes.push({ checkbox: cb, type: "sequence", value: seqTitle });
        })(groupKeys[g]);
      }
    }

    if (standalone.length > 0) {
      var caseLabel = document.createElement("div");
      caseLabel.className = "collection-picker-group-label";
      caseLabel.textContent = "Standalone Cases";
      picker.appendChild(caseLabel);

      for (var s = 0; s < standalone.length; s++) {
        (function (cs) {
          if (claimedCaseIds[cs.case_id]) return;
          var row = document.createElement("label");
          row.className = "collection-picker-item";
          var cb = document.createElement("input");
          cb.type = "checkbox";
          var label = document.createElement("span");
          label.textContent = cs.title;
          var meta = document.createElement("span");
          meta.className = "picker-item-meta";
          meta.textContent = "ID " + cs.case_id;
          row.appendChild(cb);
          row.appendChild(label);
          row.appendChild(meta);
          picker.appendChild(row);
          checkboxes.push({ checkbox: cb, type: "case", value: cs.case_id });
        })(standalone[s]);
      }
    }

    pickerField.appendChild(pickerLabel);
    pickerField.appendChild(picker);

    // Buttons
    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var createBtn = document.createElement("button");
    createBtn.className = "modal-btn modal-btn-secondary";
    createBtn.textContent = modalTitle === "New Collection" ? "Create" : "Save";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    createBtn.addEventListener("click", function () {
      var title = titleInput.value.trim();
      if (!title) {
        titleInput.style.borderColor = "#a33";
        titleInput.focus();
        return;
      }
      var selectedItems = [];
      for (var i = 0; i < checkboxes.length; i++) {
        if (checkboxes[i].checkbox.checked) {
          if (checkboxes[i].type === "sequence") {
            selectedItems.push({ type: "sequence", title: checkboxes[i].value });
          } else {
            selectedItems.push({ type: "case", case_id: checkboxes[i].value });
          }
        }
      }
      if (selectedItems.length === 0) {
        picker.style.borderColor = "#a33";
        return;
      }
      close();
      onSave(title, selectedItems);
    });

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(createBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(titleField);
    modal.appendChild(pickerField);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    titleInput.focus();
  }

  // --- Edit Collection Modal ---

  function showEditCollectionModal(collection) {
    // Fetch case titles for display
    invoke("list_cases").then(function (cases) {
      var caseTitles = {};
      for (var ci = 0; ci < cases.length; ci++) {
        caseTitles[cases[ci].case_id] = cases[ci].title;
      }
      _showEditCollectionModalInner(collection, caseTitles);
    });
  }

  function _showEditCollectionModalInner(collection, caseTitles) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    // Title field
    var titleField = document.createElement("div");
    titleField.className = "modal-field";
    var titleLabel = document.createElement("label");
    titleLabel.textContent = "Collection Title";
    var titleInput = document.createElement("input");
    titleInput.type = "text";
    titleInput.value = collection.title;
    titleField.appendChild(titleLabel);
    titleField.appendChild(titleInput);

    // Current items (reorderable)
    var itemsLabel = document.createElement("label");
    itemsLabel.textContent = "Items (drag to reorder)";
    itemsLabel.style.display = "block";
    itemsLabel.style.color = "#999";
    itemsLabel.style.fontSize = "0.82rem";
    itemsLabel.style.fontWeight = "500";
    itemsLabel.style.marginBottom = "0.35rem";
    itemsLabel.style.textTransform = "uppercase";
    itemsLabel.style.letterSpacing = "0.04em";

    var editItems = [];
    for (var i = 0; i < (collection.items || []).length; i++) {
      editItems.push({ type: collection.items[i].type, title: collection.items[i].title, case_id: collection.items[i].case_id });
    }

    var editListEl = document.createElement("div");
    editListEl.className = "collection-edit-list";

    function renderEditList() {
      editListEl.innerHTML = "";
      if (editItems.length === 0) {
        var empty = document.createElement("div");
        empty.className = "collection-edit-list-empty";
        empty.textContent = "No items. Add some below.";
        editListEl.appendChild(empty);
        return;
      }
      for (var i = 0; i < editItems.length; i++) {
        (function (idx) {
          var item = editItems[idx];
          var row = document.createElement("div");
          row.className = "collection-edit-item";
          row.draggable = true;
          row.dataset.index = idx;

          var handle = document.createElement("span");
          handle.className = "drag-handle";
          handle.textContent = "\u2630"; // ☰

          var label = document.createElement("span");
          label.className = "edit-item-label";
          label.textContent = item.type === "sequence" ? item.title : (caseTitles[item.case_id] || ("Case " + item.case_id));

          var typeTag = document.createElement("span");
          typeTag.className = "edit-item-type";
          typeTag.textContent = item.type;

          var removeBtn = document.createElement("button");
          removeBtn.className = "edit-item-remove";
          removeBtn.textContent = "\u2715"; // ✕
          removeBtn.title = "Remove from collection";
          removeBtn.addEventListener("click", function () {
            editItems.splice(idx, 1);
            renderEditList();
          });

          // DnD events
          row.addEventListener("dragstart", function (e) {
            e.dataTransfer.effectAllowed = "move";
            e.dataTransfer.setData("text/plain", String(idx));
          });

          row.addEventListener("dragover", function (e) {
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
            row.classList.add("drag-over");
          });

          row.addEventListener("dragleave", function () {
            row.classList.remove("drag-over");
          });

          row.addEventListener("drop", function (e) {
            e.preventDefault();
            row.classList.remove("drag-over");
            var fromIdx = parseInt(e.dataTransfer.getData("text/plain"), 10);
            var toIdx = idx;
            if (fromIdx === toIdx) return;
            var moved = editItems.splice(fromIdx, 1)[0];
            editItems.splice(toIdx, 0, moved);
            renderEditList();
          });

          row.appendChild(handle);
          row.appendChild(label);
          row.appendChild(typeTag);
          row.appendChild(removeBtn);
          editListEl.appendChild(row);
        })(i);
      }
    }

    renderEditList();

    // Add Items button
    var addItemsBtn = document.createElement("button");
    addItemsBtn.className = "modal-add-items-btn";
    addItemsBtn.textContent = "+ Add Items";
    addItemsBtn.addEventListener("click", function () {
      showAddItemsSubModal(editItems, collection.id, function (newItems) {
        for (var n = 0; n < newItems.length; n++) {
          editItems.push(newItems[n]);
        }
        renderEditList();
      });
    });

    // Buttons
    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var saveBtn = document.createElement("button");
    saveBtn.className = "modal-btn modal-btn-secondary";
    saveBtn.textContent = "Save";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    saveBtn.addEventListener("click", function () {
      var title = titleInput.value.trim();
      if (!title) {
        titleInput.style.borderColor = "#a33";
        titleInput.focus();
        return;
      }
      close();
      invoke("update_collection", { id: collection.id, title: title, items: editItems })
        .then(function () { loadLibrary(); })
        .catch(function (e) { statusMsg.textContent = "Error updating collection: " + e; });
    });

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(saveBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(titleField);
    modal.appendChild(itemsLabel);
    modal.appendChild(editListEl);
    modal.appendChild(addItemsBtn);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    titleInput.focus();
  }

  /**
   * Sub-modal for adding items to an existing collection being edited.
   * Shows uncollected items (not in any collection, and not in the current edit list).
   */
  function showAddItemsSubModal(currentEditItems, currentCollectionId, onAdd) {
    invoke("list_cases").then(function (cases) {
      invoke("list_collections").catch(function () { return []; }).then(function (collections) {
        var sequenceGroups = {};
        var standalone = [];
        for (var i = 0; i < cases.length; i++) {
          var c = cases[i];
          var seq = c.sequence;
          if (seq && seq.title && seq.list && seq.list.length > 1) {
            if (!sequenceGroups[seq.title]) {
              sequenceGroups[seq.title] = { list: seq.list, cases: [] };
            }
            sequenceGroups[seq.title].cases.push(c);
          } else {
            standalone.push(c);
          }
        }

        // Items claimed by OTHER collections (exclude current collection being edited)
        var claimedCaseIds = {};
        var claimedSequenceTitles = {};
        for (var col = 0; col < collections.length; col++) {
          if (collections[col].id === currentCollectionId) continue;
          var items = collections[col].items || [];
          for (var it = 0; it < items.length; it++) {
            if (items[it].type === "case") claimedCaseIds[items[it].case_id] = true;
            else if (items[it].type === "sequence") claimedSequenceTitles[items[it].title] = true;
          }
        }

        // Items already in the edit list
        for (var ei = 0; ei < currentEditItems.length; ei++) {
          if (currentEditItems[ei].type === "case") claimedCaseIds[currentEditItems[ei].case_id] = true;
          else if (currentEditItems[ei].type === "sequence") claimedSequenceTitles[currentEditItems[ei].title] = true;
        }

        var overlay = document.createElement("div");
        overlay.className = "modal-overlay";

        var modal = document.createElement("div");
        modal.className = "modal-dialog modal-dialog-wide";

        var heading = document.createElement("p");
        heading.className = "modal-message";
        heading.textContent = "Select items to add:";

        var picker = document.createElement("div");
        picker.className = "collection-picker";

        var groupKeys = Object.keys(sequenceGroups);
        var checkboxes = [];

        if (groupKeys.length > 0) {
          var seqLabel = document.createElement("div");
          seqLabel.className = "collection-picker-group-label";
          seqLabel.textContent = "Sequences";
          picker.appendChild(seqLabel);

          for (var g = 0; g < groupKeys.length; g++) {
            (function (seqTitle) {
              if (claimedSequenceTitles[seqTitle]) return;
              var sg = sequenceGroups[seqTitle];
              var row = document.createElement("label");
              row.className = "collection-picker-item";
              var cb = document.createElement("input");
              cb.type = "checkbox";
              var lbl = document.createElement("span");
              lbl.textContent = seqTitle;
              var meta = document.createElement("span");
              meta.className = "picker-item-meta";
              meta.textContent = sg.cases.length + "/" + sg.list.length + " parts";
              row.appendChild(cb);
              row.appendChild(lbl);
              row.appendChild(meta);
              picker.appendChild(row);
              checkboxes.push({ checkbox: cb, type: "sequence", value: seqTitle });
            })(groupKeys[g]);
          }
        }

        if (standalone.length > 0) {
          var caseLabel = document.createElement("div");
          caseLabel.className = "collection-picker-group-label";
          caseLabel.textContent = "Standalone Cases";
          picker.appendChild(caseLabel);

          for (var s = 0; s < standalone.length; s++) {
            (function (cs) {
              if (claimedCaseIds[cs.case_id]) return;
              var row = document.createElement("label");
              row.className = "collection-picker-item";
              var cb = document.createElement("input");
              cb.type = "checkbox";
              var lbl = document.createElement("span");
              lbl.textContent = cs.title;
              var meta = document.createElement("span");
              meta.className = "picker-item-meta";
              meta.textContent = "ID " + cs.case_id;
              row.appendChild(cb);
              row.appendChild(lbl);
              row.appendChild(meta);
              picker.appendChild(row);
              checkboxes.push({ checkbox: cb, type: "case", value: cs.case_id, title: cs.title });
            })(standalone[s]);
          }
        }

        var buttons = document.createElement("div");
        buttons.className = "modal-row-buttons";

        var addBtn = document.createElement("button");
        addBtn.className = "modal-btn modal-btn-secondary";
        addBtn.textContent = "Add Selected";

        var cancelBtn = document.createElement("button");
        cancelBtn.className = "modal-btn modal-btn-cancel";
        cancelBtn.textContent = "Cancel";

        function close() {
          document.body.removeChild(overlay);
        }

        addBtn.addEventListener("click", function () {
          var newItems = [];
          for (var i = 0; i < checkboxes.length; i++) {
            if (checkboxes[i].checkbox.checked) {
              if (checkboxes[i].type === "sequence") {
                newItems.push({ type: "sequence", title: checkboxes[i].value });
              } else {
                newItems.push({ type: "case", case_id: checkboxes[i].value, title: checkboxes[i].title });
              }
            }
          }
          close();
          if (newItems.length > 0) onAdd(newItems);
        });

        cancelBtn.addEventListener("click", close);
        overlay.addEventListener("click", function (e) {
          if (e.target === overlay) close();
        });

        buttons.appendChild(addBtn);
        buttons.appendChild(cancelBtn);

        modal.appendChild(heading);
        modal.appendChild(picker);
        modal.appendChild(buttons);
        overlay.appendChild(modal);
        document.body.appendChild(overlay);
      });
    });
  }

  // --- New Collection Button ---
  var newCollectionBtn = document.getElementById("new-collection-btn");
  if (newCollectionBtn) {
    newCollectionBtn.addEventListener("click", function () {
      showNewCollectionModal();
    });
  }

  function playCase(caseId, title) {
    console.log("[MAIN] playCase caseId=" + caseId + " title=" + title);
    statusMsg.textContent = "Loading...";
    invoke("open_game", { caseId: caseId })
      .then(function (url) {
        console.log("[MAIN] open_game returned url=" + url);
        showPlayer(title, url);
      })
      .catch(function (e) {
        console.error("[MAIN] open_game error:", e);
        statusMsg.textContent = "Error: " + e;
      });
  }

  function deleteCase(caseId, title) {
    showConfirmModal(
      'Delete "' + title + '"?\nThis cannot be undone.',
      "Delete",
      function () {
        invoke("delete_case", { caseId: caseId })
          .then(function () { loadLibrary(); })
          .catch(function (e) { statusMsg.textContent = "Error deleting case: " + e; });
      }
    );
  }

  function exportCase(caseId, title, hasPlugins) {
    console.log("[EXPORT] exportCase called, caseId=" + caseId + " title=" + title);
    var safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
    var defaultName = safeName + ".aaocase";
    statusMsg.textContent = "Choosing export location...";
    invoke("pick_export_file", { defaultName: defaultName })
      .then(function (destPath) {
        if (!destPath) { statusMsg.textContent = ""; return; }

        progressContainer.classList.remove("hidden");
        progressPhase.textContent = "Exporting...";
        progressBarInner.style.width = "0%";
        progressText.textContent = "";

        var onEvent = new Channel();
        onEvent.onmessage = function (msg) {
          if (msg.event === "progress") {
            var pct = Math.round((msg.data.completed / msg.data.total) * 100);
            progressBarInner.style.width = pct + "%";
            progressText.textContent = msg.data.completed + " / " + msg.data.total + " files (" + pct + "%)";
          } else if (msg.event === "finished") {
            progressBarInner.style.width = "100%";
            progressPhase.textContent = "Export complete!";
            progressText.textContent = formatBytes(msg.data.total_bytes);
          }
        };

        function doExport(saves, includePlugins) {
          invoke("export_case", {
            caseId: caseId,
            destPath: destPath,
            saves: saves,
            includePlugins: includePlugins,
            onEvent: onEvent
          }).then(function (size) {
            var msg = 'Exported "' + title + '" (' + formatBytes(size) + ")";
            if (saves) msg += " with saves";
            statusMsg.textContent = msg;
          }).catch(function (e) {
            console.error("[MAIN] export error:", e);
            statusMsg.textContent = "Export error: " + e;
            progressContainer.classList.add("hidden");
          });
        }

        // Smart prompts: only ask about what exists
        invoke("read_saves_for_export", { caseIds: [caseId] }).then(function (saves) {
          var hasSaves = saves !== null;
          if (!hasSaves && !hasPlugins) {
            doExport(null, true);
          } else if (hasSaves && !hasPlugins) {
            showConfirmModal("Include game saves?", "Include Saves",
              function () { doExport(saves, true); },
              function () { doExport(null, true); });
          } else if (!hasSaves && hasPlugins) {
            showConfirmModal("Include plugins?", "Include Plugins",
              function () { doExport(null, true); },
              function () { doExport(null, false); });
          } else {
            showExportOptionsModal(function (incSaves, incPlugins) {
              doExport(incSaves ? saves : null, incPlugins);
            });
          }
        });
      })
      .catch(function (e) {
        console.error("[MAIN] export error:", e);
        statusMsg.textContent = "Export error: " + e;
        progressContainer.classList.add("hidden");
      });
  }

  // --- Plugins ---

  function showPluginTargetModal(onConfirm) {
    invoke("list_cases").then(function (cases) {
      var overlay = document.createElement("div");
      overlay.className = "modal-overlay";

      var modal = document.createElement("div");
      modal.className = "modal-dialog modal-dialog-wide";

      var title = document.createElement("div");
      title.className = "modal-message";
      title.innerHTML = "<strong>Select cases to install plugin</strong>";

      var selectAllLabel = document.createElement("label");
      selectAllLabel.className = "collection-picker-item";
      var selectAllCb = document.createElement("input");
      selectAllCb.type = "checkbox";
      var selectAllText = document.createElement("span");
      selectAllText.textContent = "Select All";
      selectAllText.style.fontWeight = "600";
      selectAllLabel.appendChild(selectAllCb);
      selectAllLabel.appendChild(selectAllText);

      var picker = document.createElement("div");
      picker.className = "collection-picker";

      var checkboxes = [];

      // Group by sequence
      var sequenceGroups = {};
      var standalone = [];
      for (var i = 0; i < cases.length; i++) {
        var c = cases[i];
        var seq = c.sequence;
        if (seq && seq.title && seq.list && seq.list.length > 1) {
          if (!sequenceGroups[seq.title]) {
            sequenceGroups[seq.title] = [];
          }
          sequenceGroups[seq.title].push(c);
        } else {
          standalone.push(c);
        }
      }

      var groupKeys = Object.keys(sequenceGroups);
      if (groupKeys.length > 0) {
        var seqLabel = document.createElement("div");
        seqLabel.className = "collection-picker-group-label";
        seqLabel.textContent = "Sequences";
        picker.appendChild(seqLabel);

        for (var g = 0; g < groupKeys.length; g++) {
          var seqCases = sequenceGroups[groupKeys[g]];
          for (var sc = 0; sc < seqCases.length; sc++) {
            (function (cs) {
              var row = document.createElement("label");
              row.className = "collection-picker-item";
              var cb = document.createElement("input");
              cb.type = "checkbox";
              var label = document.createElement("span");
              label.textContent = cs.title;
              var meta = document.createElement("span");
              meta.className = "picker-item-meta";
              meta.textContent = "ID " + cs.case_id;
              row.appendChild(cb);
              row.appendChild(label);
              row.appendChild(meta);
              picker.appendChild(row);
              checkboxes.push({ checkbox: cb, caseId: cs.case_id });
            })(seqCases[sc]);
          }
        }
      }

      if (standalone.length > 0) {
        var caseLabel = document.createElement("div");
        caseLabel.className = "collection-picker-group-label";
        caseLabel.textContent = "Standalone Cases";
        picker.appendChild(caseLabel);

        for (var s = 0; s < standalone.length; s++) {
          (function (cs) {
            var row = document.createElement("label");
            row.className = "collection-picker-item";
            var cb = document.createElement("input");
            cb.type = "checkbox";
            var label = document.createElement("span");
            label.textContent = cs.title;
            var meta = document.createElement("span");
            meta.className = "picker-item-meta";
            meta.textContent = "ID " + cs.case_id;
            row.appendChild(cb);
            row.appendChild(label);
            row.appendChild(meta);
            picker.appendChild(row);
            checkboxes.push({ checkbox: cb, caseId: cs.case_id });
          })(standalone[s]);
        }
      }

      selectAllCb.addEventListener("change", function () {
        for (var j = 0; j < checkboxes.length; j++) {
          checkboxes[j].checkbox.checked = selectAllCb.checked;
        }
      });

      var buttons = document.createElement("div");
      buttons.className = "modal-row-buttons";

      var installBtn = document.createElement("button");
      installBtn.className = "modal-btn modal-btn-primary";
      installBtn.textContent = "Install";

      var cancelBtn = document.createElement("button");
      cancelBtn.className = "modal-btn modal-btn-cancel";
      cancelBtn.textContent = "Cancel";

      function close() {
        document.body.removeChild(overlay);
      }

      installBtn.addEventListener("click", function () {
        var selected = [];
        for (var j = 0; j < checkboxes.length; j++) {
          if (checkboxes[j].checkbox.checked) {
            selected.push(checkboxes[j].caseId);
          }
        }
        if (selected.length === 0) {
          picker.style.borderColor = "#a33";
          return;
        }
        close();
        onConfirm(selected);
      });

      cancelBtn.addEventListener("click", close);
      overlay.addEventListener("click", function (e) {
        if (e.target === overlay) close();
      });

      buttons.appendChild(installBtn);
      buttons.appendChild(cancelBtn);

      modal.appendChild(title);
      modal.appendChild(selectAllLabel);
      modal.appendChild(picker);
      modal.appendChild(buttons);
      overlay.appendChild(modal);
      document.body.appendChild(overlay);
    });
  }

  function doImportPlugin(pluginPath) {
    showPluginTargetModal(function (caseIds) {
      importResult.textContent = "";
      importResult.className = "";
      statusMsg.textContent = "Installing plugin to " + caseIds.length + " case(s)...";

      invoke("import_plugin", {
        sourcePath: pluginPath,
        targetCaseIds: caseIds
      })
      .then(function (importedIds) {
        importResult.innerHTML = "Plugin installed to <strong>" +
          importedIds.length + " case(s)</strong>";
        importResult.className = "result-success";
        statusMsg.textContent = "";
        loadLibrary();
      })
      .catch(function (e) {
        importResult.textContent = "Plugin import error: " + e;
        importResult.className = "result-error";
        statusMsg.textContent = "";
      });
    });
  }

  function showPluginManagerModal(caseId, caseTitle) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Plugins &mdash; " + escapeHtml(caseTitle) + "</strong>";

    var listContainer = document.createElement("div");
    listContainer.className = "plugin-list";

    var actionsRow = document.createElement("div");
    actionsRow.className = "plugin-actions-row";

    var importBtn = document.createElement("button");
    importBtn.className = "modal-btn modal-btn-primary";
    importBtn.textContent = "Import .aaoplug";

    var attachBtn = document.createElement("button");
    attachBtn.className = "modal-btn modal-btn-secondary";
    attachBtn.textContent = "Attach Code";

    actionsRow.appendChild(importBtn);
    actionsRow.appendChild(attachBtn);

    var closeBtn = document.createElement("button");
    closeBtn.className = "modal-btn modal-btn-cancel";
    closeBtn.textContent = "Close";
    closeBtn.style.width = "100%";

    function close() {
      document.body.removeChild(overlay);
      loadLibrary();
    }

    function refreshList() {
      invoke("list_plugins", { caseId: caseId })
        .then(function (manifest) {
          var scripts = (manifest && manifest.scripts) || [];
          var disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
          listContainer.innerHTML = "";
          if (scripts.length === 0) {
            var empty = document.createElement("div");
            empty.className = "plugin-list-empty";
            empty.textContent = "No plugins installed.";
            listContainer.appendChild(empty);
          } else {
            for (var i = 0; i < scripts.length; i++) {
              (function (filename) {
                var isDisabled = disabledList.indexOf(filename) !== -1;
                var item = document.createElement("div");
                item.className = "plugin-list-item" + (isDisabled ? " disabled" : "");

                var toggle = document.createElement("input");
                toggle.type = "checkbox";
                toggle.checked = !isDisabled;
                toggle.title = isDisabled ? "Enable plugin" : "Disable plugin";
                toggle.style.accentColor = "#4a90d9";
                toggle.style.width = "1rem";
                toggle.style.height = "1rem";
                toggle.style.flexShrink = "0";
                toggle.addEventListener("change", function () {
                  invoke("toggle_plugin", { caseId: caseId, filename: filename, enabled: toggle.checked })
                    .then(function () { refreshList(); })
                    .catch(function (e) { statusMsg.textContent = "Error toggling plugin: " + e; });
                });

                var name = document.createElement("span");
                name.className = "plugin-name";
                name.textContent = filename;

                var removeBtn = document.createElement("button");
                removeBtn.className = "plugin-remove-btn";
                removeBtn.textContent = "Remove";
                removeBtn.addEventListener("click", function () {
                  showConfirmModal(
                    "Remove plugin \"" + filename + "\"?",
                    "Remove",
                    function () {
                      invoke("remove_plugin", { caseId: caseId, filename: filename })
                        .then(function () { refreshList(); })
                        .catch(function (e) { statusMsg.textContent = "Error removing plugin: " + e; });
                    }
                  );
                });

                var paramsBtn = document.createElement("button");
                paramsBtn.className = "small-btn";
                paramsBtn.textContent = "Params";
                paramsBtn.style.cssText = "font-size:11px; padding:1px 6px; margin-left:auto;";
                paramsBtn.addEventListener("click", function () {
                  showPluginParamsModal(filename, "Case " + caseId, "by_case", String(caseId));
                });

                item.appendChild(toggle);
                item.appendChild(name);
                item.appendChild(paramsBtn);
                item.appendChild(removeBtn);
                listContainer.appendChild(item);
              })(scripts[i]);
            }
          }
        })
        .catch(function (e) {
          listContainer.innerHTML = "";
          var errEl = document.createElement("div");
          errEl.className = "plugin-list-empty";
          errEl.textContent = "Error loading plugins: " + e;
          listContainer.appendChild(errEl);
        });
    }

    importBtn.addEventListener("click", function () {
      invoke("pick_import_file")
        .then(function (selected) {
          if (!selected) return;
          if (!selected.toLowerCase().endsWith(".aaoplug")) {
            statusMsg.textContent = "Please select a .aaoplug file.";
            return;
          }
          statusMsg.textContent = "Installing plugin...";
          invoke("import_plugin", {
            sourcePath: selected,
            targetCaseIds: [caseId]
          })
          .then(function () {
            statusMsg.textContent = "Plugin installed.";
            refreshList();
          })
          .catch(function (e) {
            statusMsg.textContent = "Plugin import error: " + e;
          });
        })
        .catch(function (e) {
          statusMsg.textContent = "Could not open file picker: " + e;
        });
    });

    attachBtn.addEventListener("click", function () {
      showAttachCodeModal(caseId, caseTitle, refreshList);
    });

    closeBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    // Global plugins section
    var globalLabel = document.createElement("div");
    globalLabel.style.color = "#999";
    globalLabel.style.fontSize = "0.75rem";
    globalLabel.style.textTransform = "uppercase";
    globalLabel.style.letterSpacing = "0.04em";
    globalLabel.style.marginBottom = "0.35rem";
    globalLabel.textContent = "Global Plugins";

    var globalListContainer = document.createElement("div");
    globalListContainer.className = "plugin-list";

    function refreshGlobalList() {
      invoke("list_global_plugins")
        .then(function (manifest) {
          var scripts = (manifest && manifest.scripts) || [];
          var disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
          globalListContainer.innerHTML = "";
          if (scripts.length === 0) {
            var empty = document.createElement("div");
            empty.className = "plugin-list-empty";
            empty.textContent = "No global plugins.";
            globalListContainer.appendChild(empty);
          } else {
            for (var i = 0; i < scripts.length; i++) {
              (function (filename) {
                var isDisabled = disabledList.indexOf(filename) !== -1;
                var item = document.createElement("div");
                item.className = "plugin-list-item" + (isDisabled ? " disabled" : "");

                var toggle = document.createElement("input");
                toggle.type = "checkbox";
                toggle.checked = !isDisabled;
                toggle.style.accentColor = "#4a90d9";
                toggle.style.width = "1rem";
                toggle.style.height = "1rem";
                toggle.style.flexShrink = "0";
                toggle.addEventListener("change", function () {
                  invoke("toggle_global_plugin", { filename: filename, enabled: toggle.checked })
                    .then(function () { refreshGlobalList(); })
                    .catch(function (e) { statusMsg.textContent = "Error: " + e; });
                });

                var name = document.createElement("span");
                name.className = "plugin-name";
                name.textContent = filename;

                var removeBtn = document.createElement("button");
                removeBtn.className = "plugin-remove-btn";
                removeBtn.textContent = "Remove";
                removeBtn.addEventListener("click", function () {
                  showConfirmModal("Remove global plugin \"" + filename + "\"?", "Remove", function () {
                    invoke("remove_global_plugin", { filename: filename })
                      .then(function () { refreshGlobalList(); })
                      .catch(function (e) { statusMsg.textContent = "Error: " + e; });
                  });
                });

                item.appendChild(toggle);
                item.appendChild(name);
                item.appendChild(removeBtn);
                globalListContainer.appendChild(item);
              })(scripts[i]);
            }
          }
        });
    }

    var caseLabel = document.createElement("div");
    caseLabel.style.color = "#999";
    caseLabel.style.fontSize = "0.75rem";
    caseLabel.style.textTransform = "uppercase";
    caseLabel.style.letterSpacing = "0.04em";
    caseLabel.style.marginTop = "0.75rem";
    caseLabel.style.marginBottom = "0.35rem";
    caseLabel.textContent = "Case Plugins";

    modal.appendChild(titleEl);
    modal.appendChild(globalLabel);
    modal.appendChild(globalListContainer);
    modal.appendChild(caseLabel);
    modal.appendChild(listContainer);
    modal.appendChild(actionsRow);
    modal.appendChild(closeBtn);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    refreshGlobalList();
    refreshList();
  }

  // --- Plugin Params Editor Modal ---
  // Reusable for all cascade levels: global (default), collection, sequence, case.
  // Shows current param values as editable key-value pairs.
  // level: "default" | "by_collection" | "by_sequence" | "by_case"
  // key: collection_id, sequence_title, or case_id (string). Empty for default.
  // --- Scoped Plugin Modal ---
  // Shows all global plugins with per-scope enable/disable toggle + params button.
  // scopeType: "sequence" | "collection" | "case"
  // scopeKey: sequence title, collection ID, or case ID string
  // scopeLabel: display name like 'Sequence "My Seq"'
  function showScopedPluginModal(scopeType, scopeKey, scopeLabel) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Plugins &mdash; " + escapeHtml(scopeLabel) + "</strong>";

    var listContainer = document.createElement("div");
    listContainer.style.cssText = "margin: 10px 0; max-height: 350px; overflow-y: auto;";

    function close() { document.body.removeChild(overlay); }

    function refreshScopedList() {
      invoke("list_global_plugins").then(function (manifest) {
        var scripts = (manifest && manifest.scripts) || [];
        var plugins = (manifest && manifest.plugins) || {};
        var disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
        listContainer.innerHTML = "";

        if (scripts.length === 0) {
          var empty = document.createElement("div");
          empty.className = "muted";
          empty.textContent = "No global plugins installed.";
          listContainer.appendChild(empty);
          return;
        }

        for (var i = 0; i < scripts.length; i++) {
          (function (filename) {
            var globallyDisabled = disabledList.indexOf(filename) !== -1;
            var pluginEntry = plugins[filename] || {};

            // Determine effective state for this scope
            var isEnabledForScope = false;
            var stateLabel = "";
            if (globallyDisabled) {
              // Check enabled_for
              var ef = pluginEntry.enabled_for || {};
              var fieldName = scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections");
              var arr = ef[fieldName] || [];
              var matchVal = scopeType === "case" ? Number(scopeKey) : scopeKey;
              isEnabledForScope = false;
              for (var ei = 0; ei < arr.length; ei++) {
                if (scopeType === "case" ? arr[ei] === matchVal : arr[ei] === matchVal) {
                  isEnabledForScope = true;
                  break;
                }
              }
              stateLabel = isEnabledForScope ? "enabled (override)" : "disabled (global)";
            } else {
              // Check disabled_for
              var df = pluginEntry.disabled_for || {};
              var fieldName2 = scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections");
              var arr2 = df[fieldName2] || [];
              var matchVal2 = scopeType === "case" ? Number(scopeKey) : scopeKey;
              var isDisabledForScope = false;
              for (var di = 0; di < arr2.length; di++) {
                if (scopeType === "case" ? arr2[di] === matchVal2 : arr2[di] === matchVal2) {
                  isDisabledForScope = true;
                  break;
                }
              }
              isEnabledForScope = !isDisabledForScope;
              stateLabel = isDisabledForScope ? "disabled (override)" : "enabled (global)";
            }

            var row = document.createElement("div");
            row.className = "global-plugin-row";

            var toggle = document.createElement("input");
            toggle.type = "checkbox";
            toggle.checked = isEnabledForScope;
            toggle.style.accentColor = "#4a90d9";
            toggle.style.width = "1rem";
            toggle.style.height = "1rem";
            toggle.style.flexShrink = "0";
            toggle.addEventListener("change", function () {
              invoke("toggle_plugin_for_scope", {
                filename: filename,
                scopeType: scopeType,
                scopeKey: scopeKey,
                enabled: toggle.checked
              }).then(function () {
                refreshScopedList();
              }).catch(function (e) {
                statusMsg.textContent = "Error: " + e;
              });
            });

            var name = document.createElement("span");
            name.className = "plugin-name";
            name.textContent = filename;

            var badge = document.createElement("span");
            badge.className = "scope-badge";
            badge.textContent = stateLabel;

            var paramsBtn = document.createElement("button");
            paramsBtn.className = "small-btn";
            paramsBtn.textContent = "Params";
            paramsBtn.style.cssText = "font-size:0.72rem; padding:0.1rem 0.5rem;";
            paramsBtn.addEventListener("click", (function (fn) {
              return function () {
                showPluginParamsModal(fn, scopeLabel, "by_" + scopeType, scopeKey);
              };
            })(filename));

            row.appendChild(toggle);
            row.appendChild(name);
            row.appendChild(badge);
            row.appendChild(paramsBtn);
            listContainer.appendChild(row);
          })(scripts[i]);
        }
      });
    }

    var closeBtn = document.createElement("button");
    closeBtn.className = "modal-btn modal-btn-cancel";
    closeBtn.textContent = "Close";
    closeBtn.style.width = "100%";
    closeBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    modal.appendChild(titleEl);
    modal.appendChild(listContainer);
    modal.appendChild(closeBtn);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    refreshScopedList();
  }

  // --- Plugin Picker Modal ---
  function showPluginPickerModal(scripts, onSelect) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Select Plugin</strong>";

    var listEl = document.createElement("div");
    listEl.style.cssText = "display:flex; flex-direction:column; gap:6px; margin:10px 0;";

    for (var i = 0; i < scripts.length; i++) {
      (function (scriptName) {
        var btn = document.createElement("button");
        btn.className = "modal-btn modal-btn-secondary";
        btn.textContent = scriptName;
        btn.style.textAlign = "left";
        btn.addEventListener("click", function () {
          document.body.removeChild(overlay);
          onSelect(scriptName);
        });
        listEl.appendChild(btn);
      })(scripts[i]);
    }

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";
    cancelBtn.style.width = "100%";
    cancelBtn.addEventListener("click", function () {
      document.body.removeChild(overlay);
    });

    modal.appendChild(titleEl);
    modal.appendChild(listEl);
    modal.appendChild(cancelBtn);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);
  }

  function showPluginParamsModal(pluginFilename, levelLabel, level, key) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Plugin Params &mdash; " + escapeHtml(pluginFilename) + "</strong><br>" +
      "<small>Level: " + escapeHtml(levelLabel) + "</small>";

    var content = document.createElement("div");
    content.style.cssText = "margin: 10px 0; max-height: 300px; overflow-y: auto;";

    var loadingMsg = document.createElement("div");
    loadingMsg.textContent = "Loading...";
    loadingMsg.className = "muted";
    content.appendChild(loadingMsg);

    var paramsData = {};
    var descriptorsCache = null; // filled after loading

    function renderParams(params) {
      content.innerHTML = "";
      // Merge descriptor keys into display (show all known params, not just overridden ones)
      var allKeys = Object.keys(params);
      if (descriptorsCache && typeof descriptorsCache === "object") {
        var descKeys = Object.keys(descriptorsCache);
        for (var dk = 0; dk < descKeys.length; dk++) {
          if (allKeys.indexOf(descKeys[dk]) === -1) {
            allKeys.push(descKeys[dk]);
          }
        }
      }
      if (allKeys.length === 0) {
        var emptyMsg = document.createElement("div");
        emptyMsg.className = "muted";
        emptyMsg.textContent = "No params set at this level. Add new params below.";
        content.appendChild(emptyMsg);
      }
      for (var i = 0; i < allKeys.length; i++) {
        (function(paramKey) {
          var row = document.createElement("div");
          row.style.cssText = "display:flex; align-items:center; gap:6px; margin:4px 0;";

          var desc = descriptorsCache && descriptorsCache[paramKey] ? descriptorsCache[paramKey] : null;
          var val = params[paramKey];
          // If no value set but descriptor has a default, show the default
          if (val === undefined && desc && desc["default"] !== undefined) {
            val = desc["default"];
          }

          var keyLabel = document.createElement("span");
          keyLabel.style.cssText = "min-width:100px; font-size:13px; color:#ccc;";
          keyLabel.textContent = (desc && desc.label) ? desc.label : paramKey;
          keyLabel.title = paramKey;

          var input;
          var paramType = desc ? desc.type : null;

          if (paramType === "number" || (paramType === null && typeof val === "number")) {
            // Number with optional range
            if (desc && (desc.min !== undefined || desc.max !== undefined)) {
              input = document.createElement("input");
              input.type = "range";
              input.min = String(desc.min !== undefined ? desc.min : 0);
              input.max = String(desc.max !== undefined ? desc.max : 100);
              input.step = String(desc.step !== undefined ? desc.step : 1);
              input.value = String(val !== undefined ? val : 0);
              var valSpan = document.createElement("span");
              valSpan.style.cssText = "min-width:30px; font-size:12px; color:#aaa;";
              valSpan.textContent = " " + input.value;
              input.addEventListener("input", function() {
                valSpan.textContent = " " + input.value;
                paramsData[paramKey] = parseFloat(input.value);
              });
              row.appendChild(keyLabel);
              row.appendChild(input);
              row.appendChild(valSpan);
            } else {
              input = document.createElement("input");
              input.type = "number";
              input.value = String(val !== undefined ? val : 0);
              input.step = "any";
              input.style.cssText = "width:80px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
              input.addEventListener("input", function() { paramsData[paramKey] = parseFloat(input.value) || 0; });
              row.appendChild(keyLabel);
              row.appendChild(input);
            }
          } else if (paramType === "checkbox" || (paramType === null && typeof val === "boolean")) {
            input = document.createElement("input");
            input.type = "checkbox";
            input.checked = !!val;
            input.addEventListener("change", function() { paramsData[paramKey] = input.checked; });
            row.appendChild(keyLabel);
            row.appendChild(input);
          } else if (paramType === "select" && desc && desc.options) {
            input = document.createElement("select");
            input.style.cssText = "background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
            var opts = desc.options || [];
            for (var oi = 0; oi < opts.length; oi++) {
              var opt = document.createElement("option");
              if (typeof opts[oi] === "object") {
                opt.value = opts[oi].value;
                opt.textContent = opts[oi].label;
              } else {
                opt.value = String(opts[oi]);
                opt.textContent = String(opts[oi]);
              }
              input.appendChild(opt);
            }
            input.value = String(val !== undefined ? val : "");
            input.addEventListener("change", function() { paramsData[paramKey] = input.value; });
            row.appendChild(keyLabel);
            row.appendChild(input);
          } else {
            // Text (default fallback)
            input = document.createElement("input");
            input.type = "text";
            input.value = String(val !== undefined ? val : "");
            input.style.cssText = "width:120px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
            input.addEventListener("input", function() { paramsData[paramKey] = input.value; });
            row.appendChild(keyLabel);
            row.appendChild(input);
          }

          // Only show delete button for params actually overridden at this level
          if (params[paramKey] !== undefined) {
            var delBtn = document.createElement("button");
            delBtn.textContent = "x";
            delBtn.className = "small-btn danger-btn";
            delBtn.style.cssText = "padding:1px 6px; font-size:11px;";
            delBtn.addEventListener("click", function() {
              delete paramsData[paramKey];
              renderParams(paramsData);
            });
            row.appendChild(delBtn);
          }
          content.appendChild(row);
        })(allKeys[i]);
      }
    }

    // Add new param row
    var addRow = document.createElement("div");
    addRow.style.cssText = "display:flex; gap:6px; margin-top:8px;";
    var addKeyInput = document.createElement("input");
    addKeyInput.type = "text";
    addKeyInput.placeholder = "param name";
    addKeyInput.style.cssText = "width:100px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
    var addValInput = document.createElement("input");
    addValInput.type = "text";
    addValInput.placeholder = "value";
    addValInput.style.cssText = "width:80px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
    var addBtn = document.createElement("button");
    addBtn.className = "small-btn";
    addBtn.textContent = "+ Add";
    addBtn.addEventListener("click", function() {
      var k = addKeyInput.value.trim();
      if (!k) return;
      var v = addValInput.value.trim();
      // Auto-detect type
      if (v === "true") paramsData[k] = true;
      else if (v === "false") paramsData[k] = false;
      else if (v !== "" && !isNaN(Number(v))) paramsData[k] = Number(v);
      else paramsData[k] = v;
      addKeyInput.value = "";
      addValInput.value = "";
      renderParams(paramsData);
    });
    addRow.appendChild(addKeyInput);
    addRow.appendChild(addValInput);
    addRow.appendChild(addBtn);

    var btns = document.createElement("div");
    btns.className = "modal-buttons";

    var saveBtn = document.createElement("button");
    saveBtn.className = "modal-btn modal-btn-primary";
    saveBtn.textContent = "Save";
    saveBtn.addEventListener("click", function() {
      invoke("set_global_plugin_params", {
        filename: pluginFilename,
        level: level,
        key: key,
        params: paramsData
      }).then(function() {
        document.body.removeChild(overlay);
        statusMsg.textContent = "Plugin params saved for " + levelLabel + ".";
      }).catch(function(e) {
        statusMsg.textContent = "Error saving params: " + e;
      });
    });

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";
    cancelBtn.addEventListener("click", function() {
      document.body.removeChild(overlay);
    });

    btns.appendChild(saveBtn);
    btns.appendChild(cancelBtn);

    modal.appendChild(titleEl);
    modal.appendChild(content);
    modal.appendChild(addRow);
    modal.appendChild(btns);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    // Load descriptors + current params at this level
    Promise.all([
      invoke("get_plugin_descriptors", { filename: pluginFilename }).catch(function() { return null; }),
      invoke("get_plugin_params", { filename: pluginFilename })
    ]).then(function(results) {
      descriptorsCache = results[0]; // null if no descriptors
      var allParams = results[1];
      if (level === "default") {
        paramsData = (allParams && allParams["default"]) ? JSON.parse(JSON.stringify(allParams["default"])) : {};
      } else if (allParams && allParams[level] && allParams[level][key]) {
        paramsData = JSON.parse(JSON.stringify(allParams[level][key]));
      } else {
        paramsData = {};
      }
      renderParams(paramsData);
    }).catch(function(e) {
      content.innerHTML = "";
      var errMsg = document.createElement("div");
      errMsg.textContent = "Error loading params: " + e;
      errMsg.style.color = "#ff6b6b";
      content.appendChild(errMsg);
    });
  }

  function showAttachCodeModal(caseId, caseTitle, onDone) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Attach Plugin Code &mdash; " + escapeHtml(caseTitle) + "</strong>";

    var filenameField = document.createElement("div");
    filenameField.className = "modal-field";
    var filenameLabel = document.createElement("label");
    filenameLabel.textContent = "Filename";
    var filenameInput = document.createElement("input");
    filenameInput.type = "text";
    filenameInput.placeholder = "my_plugin.js";
    filenameField.appendChild(filenameLabel);
    filenameField.appendChild(filenameInput);

    var codeField = document.createElement("div");
    codeField.className = "modal-field";
    var codeLabel = document.createElement("label");
    codeLabel.textContent = "Plugin Code";
    var codeInput = document.createElement("textarea");
    codeInput.className = "attach-code-textarea";
    codeInput.placeholder = "// Paste your plugin JS code here...";
    codeField.appendChild(codeLabel);
    codeField.appendChild(codeInput);

    // Auto-detect plugin name from pasted code
    var userEditedFilename = false;
    filenameInput.addEventListener("input", function () {
      userEditedFilename = true;
    });

    function detectPluginName() {
      var code = codeInput.value;
      var nameMatch = code.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
      if (nameMatch) {
        var detected = nameMatch[1] + ".js";
        filenameInput.placeholder = detected;
        if (!userEditedFilename) {
          filenameInput.value = detected;
        }
      }
    }

    codeInput.addEventListener("input", detectPluginName);
    codeInput.addEventListener("paste", function () {
      setTimeout(detectPluginName, 0);
    });

    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var attachBtn = document.createElement("button");
    attachBtn.className = "modal-btn modal-btn-secondary";
    attachBtn.textContent = "Attach";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    attachBtn.addEventListener("click", function () {
      var filename = filenameInput.value.trim();
      if (!filename && filenameInput.placeholder && filenameInput.placeholder !== "my_plugin.js") {
        filename = filenameInput.placeholder;
      }
      var code = codeInput.value;

      if (!filename) {
        filenameInput.style.borderColor = "#a33";
        filenameInput.focus();
        return;
      }
      if (!filename.toLowerCase().endsWith(".js")) {
        filename = filename + ".js";
      }
      if (!code) {
        codeInput.style.borderColor = "#a33";
        codeInput.focus();
        return;
      }

      close();
      statusMsg.textContent = "Attaching plugin...";
      invoke("attach_plugin_code", {
        code: code,
        filename: filename,
        targetCaseIds: [caseId]
      })
      .then(function () {
        statusMsg.textContent = "Plugin \"" + filename + "\" attached.";
        if (onDone) onDone();
      })
      .catch(function (e) {
        statusMsg.textContent = "Error attaching plugin: " + e;
      });
    });

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(attachBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(titleEl);
    modal.appendChild(filenameField);
    modal.appendChild(codeField);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    filenameInput.focus();
  }

  // --- Saves ---

  function base64DecodeUtf8(str) {
    var raw = atob(str);
    var bytes = new Uint8Array(raw.length);
    for (var i = 0; i < raw.length; i++) {
      bytes[i] = raw.charCodeAt(i);
    }
    return new TextDecoder().decode(bytes);
  }

  function parsePastedSave(input) {
    var trimmed = input.trim();
    if (!trimmed) return null;

    var parsed = null;

    // 1. Try URL with save_data= parameter
    var saveDataMatch = trimmed.match(/[?&]save_data=([^&]+)/);
    if (saveDataMatch) {
      try {
        var decoded = decodeURIComponent(saveDataMatch[1]);
        var json = base64DecodeUtf8(decoded);
        parsed = JSON.parse(json);
      } catch (e) { /* not a valid URL save */ }
    }

    // 2. Try raw base64
    if (!parsed) {
      try {
        var json2 = base64DecodeUtf8(trimmed);
        parsed = JSON.parse(json2);
      } catch (e) { /* not base64 */ }
    }

    // 3. Try raw JSON
    if (!parsed) {
      try {
        parsed = JSON.parse(trimmed);
      } catch (e) { /* not JSON */ }
    }

    // Validate: must have numeric trial_id
    if (!parsed || typeof parsed !== "object" || typeof parsed.trial_id !== "number") {
      return null;
    }

    return {
      trialId: parsed.trial_id,
      saveString: JSON.stringify(parsed)
    };
  }

  function showPasteSaveModal() {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Import Save from Link or Code</strong>";

    var field = document.createElement("div");
    field.className = "modal-field";
    var label = document.createElement("label");
    label.textContent = "Paste a save link, base64 save code, or raw JSON";
    var textarea = document.createElement("textarea");
    textarea.className = "attach-code-textarea";
    textarea.rows = 4;
    textarea.placeholder = "https://aaonline.fr/player.php?trial_id=...&save_data=...\nor base64 code\nor raw JSON";
    field.appendChild(label);
    field.appendChild(textarea);

    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var importBtn = document.createElement("button");
    importBtn.className = "modal-btn modal-btn-primary";
    importBtn.textContent = "Import";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    importBtn.addEventListener("click", function () {
      var result = parsePastedSave(textarea.value);
      if (!result) {
        textarea.style.borderColor = "#a33";
        statusMsg.textContent = "Could not parse save data. Paste a save link, base64 code, or raw JSON.";
        return;
      }

      close();
      statusMsg.textContent = "Importing save for case " + result.trialId + "...";

      var savesObj = {};
      savesObj[String(result.trialId)] = {};
      savesObj[String(result.trialId)][String(Date.now())] = result.saveString;

      writeGameSaves(savesObj).then(function (writeResult) {
        if (writeResult && writeResult.success) {
          statusMsg.textContent = "Save imported for case " + result.trialId + " (" + writeResult.merged + " save merged).";
        } else {
          statusMsg.textContent = "Save imported for case " + result.trialId + ".";
        }
        loadLibrary();
      }).catch(function (e) {
        statusMsg.textContent = "Error importing save: " + e;
      });
    });

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(importBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(titleEl);
    modal.appendChild(field);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    textarea.focus();
  }

  function showSavesPluginsModal(caseIds, title, hasPlugins) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var titleEl = document.createElement("p");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Saves &amp; Plugins &mdash; " + escapeHtml(title) + "</strong>";

    var buttons = document.createElement("div");
    buttons.className = "modal-buttons";

    function close() { document.body.removeChild(overlay); }

    // Saves section label
    var savesLabel = document.createElement("div");
    savesLabel.style.cssText = "font-size:0.75rem;color:#999;text-transform:uppercase;letter-spacing:0.04em;margin-bottom:0.35rem;";
    savesLabel.textContent = "Saves";

    var exportSavesBtn = document.createElement("button");
    exportSavesBtn.className = "modal-btn modal-btn-primary";
    exportSavesBtn.textContent = "Export Saves";
    exportSavesBtn.addEventListener("click", function () { close(); exportSave(caseIds, title); });

    var importSavesBtn = document.createElement("button");
    importSavesBtn.className = "modal-btn modal-btn-secondary";
    importSavesBtn.textContent = "Import Saves";
    importSavesBtn.addEventListener("click", function () {
      close();
      invoke("pick_import_file").then(function (selected) {
        if (!selected) return;
        if (selected.toLowerCase().endsWith(".aaosave")) {
          doImportSave(selected);
        } else {
          statusMsg.textContent = "Please select a .aaosave file.";
        }
      });
    });

    var pasteSaveBtn = document.createElement("button");
    pasteSaveBtn.className = "modal-btn";
    pasteSaveBtn.textContent = "Paste Save Link/Code";
    pasteSaveBtn.addEventListener("click", function () { close(); showPasteSaveModal(); });

    buttons.appendChild(exportSavesBtn);
    buttons.appendChild(importSavesBtn);
    buttons.appendChild(pasteSaveBtn);

    // Plugins section (only if has plugins)
    if (hasPlugins) {
      var pluginsLabel = document.createElement("div");
      pluginsLabel.style.cssText = "font-size:0.75rem;color:#999;text-transform:uppercase;letter-spacing:0.04em;margin-top:0.75rem;margin-bottom:0.35rem;";
      pluginsLabel.textContent = "Plugins";

      var exportPluginsBtn = document.createElement("button");
      exportPluginsBtn.className = "modal-btn";
      exportPluginsBtn.textContent = "Export Plugins";
      exportPluginsBtn.addEventListener("click", function () {
        close();
        var safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
        invoke("pick_export_plugin_file", { defaultName: safeName + ".aaoplug" }).then(function (destPath) {
          if (!destPath) return;
          statusMsg.textContent = "Exporting plugins...";
          var caseId = Array.isArray(caseIds) ? caseIds[0] : caseIds;
          invoke("export_case_plugins", { caseId: caseId, destPath: destPath }).then(function (size) {
            statusMsg.textContent = "Exported plugins (" + formatBytes(size) + ")";
          }).catch(function (e) {
            statusMsg.textContent = "Export error: " + e;
          });
        });
      });

      buttons.appendChild(pluginsLabel);
      buttons.appendChild(exportPluginsBtn);
    }

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";
    cancelBtn.addEventListener("click", close);

    buttons.appendChild(cancelBtn);

    overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });

    modal.appendChild(titleEl);
    modal.appendChild(savesLabel);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);
  }

  function showExportOptionsModal(onConfirm) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var msg = document.createElement("p");
    msg.className = "modal-message";
    msg.textContent = "What to include in the export?";

    var savesLabel = document.createElement("label");
    savesLabel.className = "regular_label";
    savesLabel.style.cssText = "display:flex;align-items:center;gap:0.5rem;padding:0.4rem 0;cursor:pointer;color:#ccc;font-size:0.9rem;";
    var savesCb = document.createElement("input");
    savesCb.type = "checkbox";
    savesCb.checked = true;
    savesLabel.appendChild(savesCb);
    savesLabel.appendChild(document.createTextNode(" Include saves"));

    var pluginsLabel = document.createElement("label");
    pluginsLabel.className = "regular_label";
    pluginsLabel.style.cssText = "display:flex;align-items:center;gap:0.5rem;padding:0.4rem 0;cursor:pointer;color:#ccc;font-size:0.9rem;";
    var pluginsCb = document.createElement("input");
    pluginsCb.type = "checkbox";
    pluginsCb.checked = true;
    pluginsLabel.appendChild(pluginsCb);
    pluginsLabel.appendChild(document.createTextNode(" Include plugins"));

    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var exportBtn = document.createElement("button");
    exportBtn.className = "modal-btn modal-btn-primary";
    exportBtn.textContent = "Export";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() { document.body.removeChild(overlay); }

    exportBtn.addEventListener("click", function () {
      close();
      onConfirm(savesCb.checked, pluginsCb.checked);
    });
    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });

    buttons.appendChild(exportBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(msg);
    modal.appendChild(savesLabel);
    modal.appendChild(pluginsLabel);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);
  }

  function exportSave(caseIds, title) {
    if (!Array.isArray(caseIds)) {
      caseIds = [caseIds];
    }
    statusMsg.textContent = "Reading saves...";
    invoke("read_saves_for_export", { caseIds: caseIds }).then(function (saves) {
      if (!saves) {
        statusMsg.textContent = "No saves found for " + (caseIds.length > 1 ? "these cases" : "this case") + ".";
        return;
      }

      showConfirmModal(
        "Include plugins in save export?",
        "Include Plugins",
        function () { doExportSave(caseIds, title, saves, true); },
        function () { doExportSave(caseIds, title, saves, false); }
      );
    });
  }

  function doExportSave(caseIds, title, saves, includePlugins) {
      var safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
      var defaultName = safeName + ".aaosave";
      statusMsg.textContent = "Choosing export location...";

      invoke("pick_export_save_file", { defaultName: defaultName })
        .then(function (destPath) {
          if (!destPath) {
            statusMsg.textContent = "";
            return;
          }
          statusMsg.textContent = "Exporting saves...";
          invoke("export_save", {
            caseIds: caseIds,
            saves: saves,
            includePlugins: includePlugins,
            destPath: destPath
          })
          .then(function (size) {
            statusMsg.textContent = 'Exported saves (' + formatBytes(size) + ')';
          })
          .catch(function (e) {
            statusMsg.textContent = "Export error: " + e;
          });
        })
        .catch(function (e) {
          statusMsg.textContent = "Could not open save dialog: " + e;
        });
  }

  function doImportSave(path) {
    importResult.textContent = "";
    importResult.className = "";
    statusMsg.textContent = "Importing saves...";

    invoke("import_save", { sourcePath: path })
      .then(function (result) {
        var savesPromise = writeGameSaves(result.saves).then(function (writeResult) {
          var mergedCount = (writeResult && writeResult.merged) || 0;

          var cases = (result.metadata && result.metadata.cases) || [];
          var totalSaves = 0;
          for (var i = 0; i < cases.length; i++) {
            totalSaves += (cases[i].save_count || 0);
          }

          var msg = "Imported " + totalSaves + " save(s) for " + cases.length + " case(s)";
          if (mergedCount > 0) msg += " (" + mergedCount + " new)";
          if (result.plugins_installed && result.plugins_installed.length > 0) {
            msg += ", plugins installed for " + result.plugins_installed.length + " case(s)";
          }
          importResult.innerHTML = msg;
          importResult.className = "result-success";
          statusMsg.textContent = "";
          loadLibrary();
        });
      })
      .catch(function (e) {
        importResult.textContent = "Save import error: " + e;
        importResult.className = "result-error";
        statusMsg.textContent = "";
      });
  }

  // --- Download ---

  caseIdInput.addEventListener("keydown", function (e) {
    if (e.key === "Enter") {
      downloadBtn.click();
    }
  });

  downloadBtn.addEventListener("click", function () {
    var caseId = parseCaseId(caseIdInput.value);
    if (!caseId) {
      downloadResult.textContent =
        "Please enter a valid case ID or AAO URL.";
      downloadResult.className = "result-error";
      return;
    }

    if (downloadInProgress) {
      statusMsg.textContent = "A download is already in progress.";
      return;
    }

    function proceedWithDownload() {
      // First fetch case info to check for sequence
      downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Checking for sequence...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    invoke("fetch_case_info", { caseId: caseId })
      .then(function (caseInfo) {
        var seq = caseInfo.sequence;
        if (seq && seq.list && seq.list.length > 1) {
          // This case is part of a sequence
          progressContainer.classList.add("hidden");
          var partNames = seq.list.map(function (p) { return p.title || ("Case " + p.id); });
          var msg = 'This case is part of "' + (seq.title || "Untitled Sequence") + '" (' +
            seq.list.length + " parts):\n\n";
          for (var i = 0; i < partNames.length; i++) {
            var partId = seq.list[i].id;
            var alreadyDl = knownCaseIds.indexOf(partId) !== -1;
            msg += (i + 1) + ". " + partNames[i] + (alreadyDl ? " (already downloaded)" : "") + "\n";
          }
          var allIds = seq.list.map(function (p) { return p.id; });
          var seqTitle = seq.title || "Untitled Sequence";
          var seqOverlay = document.createElement("div");
          seqOverlay.className = "modal-overlay";
          var seqModal = document.createElement("div");
          seqModal.className = "modal-dialog";
          var seqMsg = document.createElement("p");
          seqMsg.className = "modal-message";
          seqMsg.style.whiteSpace = "pre-wrap";
          seqMsg.textContent = msg;
          var seqBtns = document.createElement("div");
          seqBtns.className = "modal-buttons";
          var seqAllBtn = document.createElement("button");
          seqAllBtn.className = "modal-btn modal-btn-primary";
          seqAllBtn.textContent = "Download All Parts";
          var seqOneBtn = document.createElement("button");
          seqOneBtn.className = "modal-btn modal-btn-secondary";
          seqOneBtn.textContent = "This Case Only";
          var seqCancelBtn = document.createElement("button");
          seqCancelBtn.className = "modal-btn modal-btn-cancel";
          seqCancelBtn.textContent = "Cancel";
          function closeSeqModal() {
            document.body.removeChild(seqOverlay);
          }
          function cancelSeqModal() {
            closeSeqModal();
            downloadBtn.disabled = false;
            caseIdInput.disabled = false;
          }
          seqAllBtn.addEventListener("click", function () {
            closeSeqModal();
            startSequenceDownload(allIds, seqTitle);
          });
          seqOneBtn.addEventListener("click", function () {
            closeSeqModal();
            startDownload(caseId);
          });
          seqCancelBtn.addEventListener("click", cancelSeqModal);
          seqOverlay.addEventListener("click", function (e) {
            if (e.target === seqOverlay) cancelSeqModal();
          });
          seqBtns.appendChild(seqAllBtn);
          seqBtns.appendChild(seqOneBtn);
          seqBtns.appendChild(seqCancelBtn);
          seqModal.appendChild(seqMsg);
          seqModal.appendChild(seqBtns);
          seqOverlay.appendChild(seqModal);
          document.body.appendChild(seqOverlay);
        } else {
          // No sequence, download single case
          progressContainer.classList.add("hidden");
          startDownload(caseId);
        }
      })
      .catch(function (e) {
        // If fetch_case_info fails, fall back to direct download
        console.warn("[DOWNLOAD] fetch_case_info failed, falling back to direct download:", e);
        progressContainer.classList.add("hidden");
        startDownload(caseId);
      });
    }

    // Duplicate check (for single case)
    if (knownCaseIds.indexOf(caseId) !== -1) {
      showConfirmModal(
        "Case " + caseId + " is already in your library.\nDownload again? (This will overwrite it.)",
        "Download Again",
        proceedWithDownload
      );
    } else {
      proceedWithDownload();
    }
  });

  function updateCase(caseId) {
    if (downloadInProgress) {
      statusMsg.textContent = "A download is already in progress.";
      return;
    }

    showUpdateModal(
      "What was updated for this case?",
      "Script/dialog only",
      "Re-download all assets",
      function (choice) {
        startUpdate(caseId, choice === 2);
      }
    );
  }

  function startUpdate(caseId, redownloadAssets, onDone) {
    console.log("[UPDATE] startUpdate caseId=" + caseId + " redownloadAssets=" + redownloadAssets);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    // Show progress
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = redownloadAssets
      ? "Updating case (full)..."
      : "Updating case (script only)...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      console.log("[UPDATE EVENT]", JSON.stringify(msg));
      if (msg.event === "started") {
        progressPhase.textContent = "Downloading " + msg.data.total + " assets...";
        progressText.textContent = "0 / " + msg.data.total;
      } else if (msg.event === "progress") {
        var pct = Math.round((msg.data.completed / msg.data.total) * 100);
        progressBarInner.style.width = pct + "%";
        progressText.textContent =
          msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur();
        }
        if (msg.data.elapsed_ms > 1000 && msg.data.bytes_downloaded > 0) {
          var speed = msg.data.bytes_downloaded / (msg.data.elapsed_ms / 1000);
          progressText.textContent += " — " + formatBytes(speed) + "/s";
          if (msg.data.completed > 0 && msg.data.completed < msg.data.total) {
            var etaMs = (msg.data.total - msg.data.completed) * (msg.data.elapsed_ms / msg.data.completed);
            progressText.textContent += " — ~" + formatDuration(etaMs) + " left";
          }
        }
      } else if (msg.event === "finished") {
        var sizeStr = formatBytes(msg.data.total_bytes);
        progressBarInner.style.width = "100%";
        progressPhase.textContent = "Update complete!";
        removeSpoilerBlur();
        progressText.textContent =
          msg.data.downloaded + " downloaded" +
          (msg.data.failed > 0 ? ", " + msg.data.failed + " failed" : "") +
          " (" + sizeStr + ")";
      } else if (msg.event === "error") {
        progressPhase.textContent = "Error";
        progressText.textContent = msg.data.message;
      }
    };

    invoke("update_case", {
      caseId: caseId,
      redownloadAssets: redownloadAssets,
      onEvent: onEvent
    })
      .then(function (manifest) {
        console.log("[UPDATE] update_case success:", JSON.stringify({
          case_id: manifest.case_id,
          title: manifest.title,
          total_downloaded: manifest.assets.total_downloaded,
          failed_count: manifest.failed_assets ? manifest.failed_assets.length : 0
        }));
        downloadResult.innerHTML =
          '<strong>' + escapeHtml(manifest.title) + '</strong> updated!';
        downloadResult.className = "result-success";
        loadLibrary();
      })
      .catch(function (e) {
        console.error("[UPDATE] update_case error:", e);
        downloadResult.textContent = "Update error: " + e;
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        caseIdInput.disabled = false;
        cancelDownloadBtn.classList.add("hidden");
        if (onDone) {
          onDone();
        } else {
          setTimeout(function () {
            progressContainer.classList.add("hidden");
          }, 4000);
        }
        processQueue();
      });
  }

  function retryCase(caseId, failedAssets) {
    if (downloadInProgress) {
      statusMsg.textContent = "A download is already in progress.";
      return;
    }

    var aaoCount = 0;
    var externalCount = 0;
    if (failedAssets) {
      for (var i = 0; i < failedAssets.length; i++) {
        if (failedAssets[i].url && failedAssets[i].url.indexOf("aaonline.fr") !== -1) {
          aaoCount++;
        } else {
          externalCount++;
        }
      }
    }

    var msg = "Retry " + (failedAssets ? failedAssets.length : "") + " failed assets?\n\n";
    if (aaoCount > 0 && externalCount > 0) {
      msg += aaoCount + " failed from aaonline.fr — the site was probably temporarily down. " +
        "Retrying these should work.\n\n" +
        externalCount + " failed from external hosting — these are likely dead links " +
        "(the author's hosting is down or the files were removed). " +
        "Retrying will most likely fail again.\n\n";
    } else if (aaoCount > 0) {
      msg += "All failed assets are from aaonline.fr — the site was probably temporarily down. " +
        "Retrying should work.\n\n";
    } else {
      msg += "Failed assets are from external hosting — these are likely dead links " +
        "(the author's hosting is down or the files were removed). " +
        "Retrying will most likely fail again.\n\n";
    }
    msg += "Only previously failed assets will be retried — nothing already downloaded " +
      "will be re-downloaded.";

    showConfirmModal(msg, "Retry", function () {
    console.log("[RETRY] retryCase caseId=" + caseId);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    // Show progress
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Retrying failed assets...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      console.log("[RETRY EVENT]", JSON.stringify(msg));
      if (msg.event === "started") {
        progressPhase.textContent = "Retrying " + msg.data.total + " assets...";
        progressText.textContent = "0 / " + msg.data.total;
      } else if (msg.event === "progress") {
        var pct = Math.round((msg.data.completed / msg.data.total) * 100);
        progressBarInner.style.width = pct + "%";
        progressText.textContent =
          msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur();
        }
        if (msg.data.elapsed_ms > 1000 && msg.data.bytes_downloaded > 0) {
          var speed = msg.data.bytes_downloaded / (msg.data.elapsed_ms / 1000);
          progressText.textContent += " — " + formatBytes(speed) + "/s";
          if (msg.data.completed > 0 && msg.data.completed < msg.data.total) {
            var etaMs = (msg.data.total - msg.data.completed) * (msg.data.elapsed_ms / msg.data.completed);
            progressText.textContent += " — ~" + formatDuration(etaMs) + " left";
          }
        }
      } else if (msg.event === "finished") {
        var sizeStr = formatBytes(msg.data.total_bytes);
        progressBarInner.style.width = "100%";
        progressPhase.textContent = "Retry complete!";
        removeSpoilerBlur();
        progressText.textContent =
          msg.data.downloaded + " downloaded" +
          (msg.data.failed > 0 ? ", " + msg.data.failed + " still failed" : "") +
          " (" + sizeStr + ")";
      }
    };

    invoke("retry_failed_assets", { caseId: caseId, onEvent: onEvent })
      .then(function (manifest) {
        console.log("[RETRY] retry_failed_assets success:", JSON.stringify({
          case_id: manifest.case_id,
          total_downloaded: manifest.assets.total_downloaded,
          still_failed: manifest.failed_assets ? manifest.failed_assets.length : 0
        }));
        var stillFailed = manifest.failed_assets ? manifest.failed_assets.length : 0;
        if (stillFailed === 0) {
          downloadResult.textContent = "All assets downloaded successfully!";
          downloadResult.className = "result-success";
        } else {
          downloadResult.textContent = stillFailed + " asset(s) still failed (server may be down).";
          downloadResult.className = "result-error";
        }
        loadLibrary();
      })
      .catch(function (e) {
        console.error("[RETRY] retry_failed_assets error:", e);
        downloadResult.textContent = "Retry error: " + e;
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        setTimeout(function () {
          progressContainer.classList.add("hidden");
        }, 4000);
        processQueue();
      });
    });
  }

  function startSequenceDownload(caseIds, sequenceTitle) {
    if (downloadInProgress) {
      downloadQueue.push({ type: "sequence", caseIds: caseIds, sequenceTitle: sequenceTitle });
      statusMsg.textContent = "Queued for download (" + downloadQueue.length + " in queue).";
      return;
    }
    console.log("[DOWNLOAD] startSequenceDownload ids=" + JSON.stringify(caseIds) + " title=" + sequenceTitle);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    progressContainer.classList.remove("hidden");
    cancelDownloadBtn.classList.remove("hidden");
    progressPhase.textContent = 'Downloading "' + sequenceTitle + '" (' + caseIds.length + " parts)...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var currentPartLabel = "";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      console.log("[SEQUENCE EVENT]", JSON.stringify(msg));
      if (msg.event === "sequence_progress") {
        currentPartLabel = "Part " + msg.data.current_part + "/" + msg.data.total_parts +
          ": " + msg.data.part_title;
        progressPhase.textContent = currentPartLabel;
        progressBarInner.style.width = "0%";
        progressText.textContent = "";
      } else if (msg.event === "started") {
        progressText.textContent = "0 / " + msg.data.total + " assets";
      } else if (msg.event === "progress") {
        var pct = msg.data.total > 0 ? Math.round((msg.data.completed / msg.data.total) * 100) : 0;
        progressBarInner.style.width = pct + "%";
        progressText.textContent =
          msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur();
        }
        if (msg.data.elapsed_ms > 1000 && msg.data.bytes_downloaded > 0) {
          var speed = msg.data.bytes_downloaded / (msg.data.elapsed_ms / 1000);
          progressText.textContent += " — " + formatBytes(speed) + "/s";
          if (msg.data.completed > 0 && msg.data.completed < msg.data.total) {
            var etaMs = (msg.data.total - msg.data.completed) * (msg.data.elapsed_ms / msg.data.completed);
            progressText.textContent += " — ~" + formatDuration(etaMs) + " left";
          }
        }
      } else if (msg.event === "finished") {
        var sizeStr = formatBytes(msg.data.total_bytes);
        progressBarInner.style.width = "100%";
        progressPhase.textContent = "Sequence download complete!";
        removeSpoilerBlur();
        progressText.textContent =
          msg.data.downloaded + " downloaded" +
          (msg.data.failed > 0 ? ", " + msg.data.failed + " failed" : "") +
          " (" + sizeStr + ")";
      } else if (msg.event === "error") {
        progressPhase.textContent = "Error";
        progressText.textContent = msg.data.message;
      }
    };

    invoke("download_sequence", { caseIds: caseIds, onEvent: onEvent })
      .then(function (manifests) {
        console.log("[DOWNLOAD] download_sequence success: " + manifests.length + " parts");
        downloadResult.innerHTML =
          '<strong>' + escapeHtml(sequenceTitle) + '</strong> (' +
          manifests.length + ' parts) &mdash; ready to play!';
        downloadResult.className = "result-success";
        caseIdInput.value = "";
        loadLibrary();
      })
      .catch(function (e) {
        console.error("[DOWNLOAD] download_sequence error:", e);
        downloadResult.textContent = "Error: " + e;
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        caseIdInput.disabled = false;
        cancelDownloadBtn.classList.add("hidden");
        setTimeout(function () {
          progressContainer.classList.add("hidden");
        }, 4000);
        processQueue();
      });
  }

  function startDownload(caseId) {
    if (downloadInProgress) {
      downloadQueue.push({ type: "single", caseId: caseId });
      statusMsg.textContent = "Queued for download (" + downloadQueue.length + " in queue).";
      return;
    }
    console.log("[DOWNLOAD] startDownload caseId=" + caseId);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    // Show progress
    progressContainer.classList.remove("hidden");
    cancelDownloadBtn.classList.remove("hidden");
    progressPhase.textContent = "Fetching case info...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      console.log("[DOWNLOAD EVENT]", JSON.stringify(msg));
      if (msg.event === "started") {
        progressPhase.textContent = "Downloading " + msg.data.total + " assets...";
        progressText.textContent = "0 / " + msg.data.total;
      } else if (msg.event === "progress") {
        var pct = Math.round((msg.data.completed / msg.data.total) * 100);
        progressBarInner.style.width = pct + "%";
        progressText.textContent =
          msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur();
        }
        if (msg.data.elapsed_ms > 1000 && msg.data.bytes_downloaded > 0) {
          var speed = msg.data.bytes_downloaded / (msg.data.elapsed_ms / 1000);
          progressText.textContent += " — " + formatBytes(speed) + "/s";
          if (msg.data.completed > 0 && msg.data.completed < msg.data.total) {
            var etaMs = (msg.data.total - msg.data.completed) * (msg.data.elapsed_ms / msg.data.completed);
            progressText.textContent += " — ~" + formatDuration(etaMs) + " left";
          }
        }
      } else if (msg.event === "finished") {
        var sizeStr = formatBytes(msg.data.total_bytes);
        progressBarInner.style.width = "100%";
        progressPhase.textContent = "Download complete!";
        removeSpoilerBlur();
        progressText.textContent =
          msg.data.downloaded + " downloaded" +
          (msg.data.failed > 0 ? ", " + msg.data.failed + " failed" : "") +
          " (" + sizeStr + ")";
      } else if (msg.event === "error") {
        progressPhase.textContent = "Error";
        progressText.textContent = msg.data.message;
      }
    };

    invoke("download_case", { caseId: caseId, onEvent: onEvent })
      .then(function (manifest) {
        console.log("[DOWNLOAD] download_case success:", JSON.stringify({
          case_id: manifest.case_id,
          title: manifest.title,
          total_downloaded: manifest.assets.total_downloaded,
          total_size: manifest.assets.total_size_bytes,
          failed_count: manifest.failed_assets ? manifest.failed_assets.length : 0
        }));
        downloadResult.innerHTML =
          '<strong>' + escapeHtml(manifest.title) + '</strong> by ' +
          escapeHtml(manifest.author) + ' &mdash; ready to play!';
        downloadResult.className = "result-success";
        caseIdInput.value = "";
        loadLibrary();
        // Scroll the new case into view after a brief delay
        setTimeout(function () {
          var card = document.querySelector('.case-card[data-case-id="' + manifest.case_id + '"]');
          if (card) {
            card.scrollIntoView({ behavior: "smooth", block: "nearest" });
            card.classList.add("case-card-highlight");
            setTimeout(function () { card.classList.remove("case-card-highlight"); }, 2000);
          }
        }, 200);
      })
      .catch(function (e) {
        console.error("[DOWNLOAD] download_case error:", e);
        var errMsg = String(e);
        // Make common errors more readable
        if (errMsg.indexOf("404") !== -1 || errMsg.indexOf("not found") !== -1) {
          downloadResult.textContent = "Case not found. Check the ID and try again.";
        } else if (errMsg.indexOf("timeout") !== -1 || errMsg.indexOf("connection") !== -1) {
          downloadResult.textContent = "Connection failed. Check your internet and try again.";
        } else {
          downloadResult.textContent = "Error: " + errMsg;
        }
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        caseIdInput.disabled = false;
        cancelDownloadBtn.classList.add("hidden");
        setTimeout(function () {
          progressContainer.classList.add("hidden");
        }, 4000);
        processQueue();
      });
  }

  // --- Helpers ---

  function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    var units = ["B", "KB", "MB", "GB"];
    var i = 0;
    var b = bytes;
    while (b >= 1024 && i < units.length - 1) {
      b /= 1024;
      i++;
    }
    return b.toFixed(i > 0 ? 1 : 0) + " " + units[i];
  }

  function formatDuration(ms) {
    var secs = Math.round(ms / 1000);
    if (secs < 60) return secs + "s";
    var mins = Math.floor(secs / 60);
    var remainSecs = secs % 60;
    if (mins < 60) return mins + "m " + remainSecs + "s";
    var hrs = Math.floor(mins / 60);
    var remainMins = mins % 60;
    return hrs + "h " + remainMins + "m";
  }

  function formatDate(isoStr) {
    // "2025-01-15T12:30:00Z" → "Jan 15, 2025"
    if (!isoStr) return "";
    var months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    var parts = isoStr.split("T")[0].split("-");
    if (parts.length !== 3) return isoStr.split("T")[0];
    var y = parts[0];
    var m = parseInt(parts[1], 10) - 1;
    var d = parseInt(parts[2], 10);
    return months[m] + " " + d + ", " + y;
  }

  function escapeHtml(text) {
    if (!text) return "";
    var div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  function showFailedAssetsModal(failedAssets) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";
    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>" + failedAssets.length + " failed asset(s)</strong>";
    var list = document.createElement("div");
    list.className = "plugin-list";
    for (var i = 0; i < failedAssets.length; i++) {
      var item = document.createElement("div");
      item.className = "plugin-list-item";
      var nameSpan = document.createElement("span");
      nameSpan.className = "plugin-name";
      nameSpan.textContent = failedAssets[i].url || "unknown";
      nameSpan.style.fontSize = "0.75rem";
      nameSpan.style.wordBreak = "break-all";
      var errSpan = document.createElement("span");
      errSpan.style.color = "#a66";
      errSpan.style.fontSize = "0.75rem";
      errSpan.style.flexShrink = "0";
      errSpan.textContent = failedAssets[i].error || "";
      item.appendChild(nameSpan);
      item.appendChild(errSpan);
      list.appendChild(item);
    }
    var closeBtn = document.createElement("button");
    closeBtn.className = "modal-btn modal-btn-cancel";
    closeBtn.textContent = "Close";
    closeBtn.style.width = "100%";
    closeBtn.style.marginTop = "0.75rem";
    function close() { document.body.removeChild(overlay); }
    closeBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });
    modal.appendChild(titleEl);
    modal.appendChild(list);
    modal.appendChild(closeBtn);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);
  }

  /**
   * Show a modal dialog with a message and two action buttons.
   * Calls callback(choice) where choice is 1 or 2, or does nothing if cancelled.
   */
  function showUpdateModal(message, btn1Label, btn2Label, callback) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var msg = document.createElement("p");
    msg.className = "modal-message";
    msg.textContent = message;

    var buttons = document.createElement("div");
    buttons.className = "modal-buttons";

    var btn1 = document.createElement("button");
    btn1.className = "modal-btn modal-btn-primary";
    btn1.textContent = btn1Label;

    var btn2 = document.createElement("button");
    btn2.className = "modal-btn modal-btn-secondary";
    btn2.textContent = btn2Label;

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    btn1.addEventListener("click", function () { close(); callback(1); });
    btn2.addEventListener("click", function () { close(); callback(2); });
    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(btn1);
    buttons.appendChild(btn2);
    buttons.appendChild(cancelBtn);
    modal.appendChild(msg);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);
  }

  function showConfirmModal(message, confirmLabel, onConfirm, onCancel) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var msg = document.createElement("p");
    msg.className = "modal-message";
    msg.textContent = message;

    var buttons = document.createElement("div");
    buttons.className = "modal-buttons";

    var yesBtn = document.createElement("button");
    yesBtn.className = "modal-btn modal-btn-primary";
    yesBtn.textContent = confirmLabel || "OK";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    yesBtn.addEventListener("click", function () { close(); if (onConfirm) onConfirm(); });
    cancelBtn.addEventListener("click", function () { close(); if (onCancel) onCancel(); });
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) { close(); if (onCancel) onCancel(); }
    });

    buttons.appendChild(yesBtn);
    buttons.appendChild(cancelBtn);
    modal.appendChild(msg);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);
  }

  function showPromptModal(message, inputLabel, defaultValue, confirmLabel, onConfirm) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog";

    var msg = document.createElement("p");
    msg.className = "modal-message";
    msg.textContent = message;

    var field = document.createElement("div");
    field.className = "modal-field";
    var label = document.createElement("label");
    label.textContent = inputLabel;
    var input = document.createElement("input");
    input.type = "text";
    input.value = defaultValue || "";
    input.placeholder = inputLabel;
    field.appendChild(label);
    field.appendChild(input);

    var buttons = document.createElement("div");
    buttons.className = "modal-buttons";

    var okBtn = document.createElement("button");
    okBtn.className = "modal-btn modal-btn-primary";
    okBtn.textContent = confirmLabel || "OK";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() { document.body.removeChild(overlay); }

    okBtn.addEventListener("click", function () {
      var val = input.value.trim();
      if (!val) { input.style.borderColor = "#a33"; input.focus(); return; }
      close();
      onConfirm(val);
    });

    input.addEventListener("keydown", function (e) {
      if (e.key === "Enter") okBtn.click();
    });

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(okBtn);
    buttons.appendChild(cancelBtn);
    modal.appendChild(msg);
    modal.appendChild(field);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    input.focus();
    input.select();
  }

  /**
   * Read game_saves from the game server's localStorage (different origin than launcher).
   * Creates a hidden iframe on the server origin, reads its localStorage, then cleans up.
   * Returns a Promise that resolves with the save info or null.
   */
  /**
   * Read game_saves from the game server's localStorage (different origin than launcher).
   * Creates a hidden iframe on the server origin, reads its localStorage, then cleans up.
   * Returns a Promise that resolves with the save info or null.
   */
  /**
   * Read game_saves from the game server's localStorage (different origin than launcher).
   * Uses a hidden iframe + postMessage to cross the origin boundary safely.
   * Returns a Promise that resolves with the save info or null.
   */
  var bridgeIdCounter = 0;

  function findLastSequenceSave(sequenceList) {
    var caseIds = sequenceList.map(function (p) { return p.id; });
    return invoke("find_latest_save", { caseIds: caseIds }).then(function (result) {
      if (result) {
        return {
          partId: result.partId,
          saveDate: result.saveDate,
          saveDataBase64: btoa(unescape(encodeURIComponent(result.saveString)))
        };
      }
      return findLastSequenceSaveBridge(sequenceList);
    });
  }

  function findLastSequenceSaveBridge(sequenceList) {
    return invoke("get_server_url").then(function (serverUrl) {
      var bridgeId = "read_" + (++bridgeIdCounter);
      console.log("[SAVE] Server URL:", serverUrl, "bridgeId:", bridgeId);

      return new Promise(function (resolve) {
        var iframe = document.createElement("iframe");
        iframe.style.display = "none";
        var resolved = false;

        // Listen for postMessage from the bridge page (filter by bridgeId)
        function onMessage(event) {
          if (resolved) return;
          if (!event.data || event.data.type !== "game_saves") return;
          if (event.data.bridgeId && event.data.bridgeId !== bridgeId) return;

          console.log("[SAVE]", bridgeId, "Received postMessage, data length:", event.data.data ? event.data.data.length : "null");
          if (event.data.error) {
            console.error("[SAVE] Bridge error:", event.data.error);
          }
          resolved = true;
          window.removeEventListener("message", onMessage);
          document.body.removeChild(iframe);

          var raw = event.data.data;
          var gameSaves = raw ? JSON.parse(raw) : null;
          if (!gameSaves) {
            console.log("[SAVE] No game_saves in server localStorage");
            resolve(null);
            return;
          }

          console.log("[SAVE] game_saves keys:", Object.keys(gameSaves));
          var latestDate = 0;
          var latestPartId = null;
          var latestSaveString = null;

          for (var i = 0; i < sequenceList.length; i++) {
            var partId = sequenceList[i].id;
            if (!(partId in gameSaves)) continue;
            console.log("[SAVE] Part", partId, "has", Object.keys(gameSaves[partId]).length, "saves");
            for (var saveDate in gameSaves[partId]) {
              var ts = parseInt(saveDate, 10);
              if (ts > latestDate) {
                latestDate = ts;
                latestPartId = partId;
                latestSaveString = gameSaves[partId][saveDate];
              }
            }
          }

          if (!latestPartId) {
            console.log("[SAVE] No matching saves for sequence parts");
            resolve(null);
            return;
          }
          console.log("[SAVE] Latest save: part", latestPartId, "date", new Date(latestDate));
          resolve({
            partId: latestPartId,
            saveDate: latestDate,
            saveDataBase64: btoa(unescape(encodeURIComponent(latestSaveString)))
          });
        }

        window.addEventListener("message", onMessage);

        // Timeout after 3s
        setTimeout(function () {
          if (!resolved) {
            resolved = true;
            window.removeEventListener("message", onMessage);
            if (iframe.parentNode) document.body.removeChild(iframe);
            console.warn("[SAVE]", bridgeId, "Bridge timed out");
            resolve(null);
          }
        }, 3000);

        iframe.src = serverUrl + "/localstorage_bridge.html?id=" + bridgeId;
        document.body.appendChild(iframe);
      });
    });
  }

  /**
   * Read all game_saves from the game server's localStorage.
   * If caseIds is provided, filters to only those case IDs.
   * Returns a Promise that resolves with the saves object or null.
   */
  function readGameSaves(caseIds) {
    return invoke("get_server_url").then(function (serverUrl) {
      var bridgeId = "saves_" + (++bridgeIdCounter);
      return new Promise(function (resolve) {
        var iframe = document.createElement("iframe");
        iframe.style.display = "none";
        var resolved = false;

        function onMessage(event) {
          if (resolved) return;
          if (!event.data || event.data.type !== "game_saves") return;
          if (event.data.bridgeId && event.data.bridgeId !== bridgeId) return;
          resolved = true;
          window.removeEventListener("message", onMessage);
          document.body.removeChild(iframe);

          var raw = event.data.data;
          var gameSaves = raw ? JSON.parse(raw) : null;
          if (!gameSaves) {
            resolve(null);
            return;
          }

          // Filter to requested case IDs if specified
          if (caseIds && caseIds.length > 0) {
            var filtered = {};
            var found = false;
            for (var i = 0; i < caseIds.length; i++) {
              var id = String(caseIds[i]);
              if (id in gameSaves) {
                filtered[id] = gameSaves[id];
                found = true;
              }
            }
            resolve(found ? filtered : null);
          } else {
            resolve(gameSaves);
          }
        }

        window.addEventListener("message", onMessage);
        setTimeout(function () {
          if (!resolved) {
            resolved = true;
            window.removeEventListener("message", onMessage);
            if (iframe.parentNode) document.body.removeChild(iframe);
            resolve(null);
          }
        }, 3000);

        iframe.src = serverUrl + "/localstorage_bridge.html?id=" + bridgeId;
        document.body.appendChild(iframe);
      });
    });
  }

  /**
   * Write (merge) saves into the game server's localStorage.
   * Won't overwrite existing save timestamps — only adds new ones.
   * Returns a Promise that resolves with { success, merged } or rejects.
   */
  function writeGameSaves(savesJson) {
    return invoke("get_server_url").then(function (serverUrl) {
      var bridgeId = "write_" + (++bridgeIdCounter);
      console.log("[SAVE]", bridgeId, "writeGameSaves starting");
      return new Promise(function (resolve) {
        var iframe = document.createElement("iframe");
        iframe.style.display = "none";
        var resolved = false;

        function onMessage(event) {
          if (!event.data) return;
          // Filter by bridgeId to prevent cross-talk with concurrent bridge iframes
          if (event.data.bridgeId && event.data.bridgeId !== bridgeId) return;

          // Wait for the initial game_saves message (bridge sends it on load),
          // then send our write_saves message.
          if (event.data.type === "game_saves" && !resolved) {
            console.log("[SAVE]", bridgeId, "Bridge loaded, sending write_saves");
            iframe.contentWindow.postMessage({ type: "write_saves", data: savesJson, bridgeId: bridgeId }, "*");
          }

          if (event.data.type === "write_saves_result") {
            if (event.data.bridgeId && event.data.bridgeId !== bridgeId) return;
            resolved = true;
            window.removeEventListener("message", onMessage);
            if (iframe.parentNode) document.body.removeChild(iframe);
            console.log("[SAVE]", bridgeId, "write result:", event.data);
            resolve(event.data);
          }
        }

        window.addEventListener("message", onMessage);
        setTimeout(function () {
          if (!resolved) {
            resolved = true;
            window.removeEventListener("message", onMessage);
            if (iframe.parentNode) document.body.removeChild(iframe);
            console.warn("[SAVE]", bridgeId, "Bridge timed out");
            resolve({ success: false, error: "Bridge timed out" });
          }
        }, 5000);

        iframe.src = serverUrl + "/localstorage_bridge.html?id=" + bridgeId;
        document.body.appendChild(iframe);
      });
    });
  }

  function copyTrialLink(caseId) {
    var url = "https://aaonline.fr/player.php?trial_id=" + caseId;
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(url).then(function () {
        statusMsg.textContent = "Link copied: " + url;
      }).catch(function () {
        // Fallback: open in browser
        window.__TAURI__.shell.open(url);
        statusMsg.textContent = "Opened: " + url;
      });
    } else {
      window.__TAURI__.shell.open(url);
      statusMsg.textContent = "Opened: " + url;
    }
  }

  // --- Plugins Panel ---

  var pluginsToggle = document.getElementById("plugins-toggle");
  var pluginsPanel = document.getElementById("plugins-panel");
  var globalPluginsList = document.getElementById("global-plugins-list");
  var globalAttachBtn = document.getElementById("global-attach-btn");
  var globalImportBtn = document.getElementById("global-import-btn");

  pluginsToggle.addEventListener("click", function () {
    var isOpen = !pluginsPanel.classList.contains("hidden");
    if (isOpen) {
      pluginsPanel.classList.add("hidden");
      pluginsToggle.classList.remove("open");
    } else {
      pluginsPanel.classList.remove("hidden");
      pluginsToggle.classList.add("open");
      loadGlobalPluginsPanel();
    }
  });

  function loadGlobalPluginsPanel() {
    invoke("list_global_plugins")
      .then(function (manifest) {
        var scripts = (manifest && manifest.scripts) || [];
        var plugins = (manifest && manifest.plugins) || {};
        var disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
        globalPluginsList.innerHTML = "";
        if (scripts.length === 0) {
          var empty = document.createElement("div");
          empty.className = "global-plugins-empty";
          empty.textContent = "No global plugins installed.";
          globalPluginsList.appendChild(empty);
        } else {
          for (var i = 0; i < scripts.length; i++) {
            (function (filename) {
              var isDisabled = disabledList.indexOf(filename) !== -1;
              var row = document.createElement("div");
              row.className = "global-plugin-row" + (isDisabled ? " disabled" : "");

              var toggle = document.createElement("input");
              toggle.type = "checkbox";
              toggle.checked = !isDisabled;
              toggle.style.accentColor = "#4a90d9";
              toggle.style.width = "1rem";
              toggle.style.height = "1rem";
              toggle.style.flexShrink = "0";
              toggle.addEventListener("change", function () {
                invoke("toggle_global_plugin", { filename: filename, enabled: toggle.checked })
                  .then(function () { loadGlobalPluginsPanel(); })
                  .catch(function (e) { statusMsg.textContent = "Error: " + e; });
              });

              var name = document.createElement("span");
              name.className = "plugin-name";
              name.textContent = filename;

              // Scope badge
              var pluginEntry = plugins[filename] || {};
              var scope = pluginEntry.scope;
              var scopeIsAll = scope && scope.all;
              var scopeBadge = document.createElement("span");
              scopeBadge.className = "scope-badge";
              if (scopeIsAll) {
                scopeBadge.textContent = "All cases";
              } else {
                // Old scope format — show clickable fix button
                scopeBadge = document.createElement("button");
                scopeBadge.className = "scope-badge";
                scopeBadge.style.cssText = "cursor:pointer; border:none; background:none; text-decoration:underline;";
                scopeBadge.textContent = "Restricted";
                scopeBadge.title = "Click to set scope to all cases";
              }
              if (!scopeIsAll) {
                scopeBadge.addEventListener("click", (function (fn) {
                  return function () {
                    invoke("set_global_plugin_scope", { filename: fn, scope: { all: true } })
                      .then(function () { loadGlobalPluginsPanel(); })
                      .catch(function (e) { statusMsg.textContent = "Error: " + e; });
                  };
              })(filename, scopeIsAll));

              // Override summary badge
              var overrideBadge = document.createElement("span");
              overrideBadge.className = "scope-badge";
              if (isDisabled) {
                // Globally disabled → count enabled_for entries
                var ef = pluginEntry.enabled_for || {};
                var efCount = ((ef.cases || []).length) + ((ef.sequences || []).length) + ((ef.collections || []).length);
                if (efCount > 0) {
                  overrideBadge.textContent = efCount + " enabled";
                  overrideBadge.style.color = "#6a6";
                }
              } else {
                // Globally enabled → count disabled_for entries
                var df = pluginEntry.disabled_for || {};
                var dfCount = ((df.cases || []).length) + ((df.sequences || []).length) + ((df.collections || []).length);
                if (dfCount > 0) {
                  overrideBadge.textContent = dfCount + " exception" + (dfCount !== 1 ? "s" : "");
                  overrideBadge.style.color = "#a66";
                }
              }

              var paramsBtn = document.createElement("button");
              paramsBtn.className = "small-btn";
              paramsBtn.textContent = "Params";
              paramsBtn.style.cssText = "font-size:0.72rem; padding:0.1rem 0.5rem;";
              paramsBtn.addEventListener("click", function () {
                showPluginParamsModal(filename, "Global Default", "default", "");
              });

              var removeBtn = document.createElement("button");
              removeBtn.className = "plugin-remove-btn";
              removeBtn.textContent = "Remove";
              removeBtn.addEventListener("click", function () {
                showConfirmModal("Remove global plugin \"" + filename + "\"?", "Remove", function () {
                  invoke("remove_global_plugin", { filename: filename })
                    .then(function () { loadGlobalPluginsPanel(); })
                    .catch(function (e) { statusMsg.textContent = "Error: " + e; });
                });
              });

              row.appendChild(toggle);
              row.appendChild(name);
              row.appendChild(scopeBadge);
              if (overrideBadge.textContent) row.appendChild(overrideBadge);
              row.appendChild(paramsBtn);
              row.appendChild(removeBtn);
              globalPluginsList.appendChild(row);
            })(scripts[i]);
          }
        }
      })
      .catch(function (e) {
        globalPluginsList.innerHTML = "";
        var errEl = document.createElement("div");
        errEl.className = "global-plugins-empty";
        errEl.textContent = "Error loading plugins: " + e;
        globalPluginsList.appendChild(errEl);
      });
  }

  function showGlobalAttachCodeModal(onDone) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Attach Global Plugin Code</strong>";

    var filenameField = document.createElement("div");
    filenameField.className = "modal-field";
    var filenameLabel = document.createElement("label");
    filenameLabel.textContent = "Filename";
    var filenameInput = document.createElement("input");
    filenameInput.type = "text";
    filenameInput.placeholder = "my_plugin.js";
    filenameField.appendChild(filenameLabel);
    filenameField.appendChild(filenameInput);

    var codeField = document.createElement("div");
    codeField.className = "modal-field";
    var codeLabel = document.createElement("label");
    codeLabel.textContent = "Plugin Code";
    var codeInput = document.createElement("textarea");
    codeInput.className = "attach-code-textarea";
    codeInput.placeholder = "// Paste your plugin JS code here...";
    codeField.appendChild(codeLabel);
    codeField.appendChild(codeInput);

    var userEditedFilename = false;
    filenameInput.addEventListener("input", function () {
      userEditedFilename = true;
    });

    function detectPluginName() {
      var code = codeInput.value;
      var nameMatch = code.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
      if (nameMatch) {
        var detected = nameMatch[1] + ".js";
        filenameInput.placeholder = detected;
        if (!userEditedFilename) {
          filenameInput.value = detected;
        }
      }
    }

    codeInput.addEventListener("input", detectPluginName);
    codeInput.addEventListener("paste", function () {
      setTimeout(detectPluginName, 0);
    });

    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var attachBtn = document.createElement("button");
    attachBtn.className = "modal-btn modal-btn-secondary";
    attachBtn.textContent = "Attach";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    attachBtn.addEventListener("click", function () {
      var filename = filenameInput.value.trim();
      if (!filename && filenameInput.placeholder && filenameInput.placeholder !== "my_plugin.js") {
        filename = filenameInput.placeholder;
      }
      var code = codeInput.value;

      if (!filename) {
        filenameInput.style.borderColor = "#a33";
        filenameInput.focus();
        return;
      }
      if (!filename.toLowerCase().endsWith(".js")) {
        filename = filename + ".js";
      }
      if (!code) {
        codeInput.style.borderColor = "#a33";
        codeInput.focus();
        return;
      }

      close();
      statusMsg.textContent = "Attaching global plugin...";
      invoke("attach_global_plugin_code", {
        code: code,
        filename: filename
      })
      .then(function () {
        statusMsg.textContent = "Global plugin \"" + filename + "\" attached.";
        if (onDone) onDone();
      })
      .catch(function (e) {
        statusMsg.textContent = "Error attaching plugin: " + e;
      });
    });

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    buttons.appendChild(attachBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(titleEl);
    modal.appendChild(filenameField);
    modal.appendChild(codeField);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    filenameInput.focus();
  }

  globalAttachBtn.addEventListener("click", function () {
    showGlobalAttachCodeModal(function () { loadGlobalPluginsPanel(); });
  });

  globalImportBtn.addEventListener("click", function () {
    invoke("pick_import_file")
      .then(function (selected) {
        if (!selected) return;
        if (!selected.toLowerCase().endsWith(".aaoplug")) {
          statusMsg.textContent = "Please select a .aaoplug file.";
          return;
        }
        statusMsg.textContent = "Importing global plugin...";
        invoke("import_global_plugin_file", { sourcePath: selected })
          .then(function () {
            statusMsg.textContent = "Global plugin imported.";
            loadGlobalPluginsPanel();
          })
          .catch(function () {
            // Command may not exist — fall back to attach code modal
            statusMsg.textContent = "Direct import not available. Use Attach Code instead.";
          });
      })
      .catch(function (e) {
        statusMsg.textContent = "Could not open file picker: " + e;
      });
  });

  // --- Settings ---

  var settingsToggle = document.getElementById("settings-toggle");
  var settingsPanel = document.getElementById("settings-panel");
  var settingsLanguage = document.getElementById("settings-language");
  var settingsConcurrency = document.getElementById("settings-concurrency");
  var concurrencyValue = document.getElementById("concurrency-value");
  var dataDirPath = document.getElementById("data-dir-path");
  var openDataDirBtn = document.getElementById("open-data-dir-btn");
  var storageText = document.getElementById("storage-text");
  var optimizeStorageBtn = document.getElementById("optimize-storage-btn");
  var clearUnusedBtn = document.getElementById("clear-unused-defaults-btn");

  var settingsSaveTimeout = null;

  settingsToggle.addEventListener("click", function () {
    var isOpen = !settingsPanel.classList.contains("hidden");
    if (isOpen) {
      settingsPanel.classList.add("hidden");
      settingsToggle.classList.remove("open");
    } else {
      settingsPanel.classList.remove("hidden");
      settingsToggle.classList.add("open");
      loadStorageInfo();
    }
  });

  var settingsAutoSave = document.getElementById("settings-autosave");
  var settingsBlurSpoilers = document.getElementById("settings-blur-spoilers");

  function loadSettings() {
    invoke("get_settings").then(function (settings) {
      settingsLanguage.value = settings.language;
      settingsConcurrency.value = settings.concurrent_downloads;
      concurrencyValue.textContent = settings.concurrent_downloads;
      if (settingsAutoSave) settingsAutoSave.checked = settings.auto_save;
      if (settingsBlurSpoilers) settingsBlurSpoilers.checked = settings.blur_spoilers;
    }).catch(function (e) {
      console.error("[SETTINGS] Failed to load settings:", e);
    });
  }

  function saveSettings() {
    var settings = {
      language: settingsLanguage.value,
      concurrent_downloads: parseInt(settingsConcurrency.value, 10),
      auto_save: settingsAutoSave ? settingsAutoSave.checked : true,
      blur_spoilers: settingsBlurSpoilers ? settingsBlurSpoilers.checked : true
    };
    invoke("save_settings", { settings: settings }).catch(function (e) {
      console.error("[SETTINGS] Failed to save settings:", e);
    });
  }

  function debounceSave() {
    if (settingsSaveTimeout) clearTimeout(settingsSaveTimeout);
    settingsSaveTimeout = setTimeout(saveSettings, 300);
  }

  settingsLanguage.addEventListener("change", debounceSave);

  settingsConcurrency.addEventListener("input", function () {
    concurrencyValue.textContent = settingsConcurrency.value;
    debounceSave();
  });

  if (settingsAutoSave) settingsAutoSave.addEventListener("change", debounceSave);
  if (settingsBlurSpoilers) settingsBlurSpoilers.addEventListener("change", debounceSave);

  function loadStorageInfo() {
    invoke("get_storage_info").then(function (info) {
      dataDirPath.textContent = info.data_dir;
      var casesSize = formatBytes(info.cases_size_bytes);
      var defaultsSize = formatBytes(info.defaults_size_bytes);
      var totalSize = formatBytes(info.total_size_bytes);
      storageText.textContent =
        info.cases_count + " case" + (info.cases_count !== 1 ? "s" : "") +
        " (" + casesSize + ") + defaults cache (" + defaultsSize + ") = " + totalSize + " total";
      storageText.className = "";
    }).catch(function (e) {
      console.error("[SETTINGS] Failed to load storage info:", e);
      storageText.textContent = "Unable to compute storage info.";
    });
  }

  openDataDirBtn.addEventListener("click", function () {
    invoke("open_data_dir").catch(function (e) {
      console.error("[SETTINGS] Failed to open data dir:", e);
    });
  });

  optimizeStorageBtn.addEventListener("click", function () {
    optimizeStorageBtn.disabled = true;
    optimizeStorageBtn.textContent = "Optimizing...";
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Optimizing storage...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "Scanning cases...";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      if (msg.event === "progress") {
        var pct = msg.data.total > 0 ? Math.round((msg.data.completed / msg.data.total) * 100) : 0;
        progressBarInner.style.width = pct + "%";
        progressText.textContent = msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur();
        }
      }
    };

    invoke("optimize_storage", { onEvent: onEvent }).then(function (result) {
      optimizeStorageBtn.textContent = "Optimize Storage";
      optimizeStorageBtn.disabled = false;
      removeSpoilerBlur();
      progressContainer.classList.add("hidden");
      if (result.deduped > 0) {
        statusMsg.textContent = "Optimized: " + result.deduped + " files deduplicated, " + formatBytes(result.bytes_saved) + " saved.";
      } else {
        statusMsg.textContent = "Storage is already optimized. No duplicates found.";
      }
      loadStorageInfo();
    }).catch(function (e) {
      optimizeStorageBtn.textContent = "Optimize Storage";
      optimizeStorageBtn.disabled = false;
      progressContainer.classList.add("hidden");
      console.error("[SETTINGS] Failed to optimize storage:", e);
      statusMsg.textContent = "Error optimizing storage: " + e;
    });
  });

  clearUnusedBtn.addEventListener("click", function () {
    showConfirmModal(
      "Remove cached default assets not used by any downloaded case?\n\nThis frees disk space safely. The assets will be re-downloaded if needed later.",
      "Clear Unused",
      function () {
        clearUnusedBtn.disabled = true;
        clearUnusedBtn.textContent = "Clearing...";
        invoke("clear_unused_defaults").then(function (result) {
          clearUnusedBtn.textContent = "Clear Unused";
          clearUnusedBtn.disabled = false;
          if (result.deleted > 0) {
            statusMsg.textContent = "Cleared " + result.deleted + " unused files (" + formatBytes(result.bytes_freed) + " freed).";
          } else {
            statusMsg.textContent = "No unused default assets found.";
          }
          loadStorageInfo();
        }).catch(function (e) {
          clearUnusedBtn.textContent = "Clear Unused";
          clearUnusedBtn.disabled = false;
          console.error("[SETTINGS] Failed to clear unused defaults:", e);
          statusMsg.textContent = "Error clearing unused defaults: " + e;
        });
      }
    );
  });

  // --- Import ---

  var importFolderBtn = document.getElementById("import-folder-btn");
  var importFileBtn = document.getElementById("import-file-btn");
  var importPasteSaveBtn = document.getElementById("import-paste-save-btn");
  var importResult = document.getElementById("import-result");

  function doImport(sourcePath) {
    console.log("[IMPORT] doImport called with path:", sourcePath);
    importFolderBtn.disabled = true;
    importFileBtn.disabled = true;
    importResult.textContent = "";
    importResult.className = "";

    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Importing...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      if (msg.event === "sequence_progress") {
        progressPhase.textContent =
          "Case " + msg.data.current_part + " / " + msg.data.total_parts + ": " + msg.data.part_title;
      } else if (msg.event === "progress") {
        var pct = Math.round((msg.data.completed / msg.data.total) * 100);
        progressBarInner.style.width = pct + "%";
        progressText.textContent =
          msg.data.completed + " / " + msg.data.total + " files (" + pct + "%)";
      } else if (msg.event === "finished") {
        progressBarInner.style.width = "100%";
        progressPhase.textContent = "Import complete!";
        progressText.textContent =
          msg.data.downloaded + " assets (" + formatBytes(msg.data.total_bytes) + ")";
      }
    };

    invoke("import_case", { sourcePath: sourcePath, onEvent: onEvent })
      .then(function (result) {
        var manifest = result.manifest;
        var savesInfo = "";

        // If the imported file contained saves, merge them into localStorage.
        // IMPORTANT: wait for writeGameSaves to complete BEFORE loadLibrary(),
        // because both create bridge iframes and their postMessage listeners cross-talk.
        console.log("[IMPORT] result.saves:", result.saves ? "present (" + JSON.stringify(result.saves).length + " bytes)" : "null");
        console.log("[IMPORT] result.missing_defaults:", result.missing_defaults);

        var savesPromise = Promise.resolve();
        if (result.saves) {
          var saveCount = 0;
          for (var caseId in result.saves) {
            for (var ts in result.saves[caseId]) {
              saveCount++;
            }
          }
          if (saveCount > 0) {
            savesInfo = ", " + saveCount + " save" + (saveCount > 1 ? "s" : "") + " imported";
            savesPromise = writeGameSaves(result.saves).then(function (writeResult) {
              if (writeResult && writeResult.success) {
                console.log("[IMPORT] Merged " + writeResult.merged + " saves into localStorage");
              } else {
                console.warn("[IMPORT] Failed to write saves:", writeResult && writeResult.error);
              }
            });
          }
        }

        // Wait for saves to be written, THEN refresh the library (which also uses bridge iframes)
        savesPromise.then(function () {
          var missingInfo = "";
          if (result.missing_defaults > 0) {
            missingInfo = '<br><span style="color:#e8a030;">' + result.missing_defaults +
              ' shared assets missing. Use "Update" on the case to try downloading them ' +
              '&mdash; some links may no longer work.</span>';
          }

          // Batch import (parent folder with multiple case subfolders)
          if (result.batch_manifests && result.batch_manifests.length > 1) {
            var totalAssets = 0;
            var totalBytes = 0;
            for (var i = 0; i < result.batch_manifests.length; i++) {
              totalAssets += result.batch_manifests[i].assets.total_downloaded;
              totalBytes += result.batch_manifests[i].assets.total_size_bytes;
            }
            var html = '<strong>' + result.batch_manifests.length + ' cases imported</strong> (' +
              totalAssets + ' assets, ' + formatBytes(totalBytes) + savesInfo + ')';
            if (result.batch_errors && result.batch_errors.length > 0) {
              html += '<br><span style="color:#e8a030;">' + result.batch_errors.length +
                ' case(s) skipped: ' + escapeHtml(result.batch_errors.join("; ")) + '</span>';
            }
            html += missingInfo;
            importResult.innerHTML = html;

            // Offer to create a collection from the imported batch
            showPromptModal(
              "Create a collection from these " + result.batch_manifests.length + " imported cases?",
              "Collection name",
              "Imported Cases",
              "Create",
              function (collectionName) {
                var batchSeqGroups = {};
                var batchStandalone = [];
                for (var bi = 0; bi < result.batch_manifests.length; bi++) {
                  var bm = result.batch_manifests[bi];
                  var bseq = bm.sequence;
                  if (bseq && bseq.title && bseq.list && bseq.list.length > 1) {
                    if (!batchSeqGroups[bseq.title]) batchSeqGroups[bseq.title] = true;
                  } else {
                    batchStandalone.push(bm.case_id);
                  }
                }
                var collItems = [];
                var batchSeqKeys = Object.keys(batchSeqGroups);
                for (var bsi = 0; bsi < batchSeqKeys.length; bsi++) {
                  collItems.push({ type: "sequence", title: batchSeqKeys[bsi] });
                }
                for (var bci = 0; bci < batchStandalone.length; bci++) {
                  collItems.push({ type: "case", case_id: batchStandalone[bci] });
                }
                invoke("create_collection", { title: collectionName, items: collItems })
                  .then(function () { loadLibrary(); })
                  .catch(function (e) { statusMsg.textContent = "Error creating collection: " + e; });
              }
            );
          } else {
            // Single case import
            importResult.innerHTML =
              '<strong>' + escapeHtml(manifest.title) + '</strong> by ' +
              escapeHtml(manifest.author) + ' &mdash; imported (' +
              manifest.assets.total_downloaded + ' assets, ' +
              formatBytes(manifest.assets.total_size_bytes) + savesInfo + ')' + missingInfo;
          }

          importResult.className = "result-success";
          progressBarInner.style.width = "100%";
          progressPhase.textContent = "Import complete!";
          loadLibrary();
        });
      })
      .catch(function (e) {
        importResult.textContent = "Import error: " + e;
        importResult.className = "result-error";
        progressContainer.classList.add("hidden");
      })
      .finally(function () {
        importFolderBtn.disabled = false;
        importFileBtn.disabled = false;
      });
  }

  importFolderBtn.addEventListener("click", function () {
    if (downloadInProgress) {
      importResult.textContent = "A download is already in progress.";
      importResult.className = "result-error";
      return;
    }

    invoke("pick_folder")
      .then(function (selected) {
        console.log("[IMPORT] pick_folder returned:", selected);
        if (!selected) return;
        doImport(selected);
      })
      .catch(function (e) {
        console.error("[IMPORT] pick_folder error:", e);
        importResult.textContent = "Could not open folder picker: " + e;
        importResult.className = "result-error";
      });
  });

  importFileBtn.addEventListener("click", function () {
    if (downloadInProgress) {
      importResult.textContent = "A download is already in progress.";
      importResult.className = "result-error";
      return;
    }

    invoke("pick_import_file")
      .then(function (selected) {
        console.log("[IMPORT] pick_import_file returned:", selected);
        if (!selected) return;
        if (selected.toLowerCase().endsWith(".aaosave")) {
          doImportSave(selected);
        } else if (selected.toLowerCase().endsWith(".aaoplug")) {
          doImportPlugin(selected);
        } else {
          doImport(selected);
        }
      })
      .catch(function (e) {
        console.error("[IMPORT] pick_import_file error:", e);
        importResult.textContent = "Could not open file picker: " + e;
        importResult.className = "result-error";
      });
  });

  importPasteSaveBtn.addEventListener("click", function () {
    showPasteSaveModal();
  });

  // --- Init ---
  loadLibrary();
  loadSettings();
});
