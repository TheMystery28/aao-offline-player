import { formatBytes, escapeHtml, showPromptModal } from './helpers.js';

/**
 * Initialise the Import section UI and event listeners.
 *
 * @param {object} ctx - Context bag from the main closure:
 *   invoke, Channel, statusMsg, loadLibrary, showPasteSaveModal,
 *   writeGameSaves, doImportSave, doImportPlugin,
 *   progressContainer, progressPhase, progressBarInner, progressText,
 *   isDownloadInProgress  (getter function returning boolean)
 */
export function initImport(ctx) {
  var invoke = ctx.invoke;
  var Channel = ctx.Channel;
  var statusMsg = ctx.statusMsg;
  var loadLibrary = ctx.loadLibrary;
  var showPasteSaveModal = ctx.showPasteSaveModal;
  var writeGameSaves = ctx.writeGameSaves;
  var doImportSave = ctx.doImportSave;
  var doImportPlugin = ctx.doImportPlugin;
  var progressContainer = ctx.progressContainer;
  var progressPhase = ctx.progressPhase;
  var progressBarInner = ctx.progressBarInner;
  var progressText = ctx.progressText;
  var isDownloadInProgress = ctx.isDownloadInProgress;

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
    if (isDownloadInProgress()) {
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
    if (isDownloadInProgress()) {
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
}
