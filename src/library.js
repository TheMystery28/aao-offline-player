import { formatBytes, escapeHtml, showUpdateModal, showConfirmModal, groupCasesBySequence, applySpoilerBlur } from './helpers.js';
import { buildSequenceGroupCore } from './collections/rendering.js';

// ── Asset gallery helpers ──────────────────────────────────────────────────

const IMAGE_EXTS = ['gif', 'png', 'jpg', 'jpeg', 'webp', 'bmp', 'svg'];
const AUDIO_EXTS = ['mp3', 'ogg', 'wav', 'aac', 'm4a', 'flac'];

function getExt(path) { return (path.split('.').pop() || '').toLowerCase(); }
function isImage(path) { return IMAGE_EXTS.indexOf(getExt(path)) !== -1; }
function isAudio(path) { return AUDIO_EXTS.indexOf(getExt(path)) !== -1; }

export function switchInspectTab(name) {
  const tabs = ['images', 'audio', 'failed'];
  tabs.forEach(function (t) {
    const tabBtn = document.querySelector('.inspect-tab[data-tab="' + t + '"]');
    const panel = document.getElementById('inspect-' + t + '-panel');
    if (tabBtn) tabBtn.classList.toggle('active', t === name);
    if (panel) panel.classList.toggle('hidden', t !== name);
  });
}

export async function showInspectModal(manifest, serverUrl, runtimeFailures) {
  const images = [];
  const audio = [];
  const assetMap = manifest.asset_map || {};

  Object.keys(assetMap).forEach(function (localUrl) {
    const localPath = assetMap[localUrl];
    // Mirror Rust case_relative(): assets/* need case/{id}/ prefix; defaults/* serve as-is
    const serverPath = localPath.indexOf('assets/') === 0
      ? 'case/' + manifest.case_id + '/' + localPath
      : localPath;
    const fullUrl = serverUrl + '/' + serverPath;
    const name = localPath.split('/').pop();
    if (isImage(localPath)) {
      images.push({ url: fullUrl, name: name });
    } else if (isAudio(localPath)) {
      audio.push({ url: fullUrl, name: name });
    }
  });

  // Combine download failures with assets missing from disk
  const downloadFailed = manifest.failed_assets || [];
  let missingAssets = [];
  try {
    missingAssets = await window.__TAURI__.core.invoke('get_missing_assets', { caseId: manifest.case_id });
  } catch (e) {
    console.warn('[Inspect] Failed to check missing assets:', e);
  }
  // Merge: download failures + disk-missing + runtime load failures
  const runtimeEntries = (runtimeFailures || []).map(function (r) {
    return { url: r, error: 'Failed to load at runtime' };
  });
  const failed = downloadFailed.concat(
    missingAssets.map(function (m) {
      return { url: m.url, error: 'File missing from disk: ' + m.local_path };
    })
  ).concat(runtimeEntries);
  // Deduplicate by url
  const seenUrls = {};
  const dedupedFailed = [];
  for (let i = 0; i < failed.length; i++) {
    if (!seenUrls[failed[i].url]) {
      seenUrls[failed[i].url] = true;
      dedupedFailed.push(failed[i]);
    }
  }

  // Update title and counts
  const titleEl = document.getElementById('inspect-modal-title');
  if (titleEl) titleEl.textContent = manifest.title + ' \u2014 Assets';
  const imgCount = document.getElementById('inspect-image-count');
  if (imgCount) imgCount.textContent = images.length;
  const audCount = document.getElementById('inspect-audio-count');
  if (audCount) audCount.textContent = audio.length;
  const failCount = document.getElementById('inspect-failed-count');
  if (failCount) failCount.textContent = dedupedFailed.length;

  // Render images
  const imgGrid = document.getElementById('inspect-image-grid');
  if (imgGrid) {
    imgGrid.innerHTML = '';
    images.forEach(function (img) {
      const el = document.createElement('img');
      el.className = 'inspect-image-item';
      el.src = img.url;
      el.title = img.name;
      el.alt = img.name;
      el.loading = 'lazy';
      applySpoilerBlur(el);
      imgGrid.appendChild(el);
    });
  }

  // Render audio
  const audioList = document.getElementById('inspect-audio-list');
  if (audioList) {
    audioList.innerHTML = '';
    audio.forEach(function (aud) {
      const row = document.createElement('div');
      row.className = 'inspect-audio-row';
      const label = document.createElement('span');
      label.className = 'inspect-audio-name';
      label.textContent = aud.name;
      applySpoilerBlur(label);
      const player = document.createElement('audio');
      player.controls = true;
      player.src = aud.url;
      row.appendChild(label);
      row.appendChild(player);
      audioList.appendChild(row);
    });
  }

  // Render failed
  const failedList = document.getElementById('inspect-failed-list');
  if (failedList) {
    failedList.innerHTML = '';
    if (dedupedFailed.length === 0) {
      failedList.innerHTML = '<p class="muted">No failed downloads.</p>';
    } else {
      dedupedFailed.forEach(function (f) {
        const row = document.createElement('div');
        row.className = 'inspect-failed-row';
        const urlDiv = document.createElement('div');
        urlDiv.className = 'inspect-failed-url';
        urlDiv.textContent = f.url;
        const errDiv = document.createElement('div');
        errDiv.className = 'inspect-failed-error';
        errDiv.textContent = f.error;
        row.appendChild(urlDiv);
        row.appendChild(errDiv);
        failedList.appendChild(row);
      });
    }
  }

  // Activate images tab by default and show modal
  switchInspectTab('images');
  const modal = document.getElementById('inspect-modal');
  if (modal) modal.classList.remove('hidden');
}

