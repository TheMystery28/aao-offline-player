import { formatBytes, escapeHtml, showUpdateModal, showConfirmModal } from './helpers.js';
import { buildSequenceGroupCore } from './collections/rendering.js';

/**
 * Initialise the Library section: case list rendering, search/sort, and case actions.
 *
 * @param {AppContext} ctx - Shared context bag. Functions from other modules
 *   (collections, plugins, download, saves, player, settings) are accessed
 *   through ctx at call-time, so they may be attached after this init runs.
 *
 * Required at init:
 *   invoke, Channel, statusMsg, caseList, emptyLibrary, libraryLoading
 *
 * Required at call-time (via ctx):
 *   showPlayer, findLastSequenceSave, showSavesPluginsModal, showExportOptionsModal,
 *   showPluginManagerModal, showScopedPluginModal, loadGlobalPluginsPanel,
 *   pluginsPanel, pluginsToggle, startUpdate, updateCase, retryCase,
 *   startSequenceDownload, downloadInProgress, copyTrialLink,
 *   progressContainer, progressPhase, progressBarInner, progressText,
 *   loadStorageInfo, appendCollectionGroup (from collections.js)
 */
export function initLibrary(ctx) {
  var invoke = ctx.invoke;
  var Channel = ctx.Channel;
  var statusMsg = ctx.statusMsg;
  var caseList = ctx.caseList;
  var emptyLibrary = ctx.emptyLibrary;
  var libraryLoading = ctx.libraryLoading;

  // DOM refs
  var librarySearch = document.getElementById("library-search");
  var librarySort = document.getElementById("library-sort");

  // State
  var cachedCases = [];
  var cachedCollections = [];

  // Coalesce rapid loadLibrary() calls via requestAnimationFrame.
  // Multiple calls in the same JS execution block result in a single refresh.
  var isLibraryRefreshScheduled = false;

  function loadLibrary() {
    if (isLibraryRefreshScheduled) return;
    isLibraryRefreshScheduled = true;
    requestAnimationFrame(function () {
      isLibraryRefreshScheduled = false;
      loadLibraryImpl();
    });
  }

  function loadLibraryImpl() {
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
        ctx.knownCaseIds.length = 0;
        for (var i = 0; i < cachedCases.length; i++) {
          ctx.knownCaseIds.push(cachedCases[i].case_id);
        }
        applySearchAndSort();
        ctx.loadStorageInfo();
        ctx.loadGlobalPluginsPanel();
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
    var sequenceGroups = {}; // title -> { list: [...], cases: [...] }
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
      ctx.appendCollectionGroup(collections[co], casesById, sequenceGroups, searchQuery);
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
    var core = buildSequenceGroupCore(ctx, sequenceTitle, sequenceList, downloadedCases, searchQuery);
    if (searchQuery && core.renderedParts === 0) {
      return;
    }
    var footer = core.footer;
    var downloadedIds = core.downloadedIds;

    // Library-specific footer buttons (not shown in collection view)

    // Update All button
    if (downloadedCases.length > 0) {
      var updateAllBtn = document.createElement("button");
      updateAllBtn.className = "update-btn";
      updateAllBtn.textContent = "Update All";
      updateAllBtn.addEventListener("click", (function (cases) {
        return function () {
          if (ctx.downloadInProgress()) {
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
                ctx.startUpdate(c.case_id, redownload, function () {
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
              // Smart prompts (centralized in saves.js)
              ctx.promptExportOptions(ids, function (saves, includePlugins) {
                withExportProgress("Exporting sequence...", function (onEvent) {
                  return invoke("export_sequence", {
                    caseIds: ids,
                    sequenceTitle: title,
                    sequenceList: list,
                    destPath: destPath,
                    saves: saves,
                    includePlugins: includePlugins,
                    onEvent: onEvent
                  });
                }).then(function (size) {
                  var msg = 'Exported "' + title + '" (' + formatBytes(size) + ")";
                  if (saves) msg += " with saves";
                  statusMsg.textContent = msg;
                }).catch(function (e) {
                  console.error("[MAIN] export sequence error:", e);
                  statusMsg.textContent = "Export error: " + e;
                  ctx.progressContainer.classList.add("hidden");
                });
              });
            });
        };
      })(downloadedIds, sequenceTitle, sequenceList));
      footer.appendChild(exportSeqBtn);
    }

    // Saves & Plugins button for entire sequence
    if (downloadedCases.length > 0) {
      var seqSavesBtn = document.createElement("button");
      seqSavesBtn.className = "save-btn";
      seqSavesBtn.textContent = "Saves";
      seqSavesBtn.title = "Saves & plugins for all parts";
      seqSavesBtn.addEventListener("click", (function (ids, title) {
        return function () { ctx.showSavesPluginsModal(ids, title); };
      })(downloadedIds, sequenceTitle));
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

    core.group.appendChild(footer);
    caseList.appendChild(core.group);
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
        return function () { ctx.updateCase(c.case_id); };
      })(manifest));
      actions.appendChild(updatePartBtn);

      if (failedCount > 0) {
        var retryPartBtn = document.createElement("button");
        retryPartBtn.className = "retry-btn";
        retryPartBtn.textContent = "Retry (" + failedCount + ")";
        retryPartBtn.title = "Retry failed assets (likely dead links — may not help)";
        retryPartBtn.addEventListener("click", (function (c) {
          return function () { ctx.retryCase(c.case_id, c.failed_assets); };
        })(manifest));
        actions.appendChild(retryPartBtn);
      }

      var linkPartBtn = document.createElement("button");
      linkPartBtn.className = "link-btn";
      linkPartBtn.textContent = "Link";
      linkPartBtn.title = "Copy AAO link";
      linkPartBtn.addEventListener("click", (function (id) {
        return function () { ctx.copyTrialLink(id); };
      })(manifest.case_id));
      actions.appendChild(linkPartBtn);

      var exportBtn = document.createElement("button");
      exportBtn.className = "export-btn";
      exportBtn.textContent = "Export";
      exportBtn.addEventListener("click", (function (c) {
        return function () { exportCase(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(exportBtn);

      var saveBtn = document.createElement("button");
      saveBtn.className = "save-btn";
      saveBtn.textContent = "Saves";
      saveBtn.title = "Saves & plugins";
      saveBtn.addEventListener("click", (function (c) {
        return function () { ctx.showSavesPluginsModal([c.case_id], c.title); };
      })(manifest));
      actions.appendChild(saveBtn);

      var pluginBtn = document.createElement("button");
      pluginBtn.className = "plugin-btn";
      pluginBtn.textContent = "Plugins";
      pluginBtn.title = "Manage plugins";
      pluginBtn.addEventListener("click", (function (c) {
        return function () { ctx.showPluginManagerModal(c.case_id, c.title); };
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
    ctx.appendCaseCardInto(caseList, c);
  }

  // --- Case Action Functions ---

  function playCase(caseId, title) {
    console.log("[MAIN] playCase caseId=" + caseId + " title=" + title);
    statusMsg.textContent = "Loading...";
    invoke("open_game", { caseId: caseId })
      .then(function (url) {
        console.log("[MAIN] open_game returned url=" + url);
        ctx.showPlayer(title, url);
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

  function withExportProgress(phaseLabel, exportFn) {
    ctx.progressContainer.classList.remove("hidden");
    ctx.progressPhase.textContent = phaseLabel;
    ctx.progressBarInner.style.width = "0%";
    ctx.progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      if (msg.event === "progress") {
        var pct = Math.round((msg.data.completed / msg.data.total) * 100);
        ctx.progressBarInner.style.width = pct + "%";
        ctx.progressText.textContent = msg.data.completed + " / " + msg.data.total + " files (" + pct + "%)";
      } else if (msg.event === "finished") {
        ctx.progressBarInner.style.width = "100%";
        ctx.progressPhase.textContent = "Export complete!";
        ctx.progressText.textContent = formatBytes(msg.data.total_bytes);
      }
    };

    return exportFn(onEvent);
  }

  function exportCase(caseId, title) {
    console.log("[EXPORT] exportCase called, caseId=" + caseId + " title=" + title);
    var safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
    var defaultName = safeName + ".aaocase";
    statusMsg.textContent = "Choosing export location...";
    invoke("pick_export_file", { defaultName: defaultName })
      .then(function (destPath) {
        if (!destPath) { statusMsg.textContent = ""; return; }

        // Smart prompts (centralized in saves.js)
        ctx.promptExportOptions([caseId], function (saves, includePlugins) {
          withExportProgress("Exporting...", function (onEvent) {
            return invoke("export_case", {
              caseId: caseId,
              destPath: destPath,
              saves: saves,
              includePlugins: includePlugins,
              onEvent: onEvent
            });
          }).then(function (size) {
            var msg = 'Exported "' + title + '" (' + formatBytes(size) + ")";
            if (saves) msg += " with saves";
            statusMsg.textContent = msg;
          }).catch(function (e) {
            console.error("[MAIN] export error:", e);
            statusMsg.textContent = "Export error: " + e;
            ctx.progressContainer.classList.add("hidden");
          });
        });
      })
      .catch(function (e) {
        console.error("[MAIN] export error:", e);
        statusMsg.textContent = "Export error: " + e;
        ctx.progressContainer.classList.add("hidden");
      });
  }

  return {
    loadLibrary: loadLibrary,
    getCachedCases: function () { return cachedCases; },
    getCachedCollections: function () { return cachedCollections; },
    appendSequencePart: appendSequencePart,
    playCase: playCase,
    deleteCase: deleteCase,
    exportCase: exportCase,
    withExportProgress: withExportProgress
  };
}
