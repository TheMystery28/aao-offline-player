import { formatBytes, formatDate, escapeHtml, showFailedAssetsModal, showConfirmModal } from '../helpers.js';

/**
 * Render a collection group with header, items, and footer actions.
 */
export function appendCollectionGroup(ctx, collection, allCases, sequenceGroups, searchQuery) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;
  var caseList = ctx.caseList;

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
          ctx.pluginsPanel.classList.remove("hidden");
          ctx.pluginsToggle.classList.add("open");
          ctx.loadGlobalPluginsPanel();
          ctx.pluginsToggle.scrollIntoView({ behavior: "smooth" });
          return;
        }
        ctx.showScopedPluginModal("collection", col.id, 'Collection "' + col.title + '"');
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
      appendSequenceGroupInto(ctx, itemsContainer, item.title, sg.list, sg.cases, searchQuery);
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
      appendCaseCardInto(ctx, itemsContainer, allCases[item.case_id]);
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

  // Play from Part 1 -- play the first playable case across all items in order
  var firstPlayable = findFirstPlayableInCollection(ctx, items, allCases, sequenceGroups);
  if (firstPlayable) {
    var playFirstBtn = document.createElement("button");
    playFirstBtn.className = "play-btn";
    playFirstBtn.innerHTML = "&#9654; Play from Part 1";
    playFirstBtn.addEventListener("click", (function (c) {
      return function () { ctx.playCase(c.case_id, c.title); };
    })(firstPlayable));
    footer.appendChild(playFirstBtn);
  }

  // Continue button -- find latest save across all cases in the collection
  var allCollectionCaseIds = getCollectionCaseIds(ctx, items, allCases, sequenceGroups);
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
        ctx.findLastSequenceSave(fakeList).then(function (lastSave) {
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
              ctx.showPlayer(matchTitle, fullUrl);
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
    return function () { ctx.showEditCollectionModal(col); };
  })(collection));
  footer.appendChild(editBtn);

  // Export Collection button
  var exportColBtn = document.createElement("button");
  exportColBtn.className = "export-btn";
  exportColBtn.textContent = "Export Collection";
  exportColBtn.addEventListener("click", (function (col, caseIds) {
    return function () { exportCollection(ctx, col, caseIds); };
  })(collection, allCollectionCaseIds));
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
            .then(function () { ctx.loadLibrary(); })
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
export function appendSequenceGroupInto(ctx, container, sequenceTitle, sequenceList, downloadedCases, searchQuery) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

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
    ctx.appendSequencePart(partsContainer, partInfo, k + 1, downloaded);
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
        return function () { ctx.playCase(c.case_id, c.title); };
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
    seqFooter.appendChild(continueBtn);
  }

  if (missingIds.length > 0) {
    var dlBtn = document.createElement("button");
    dlBtn.className = "update-btn";
    dlBtn.textContent = "Download " + missingIds.length + " remaining";
    dlBtn.addEventListener("click", (function (ids, title) {
      return function () {
        if (ctx.downloadInProgress()) {
          statusMsg.textContent = "A download is already in progress.";
          return;
        }
        ctx.startSequenceDownload(ids, title);
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
export function appendCaseCardInto(ctx, container, c) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

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
    ctx.playCase(c.case_id, c.title);
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
    ctx.exportCase(c.case_id, c.title);
  });
  card.querySelector(".save-btn").addEventListener("click", function () {
    ctx.showSavesPluginsModal([c.case_id], c.title);
  });
  card.querySelector(".plugin-btn").addEventListener("click", function () {
    ctx.showPluginManagerModal(c.case_id, c.title);
  });
  card.querySelector(".delete-btn").addEventListener("click", function () {
    ctx.deleteCase(c.case_id, c.title);
  });

  container.appendChild(card);
}

export function findFirstPlayableInCollection(ctx, items, allCases, sequenceGroups) {
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

export function getCollectionCaseIds(ctx, items, allCases, sequenceGroups) {
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

export function exportCollection(ctx, collection, caseIds) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

  var safeName = collection.title.replace(/[^a-zA-Z0-9 _-]/g, "").trim();
  var defaultName = safeName + ".aaocase";
  statusMsg.textContent = "Choosing export location...";
  invoke("pick_export_file", { defaultName: defaultName })
    .then(function (destPath) {
      if (!destPath) {
        statusMsg.textContent = "";
        return;
      }
      // Smart prompts (centralized in saves.js)
      ctx.promptExportOptions(caseIds, function (saves, includePlugins) {
        ctx.withExportProgress("Exporting collection...", function (onEvent) {
          return invoke("export_collection", {
            collectionId: collection.id,
            destPath: destPath,
            saves: saves,
            includePlugins: includePlugins,
            onEvent: onEvent
          });
        }).then(function (size) {
          var msg = 'Exported collection "' + collection.title + '" (' + formatBytes(size) + ")";
          if (saves) msg += " with saves";
          statusMsg.textContent = msg;
        }).catch(function (e) {
          console.error("[MAIN] export collection error:", e);
          statusMsg.textContent = "Export error: " + e;
          ctx.progressContainer.classList.add("hidden");
        });
      });
    })
    .catch(function (e) {
      console.error("[MAIN] export collection error:", e);
      statusMsg.textContent = "Export error: " + e;
      ctx.progressContainer.classList.add("hidden");
    });
}
