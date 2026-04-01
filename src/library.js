import { formatBytes, formatDate, escapeHtml, showFailedAssetsModal, showUpdateModal, showConfirmModal } from './helpers.js';

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
            ctx.pluginsPanel.classList.remove("hidden");
            ctx.pluginsToggle.classList.add("open");
            ctx.loadGlobalPluginsPanel();
            ctx.pluginsToggle.scrollIntoView({ behavior: "smooth" });
            return;
          }
          ctx.showScopedPluginModal("sequence", title, 'Sequence "' + title + '"');
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
          ctx.findLastSequenceSave(seqList).then(function (lastSave) {
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
                ctx.showPlayer(matchTitle, fullUrl);
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
          if (ctx.downloadInProgress()) {
            statusMsg.textContent = "A download is already in progress.";
            return;
          }
          ctx.startSequenceDownload(ids, title);
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
              ctx.progressContainer.classList.remove("hidden");
              ctx.progressPhase.textContent = "Exporting sequence...";
              ctx.progressBarInner.style.width = "0%";
              ctx.progressText.textContent = "";

              var onEvent = new Channel();
              onEvent.onmessage = function (msg) {
                if (msg.event === "progress") {
                  var pct = Math.round((msg.data.completed / msg.data.total) * 100);
                  ctx.progressBarInner.style.width = pct + "%";
                  ctx.progressText.textContent =
                    msg.data.completed + " / " + msg.data.total + " files (" + pct + "%)";
                } else if (msg.event === "finished") {
                  ctx.progressBarInner.style.width = "100%";
                  ctx.progressPhase.textContent = "Export complete!";
                  ctx.progressText.textContent = formatBytes(msg.data.total_bytes);
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
                  ctx.progressContainer.classList.add("hidden");
                });
              }

              // Smart prompts (live check for plugins across all cases)
              var pluginChecks = ids.map(function (id) { return invoke("list_plugins", { caseId: id }); });
              Promise.all([
                invoke("read_saves_for_export", { caseIds: ids }),
                Promise.all(pluginChecks)
              ]).then(function (results) {
                var saves = results[0];
                var pluginStates = results[1];
                var hasSaves = saves !== null;
                var seqHasPlugins = pluginStates.some(function (ps) {
                  return ps.scripts.length > 0 || ps.disabled.length > 0;
                });
                if (!hasSaves && !seqHasPlugins) {
                  doSeqExport(null, false);
                } else if (hasSaves && !seqHasPlugins) {
                  showConfirmModal("Include saves?", "Include Saves",
                    function () { doSeqExport(saves, false); },
                    function () { doSeqExport(null, false); });
                } else if (!hasSaves && seqHasPlugins) {
                  showConfirmModal("Include plugins?", "Include Plugins",
                    function () { doSeqExport(null, true); },
                    function () { doSeqExport(null, false); });
                } else {
                  ctx.showExportOptionsModal(function (incSaves, incPlugins) {
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
        return function () { ctx.showSavesPluginsModal(ids, title, hasPlug); };
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
        return function () { exportCase(c.case_id, c.title, c.has_plugins); };
      })(manifest));
      actions.appendChild(exportBtn);

      var saveBtn = document.createElement("button");
      saveBtn.className = "save-btn";
      saveBtn.textContent = "Saves";
      saveBtn.title = "Saves & plugins";
      saveBtn.addEventListener("click", (function (c) {
        return function () { ctx.showSavesPluginsModal([c.case_id], c.title, c.has_plugins); };
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
        ctx.findLastSequenceSave([{ id: caseId }]).then(function (lastSave) {
          if (!lastSave) {
            statusMsg.textContent = "No saves found for this case.";
            return;
          }
          statusMsg.textContent = 'Resuming "' + caseTitle + '"...';
          invoke("open_game", { caseId: caseId })
            .then(function (url) {
              var sep = url.indexOf("?") === -1 ? "?" : "&";
              var fullUrl = url + sep + "save_data=" + encodeURIComponent(lastSave.saveDataBase64);
              ctx.showPlayer(caseTitle, fullUrl);
            })
            .catch(function (e) { statusMsg.textContent = "Error: " + e; });
        });
      });
    })(c.case_id, c.title);

    card.querySelector(".update-btn").addEventListener("click", function () {
      ctx.updateCase(c.case_id);
    });

    var retryBtn = card.querySelector(".retry-btn");
    if (retryBtn) {
      retryBtn.addEventListener("click", function () {
        ctx.retryCase(c.case_id, c.failed_assets);
      });
    }

    var failedSpan = card.querySelector(".case-failed");
    if (failedSpan && c.failed_assets) {
      failedSpan.addEventListener("click", (function (fa) {
        return function (e) { e.stopPropagation(); showFailedAssetsModal(fa); };
      })(c.failed_assets));
    }

    card.querySelector(".link-btn").addEventListener("click", function () {
      ctx.copyTrialLink(c.case_id);
    });

    card.querySelector(".export-btn").addEventListener("click", function () {
      exportCase(c.case_id, c.title, c.has_plugins);
    });

    card.querySelector(".save-btn").addEventListener("click", function () {
      ctx.showSavesPluginsModal([c.case_id], c.title, c.has_plugins);
    });

    card.querySelector(".plugin-btn").addEventListener("click", function () {
      ctx.showPluginManagerModal(c.case_id, c.title);
    });

    card.querySelector(".delete-btn").addEventListener("click", function () {
      deleteCase(c.case_id, c.title);
    });

    caseList.appendChild(card);
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

  function exportCase(caseId, title, hasPlugins) {
    console.log("[EXPORT] exportCase called, caseId=" + caseId + " title=" + title);
    var safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
    var defaultName = safeName + ".aaocase";
    statusMsg.textContent = "Choosing export location...";
    invoke("pick_export_file", { defaultName: defaultName })
      .then(function (destPath) {
        if (!destPath) { statusMsg.textContent = ""; return; }

        ctx.progressContainer.classList.remove("hidden");
        ctx.progressPhase.textContent = "Exporting...";
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
            ctx.progressContainer.classList.add("hidden");
          });
        }

        // Smart prompts: only ask about what exists (live check for plugins)
        Promise.all([
          invoke("read_saves_for_export", { caseIds: [caseId] }),
          invoke("list_plugins", { caseId: caseId })
        ]).then(function (results) {
          var saves = results[0];
          var pluginState = results[1];
          var hasSaves = saves !== null;
          var caseHasPlugins = pluginState.scripts.length > 0 || pluginState.disabled.length > 0;
          if (!hasSaves && !caseHasPlugins) {
            doExport(null, false);
          } else if (hasSaves && !caseHasPlugins) {
            showConfirmModal("Include game saves?", "Include Saves",
              function () { doExport(saves, false); },
              function () { doExport(null, false); });
          } else if (!hasSaves && caseHasPlugins) {
            showConfirmModal("Include plugins?", "Include Plugins",
              function () { doExport(null, true); },
              function () { doExport(null, false); });
          } else {
            ctx.showExportOptionsModal(function (incSaves, incPlugins) {
              doExport(incSaves ? saves : null, incPlugins);
            });
          }
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
    exportCase: exportCase
  };
}