// ──────────────────────────────────────────────────────────────────────────

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
  const invoke = ctx.invoke;
  const Channel = ctx.Channel;
  const statusMsg = ctx.statusMsg;
  const caseList = ctx.caseList;
  const emptyLibrary = ctx.emptyLibrary;
  const libraryLoading = ctx.libraryLoading;

  // DOM refs
  const librarySearch = document.getElementById("library-search");

  // State
  let cachedCases = [];
  let cachedCollections = [];

  // Coalesce rapid loadLibrary() calls via microtask.
  // Multiple calls in the same JS execution block result in a single refresh.
  // Uses queueMicrotask instead of requestAnimationFrame because rAF is paused
  // on Android WebView during heavy async operations (downloads), which blocks
  // the UI from updating until a user interaction forces a paint.
  // queueMicrotask runs immediately after the current synchronous code finishes,
  // before yielding to the event loop for rendering — no stale-frame risk.
  let isLibraryRefreshScheduled = false;

  function loadLibrary() {
    if (isLibraryRefreshScheduled) return;
    isLibraryRefreshScheduled = true;
    queueMicrotask(function () {
      isLibraryRefreshScheduled = false;
      loadLibraryImpl();
    }, 0);
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
        for (let i = 0; i < cachedCases.length; i++) {
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
    const query = (librarySearch.value || "").trim().toLowerCase();

    let filtered = cachedCases;
    if (query) {
      filtered = cachedCases.filter(function (c) {
        return c.title.toLowerCase().indexOf(query) !== -1 ||
               c.author.toLowerCase().indexOf(query) !== -1 ||
               String(c.case_id).indexOf(query) !== -1;
      });
    }

    // Default sort: name A-Z
    const sorted = filtered.slice();
    sorted.sort(function (a, b) { return a.title.localeCompare(b.title); });

    renderCaseList(sorted, cachedCollections, query);
  }

  let searchDebounceTimer = null;
  librarySearch.addEventListener("input", function () {
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(applySearchAndSort, 200);
  });

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
    const casesById = {};
    for (let ci = 0; ci < cases.length; ci++) {
      casesById[cases[ci].case_id] = cases[ci];
    }

    // Group cases by sequence title
    const grouped = groupCasesBySequence(cases);
    const sequenceGroups = grouped.sequenceGroups;
    const standalone = grouped.standalone;

    // Sort sequence groups
    const groupKeys = Object.keys(sequenceGroups);
    for (let g = 0; g < groupKeys.length; g++) {
      const groupTitle = groupKeys[g];
      const group = sequenceGroups[groupTitle];
      const listIds = group.list.map(function (p) { return p.id; });
      group.cases.sort(function (a, b) {
        return listIds.indexOf(a.case_id) - listIds.indexOf(b.case_id);
      });
    }

    // Determine which items are claimed by a collection
    const claimedCaseIds = {};
    const claimedSequenceTitles = {};
    for (let col = 0; col < collections.length; col++) {
      const items = collections[col].items || [];
      for (let it = 0; it < items.length; it++) {
        if (items[it].type === "case") {
          claimedCaseIds[items[it].case_id] = true;
        } else if (items[it].type === "sequence") {
          claimedSequenceTitles[items[it].title] = true;
        }
      }
    }

    // Render collections first
    for (let co = 0; co < collections.length; co++) {
      ctx.appendCollectionGroup(collections[co], casesById, sequenceGroups, searchQuery);
    }

    // Render remaining uncollected sequences
    for (let gs = 0; gs < groupKeys.length; gs++) {
      if (!claimedSequenceTitles[groupKeys[gs]]) {
        appendSequenceGroup(groupKeys[gs], sequenceGroups[groupKeys[gs]].list, sequenceGroups[groupKeys[gs]].cases, searchQuery);
      }
    }

    // Render remaining uncollected standalone cases
    for (let s = 0; s < standalone.length; s++) {
      if (!claimedCaseIds[standalone[s].case_id]) {
        appendCaseCard(standalone[s]);
      }
    }
  }

  function appendSequenceGroup(sequenceTitle, sequenceList, downloadedCases, searchQuery) {
    const core = buildSequenceGroupCore(ctx, sequenceTitle, sequenceList, downloadedCases, searchQuery);
    if (searchQuery && core.renderedParts === 0) {
      return;
    }
    const footer = core.footer;
    const downloadedIds = core.downloadedIds;

    // Library-specific footer buttons (not shown in collection view)

    // Update All button
    if (downloadedCases.length > 0) {
      const updateAllBtn = document.createElement("button");
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
              const redownload = (choice === 2);

              // Update each part sequentially
              let idx = 0;
              function updateNext() {
                if (idx >= cases.length) {
                  loadLibrary();
                  statusMsg.textContent = "All " + cases.length + " parts updated.";
                  return;
                }
                const c = cases[idx];
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
      const exportSeqBtn = document.createElement("button");
      exportSeqBtn.className = "export-btn";
      exportSeqBtn.textContent = "Export Sequence";
      exportSeqBtn.addEventListener("click", (function (ids, title, list) {
        return function () {
          const safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
          const defaultName = safeName + ".aaocase";
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
                  let msg = 'Exported "' + title + '" (' + formatBytes(size) + ")";
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
      const seqSavesBtn = document.createElement("button");
      seqSavesBtn.className = "save-btn";
      seqSavesBtn.textContent = "Saves";
      seqSavesBtn.title = "Saves & plugins for all parts";
      seqSavesBtn.addEventListener("click", (function (ids, title) {
        return function () { ctx.showSavesPluginsModal(ids, title); };
      })(downloadedIds, sequenceTitle));
      footer.appendChild(seqSavesBtn);
    }

    // Delete all button
    const delAllBtn = document.createElement("button");
    delAllBtn.className = "delete-btn";
    delAllBtn.textContent = "Delete All";
    delAllBtn.addEventListener("click", (function (cases, title) {
      return function () {
        showConfirmModal(
          'Delete all ' + cases.length + ' parts of "' + title + '"?\nThis cannot be undone.',
          "Delete All",
          function () {
            const deletePromises = cases.map(function (c) {
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
    const row = document.createElement("div");
    row.className = "sequence-part" + (manifest ? "" : " sequence-part-missing");

    if (manifest) {
      const sizeStr = formatBytes(manifest.assets.total_size_bytes);
      const failedCount = manifest.failed_assets ? manifest.failed_assets.length : 0;
      row.innerHTML =
        '<span class="sequence-part-info">' +
          '<span class="sequence-part-num">' + partNum + '.</span> ' +
          escapeHtml(manifest.title) +
          '<span class="muted"> &middot; ' + manifest.assets.total_downloaded + ' assets (' + sizeStr + ')' +
          (failedCount > 0 ? ' &middot; <span class="case-failed">' + failedCount + ' failed</span>' : '') +
          (manifest.has_plugins ? ' &middot; <span class="case-plugins">Plugins</span>' : '') +
          '</span>' +
        '</span>';

      const actions = document.createElement("span");
      actions.className = "sequence-part-actions";

      const playBtn = document.createElement("button");
      playBtn.className = "play-btn";
      playBtn.innerHTML = "&#9654;";
      playBtn.title = "Play this part";
      playBtn.addEventListener("click", (function (c) {
        return function () { playCase(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(playBtn);

      const updatePartBtn = document.createElement("button");
      updatePartBtn.className = "update-btn";
      updatePartBtn.textContent = "Update";
      updatePartBtn.addEventListener("click", (function (c) {
        return function () { ctx.updateCase(c.case_id); };
      })(manifest));
      actions.appendChild(updatePartBtn);

      if (failedCount > 0) {
        const retryPartBtn = document.createElement("button");
        retryPartBtn.className = "retry-btn";
        retryPartBtn.textContent = "Retry (" + failedCount + ")";
        retryPartBtn.title = "Retry failed assets (likely dead links — may not help)";
        retryPartBtn.addEventListener("click", (function (c) {
          return function () { ctx.retryCase(c.case_id, c.failed_assets); };
        })(manifest));
        actions.appendChild(retryPartBtn);
      }

      const linkPartBtn = document.createElement("button");
      linkPartBtn.className = "link-btn";
      linkPartBtn.textContent = "Link";
      linkPartBtn.title = "Copy AAO link";
      linkPartBtn.addEventListener("click", (function (id) {
        return function () { ctx.copyTrialLink(id); };
      })(manifest.case_id));
      actions.appendChild(linkPartBtn);

      const exportBtn = document.createElement("button");
      exportBtn.className = "export-btn";
      exportBtn.textContent = "Export";
      exportBtn.addEventListener("click", (function (c) {
        return function () { exportCase(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(exportBtn);

      const saveBtn = document.createElement("button");
      saveBtn.className = "save-btn";
      saveBtn.textContent = "Saves";
      saveBtn.title = "Saves & plugins";
      saveBtn.addEventListener("click", (function (c) {
        return function () { ctx.showSavesPluginsModal([c.case_id], c.title); };
      })(manifest));
      actions.appendChild(saveBtn);

      const pluginBtn = document.createElement("button");
      pluginBtn.className = "plugin-btn";
      pluginBtn.textContent = "Plugins";
      pluginBtn.title = "Manage plugins";
      pluginBtn.addEventListener("click", (function (c) {
        return function () { ctx.showPluginManagerModal(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(pluginBtn);

      const inspectPartBtn = document.createElement("button");
      inspectPartBtn.className = "inspect-btn small-btn";
      inspectPartBtn.textContent = "Inspect";
      inspectPartBtn.title = "Browse case assets";
      inspectPartBtn.addEventListener("click", (function (m) {
        return function () {
          invoke("get_server_url").then(function (serverUrl) {
            showInspectModal(m, serverUrl, ctx.getRuntimeFailedAssets ? ctx.getRuntimeFailedAssets() : []);
          });
        };
      })(manifest));
      actions.appendChild(inspectPartBtn);

      const deleteBtn = document.createElement("button");
      deleteBtn.className = "delete-btn";
      deleteBtn.textContent = "Delete";
      deleteBtn.addEventListener("click", (function (c) {
        return function () { deleteCase(c.case_id, c.title); };
      })(manifest));
      actions.appendChild(deleteBtn);

      // ARIA labels for screen readers
      const partBtns = actions.querySelectorAll("button");
      for (let pb = 0; pb < partBtns.length; pb++) {
        const label = partBtns[pb].textContent.replace(/[^a-zA-Z ]/g, "").trim();
        partBtns[pb].setAttribute("aria-label", label + " " + manifest.title);
      }

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

    const onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      if (msg.event === "progress") {
        const pct = Math.round((msg.data.completed / msg.data.total) * 100);
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
    const safeName = title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
    const defaultName = safeName + ".aaocase";
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
            let msg = 'Exported "' + title + '" (' + formatBytes(size) + ")";
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

  // Wire up inspect modal event listeners (once, at init time)
  (function initInspectModal() {
    const modal = document.getElementById('inspect-modal');
    const closeBtn = document.getElementById('inspect-close-btn');
    if (!modal || !closeBtn) return;

    // Tab switching
    modal.querySelectorAll('.inspect-tab').forEach(function (btn) {
      btn.addEventListener('click', function () {
        switchInspectTab(btn.getAttribute('data-tab'));
      });
    });

    // Close button
    closeBtn.addEventListener('click', function () {
      modal.classList.add('hidden');
    });

    // Click outside modal content to close
    modal.addEventListener('click', function (e) {
      if (e.target === modal) modal.classList.add('hidden');
    });

    // Escape key to close
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && !modal.classList.contains('hidden')) {
        modal.classList.add('hidden');
      }
    });
  })();

  return {
    loadLibrary: loadLibrary,
    getCachedCases: function () { return cachedCases; },
    getCachedCollections: function () { return cachedCollections; },
    appendSequencePart: appendSequencePart,
    playCase: playCase,
    deleteCase: deleteCase,
    exportCase: exportCase,
    withExportProgress: withExportProgress,
    showInspectModal: showInspectModal
  };
}
