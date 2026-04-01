import { escapeHtml, base64DecodeUtf8, showConfirmModal, formatBytes } from './helpers.js';

// --- Saves ---

var bridgeIdCounter = 0;

/** @param {AppContext} ctx */
export function initSaves(ctx) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;
  var loadLibrary = ctx.loadLibrary;

  var importResult = document.getElementById("import-result");

  function nextBridgeId(prefix) {
    return prefix + "_" + (++bridgeIdCounter);
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

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";
    cancelBtn.addEventListener("click", close);
    buttons.appendChild(cancelBtn);

    // Plugins section (live check — show only if case has active plugins)
    var pluginChecks = caseIds.map(function (id) { return invoke("list_plugins", { caseId: id }); });
    Promise.all(pluginChecks).then(function (pluginStates) {
      var anyPlugins = pluginStates.some(function (ps) {
        return ps.scripts.length > 0 || ps.disabled.length > 0;
      });
      if (!anyPlugins) return;

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

      buttons.insertBefore(pluginsLabel, cancelBtn);
      buttons.insertBefore(exportPluginsBtn, cancelBtn);
    });

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
    var pluginChecks = caseIds.map(function (id) { return invoke("list_plugins", { caseId: id }); });
    Promise.all([
      invoke("read_saves_for_export", { caseIds: caseIds }),
      Promise.all(pluginChecks)
    ]).then(function (results) {
      var saves = results[0];
      var pluginStates = results[1];
      if (!saves) {
        statusMsg.textContent = "No saves found for " + (caseIds.length > 1 ? "these cases" : "this case") + ".";
        return;
      }
      var anyHasPlugins = pluginStates.some(function (ps) {
        return ps.scripts.length > 0 || ps.disabled.length > 0;
      });
      if (!anyHasPlugins) {
        doExportSave(caseIds, title, saves, false);
      } else {
        showConfirmModal(
          "Include plugins in save export?",
          "Include Plugins",
          function () { doExportSave(caseIds, title, saves, true); },
          function () { doExportSave(caseIds, title, saves, false); }
        );
      }
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

  // --- Save Bridge Helpers ---

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

  return {
    readGameSaves: readGameSaves,
    writeGameSaves: writeGameSaves,
    findLastSequenceSave: findLastSequenceSave,
    findLastSequenceSaveBridge: findLastSequenceSaveBridge,
    exportSave: exportSave,
    showSavesPluginsModal: showSavesPluginsModal,
    showExportOptionsModal: showExportOptionsModal,
    showPasteSaveModal: showPasteSaveModal,
    doImportSave: doImportSave,
    doExportSave: doExportSave,
    copyTrialLink: copyTrialLink,
    nextBridgeId: nextBridgeId
  };
}
