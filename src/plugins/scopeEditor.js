import { escapeHtml } from '../helpers.js';

/**
 * Scope Editor Modal — shows all per-scope overrides for a single plugin.
 * If globally enabled: shows disabled_for exceptions.
 * If globally disabled: shows enabled_for overrides.
 */
export function showScopeEditorModal(ctx, pluginFilename) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;
  var getCachedCases = ctx.getCachedCases;
  var getCachedCollections = ctx.getCachedCollections;

  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog modal-dialog-wide";

  var titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.innerHTML = "<strong>Scope &mdash; " + escapeHtml(pluginFilename) + "</strong>";

  var contentEl = document.createElement("div");
  contentEl.style.cssText = "margin: 10px 0; max-height: 400px; overflow-y: auto;";

  function close() {
    document.body.removeChild(overlay);
    ctx.loadGlobalPluginsPanel();
  }

  function resolveCollectionTitle(colId) {
    var cols = getCachedCollections();
    for (var i = 0; i < (cols || []).length; i++) {
      if (cols[i].id === colId) return cols[i].title;
    }
    return colId;
  }

  function resolveCaseTitle(caseId) {
    var cases = getCachedCases();
    var id = Number(caseId);
    for (var i = 0; i < (cases || []).length; i++) {
      if (cases[i].case_id === id) return cases[i].title;
    }
    return "Case " + caseId;
  }

  function refreshScopeEditor() {
    invoke("list_global_plugins").then(function (manifest) {
      var disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
      var plugins = (manifest && manifest.plugins) || {};
      var entry = plugins[pluginFilename] || {};
      var globallyDisabled = disabledList.indexOf(pluginFilename) !== -1;

      contentEl.innerHTML = "";

      // Global toggle
      var globalRow = document.createElement("div");
      globalRow.style.cssText = "display:flex; align-items:center; gap:0.5rem; margin-bottom:0.75rem; padding-bottom:0.5rem; border-bottom:1px solid #2a2a4a;";
      var globalToggle = document.createElement("input");
      globalToggle.type = "checkbox";
      globalToggle.checked = !globallyDisabled;
      globalToggle.style.accentColor = "#4a90d9";
      globalToggle.style.width = "1rem";
      globalToggle.style.height = "1rem";
      globalToggle.addEventListener("change", function () {
        invoke("toggle_global_plugin", { filename: pluginFilename, enabled: globalToggle.checked })
          .then(refreshScopeEditor)
          .catch(function (e) { statusMsg.textContent = "Error: " + e; });
      });
      var globalLabelEl = document.createElement("span");
      globalLabelEl.style.cssText = "color:#ccc; font-size:0.9rem;";
      globalLabelEl.textContent = "Globally " + (globallyDisabled ? "disabled" : "enabled");
      globalRow.appendChild(globalToggle);
      globalRow.appendChild(globalLabelEl);
      contentEl.appendChild(globalRow);

      // Section label
      var sectionLabel = document.createElement("div");
      sectionLabel.style.cssText = "color:#999; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.04em; margin-bottom:0.35rem;";
      if (globallyDisabled) {
        sectionLabel.textContent = "Enabled for (overrides)";
      } else {
        sectionLabel.textContent = "Disabled for (exceptions)";
      }
      contentEl.appendChild(sectionLabel);

      // Build override list
      var overrideField = globallyDisabled ? "enabled_for" : "disabled_for";
      var overrides = entry[overrideField] || {};
      var overrideItems = [];

      var colArr = overrides.collections || [];
      for (var ci = 0; ci < colArr.length; ci++) {
        overrideItems.push({ type: "collection", key: String(colArr[ci]), label: "Collection: " + resolveCollectionTitle(String(colArr[ci])) });
      }
      var seqArr = overrides.sequences || [];
      for (var si = 0; si < seqArr.length; si++) {
        overrideItems.push({ type: "sequence", key: String(seqArr[si]), label: "Sequence: " + seqArr[si] });
      }
      var caseArr = overrides.cases || [];
      for (var cai = 0; cai < caseArr.length; cai++) {
        overrideItems.push({ type: "case", key: String(caseArr[cai]), label: "Case: " + resolveCaseTitle(caseArr[cai]) });
      }

      if (overrideItems.length === 0) {
        var emptyMsg = document.createElement("div");
        emptyMsg.className = "muted";
        emptyMsg.style.cssText = "font-size:0.82rem; padding:0.3rem 0;";
        emptyMsg.textContent = "No per-scope overrides.";
        contentEl.appendChild(emptyMsg);
      } else {
        for (var oi = 0; oi < overrideItems.length; oi++) {
          (function (item) {
            var row = document.createElement("div");
            row.className = "global-plugin-row";
            var label = document.createElement("span");
            label.className = "plugin-name";
            label.textContent = item.label;
            var removeBtn = document.createElement("button");
            removeBtn.className = "plugin-remove-btn";
            removeBtn.textContent = "Remove";
            removeBtn.addEventListener("click", function () {
              // Remove override: if globally enabled, re-enable for this scope (removes from disabled_for)
              // If globally disabled, re-disable for this scope (removes from enabled_for)
              invoke("toggle_plugin_for_scope", {
                filename: pluginFilename,
                scopeType: item.type,
                scopeKey: item.key,
                enabled: !globallyDisabled
              }).then(refreshScopeEditor)
                .catch(function (e) { statusMsg.textContent = "Error: " + e; });
            });
            row.appendChild(label);
            row.appendChild(removeBtn);
            contentEl.appendChild(row);
          })(overrideItems[oi]);
        }
      }

      // Add override button + inline picker
      var addBtnLabel = globallyDisabled ? "+ Enable for Scope" : "+ Add Exception";
      var addBtn = document.createElement("button");
      addBtn.className = "small-btn";
      addBtn.textContent = addBtnLabel;
      addBtn.style.cssText = "margin-top:0.5rem; font-size:0.78rem;";

      var pickerContainer = document.createElement("div");
      pickerContainer.style.cssText = "max-height:180px; overflow-y:auto; padding:0.3rem 0; display:none; margin-top:0.3rem;";

      var pickerBuilt = false;
      addBtn.addEventListener("click", function () {
        if (pickerContainer.style.display === "none") {
          pickerContainer.style.display = "block";
          addBtn.textContent = "Hide Picker";
          if (!pickerBuilt) {
            pickerBuilt = true;
            buildPicker(overrideField, overrides, globallyDisabled);
          }
        } else {
          pickerContainer.style.display = "none";
          addBtn.textContent = addBtnLabel;
        }
      });

      function buildPicker(field, currentOverrides, isGloballyDisabled) {
        pickerContainer.innerHTML = "";

        // Use cached data (already populated by loadLibrary)
        var cases = getCachedCases() || [];
        var cols = getCachedCollections() || [];

        function renderPicker(cases, cols) {
          pickerContainer.innerHTML = "";
          var collectionCaseIds = {};
          var collectionSeqTitles = {};
          for (var ci = 0; ci < cols.length; ci++) {
            var colItems = cols[ci].items || [];
            for (var ii = 0; ii < colItems.length; ii++) {
              if (colItems[ii].type === "case") collectionCaseIds[colItems[ii].case_id] = true;
              if (colItems[ii].type === "sequence") collectionSeqTitles[colItems[ii].title] = true;
            }
          }

          var seqTitles = [];
          var seenSeqs = {};
          var sequenceCaseIds = {};
          for (var si = 0; si < cases.length; si++) {
            var seq = cases[si].sequence;
            if (seq && seq.title) {
              sequenceCaseIds[cases[si].case_id] = true;
              if (!seenSeqs[seq.title] && !collectionSeqTitles[seq.title]) {
                seenSeqs[seq.title] = true;
                seqTitles.push(seq.title);
              }
            }
          }

          var standaloneCases = [];
          for (var sci = 0; sci < cases.length; sci++) {
            if (!sequenceCaseIds[cases[sci].case_id] && !collectionCaseIds[cases[sci].case_id]) {
              standaloneCases.push(cases[sci]);
            }
          }

          function isAlreadyOverridden(scopeType, scopeKey) {
            var fieldArr = (currentOverrides[scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections")] || []);
            for (var i = 0; i < fieldArr.length; i++) {
              if (String(fieldArr[i]) === String(scopeKey)) return true;
            }
            return false;
          }

          function makePickerRow(label, scopeType, scopeKey) {
            var row = document.createElement("label");
            row.style.cssText = "display:flex; align-items:center; gap:0.4rem; color:#ddd; font-size:0.82rem; padding:0.15rem 0; cursor:pointer;";
            var cb = document.createElement("input");
            cb.type = "checkbox";
            cb.style.accentColor = "#4a90d9";
            var alreadyDone = isAlreadyOverridden(scopeType, scopeKey);
            cb.checked = alreadyDone;
            cb.disabled = alreadyDone;
            if (alreadyDone) row.style.opacity = "0.5";
            cb.addEventListener("change", function () {
              invoke("toggle_plugin_for_scope", {
                filename: pluginFilename,
                scopeType: scopeType,
                scopeKey: scopeKey,
                enabled: isGloballyDisabled
              }).then(refreshScopeEditor)
                .catch(function (e) { statusMsg.textContent = "Error: " + e; });
            });
            row.appendChild(cb);
            row.appendChild(document.createTextNode(label));
            return row;
          }

          function makeGroupLabel(text) {
            var lbl = document.createElement("div");
            lbl.style.cssText = "color:#888; font-size:0.68rem; text-transform:uppercase; letter-spacing:0.04em; margin:0.4rem 0 0.2rem 0;";
            lbl.textContent = text;
            return lbl;
          }

          if (cols.length > 0) {
            pickerContainer.appendChild(makeGroupLabel("Collections"));
            for (var colIdx = 0; colIdx < cols.length; colIdx++) {
              pickerContainer.appendChild(makePickerRow(cols[colIdx].title, "collection", cols[colIdx].id));
            }
          }
          if (seqTitles.length > 0) {
            pickerContainer.appendChild(makeGroupLabel("Sequences"));
            for (var seqIdx = 0; seqIdx < seqTitles.length; seqIdx++) {
              pickerContainer.appendChild(makePickerRow(seqTitles[seqIdx], "sequence", seqTitles[seqIdx]));
            }
          }
          if (standaloneCases.length > 0) {
            pickerContainer.appendChild(makeGroupLabel("Individual Cases"));
            for (var caseIdx = 0; caseIdx < standaloneCases.length; caseIdx++) {
              pickerContainer.appendChild(makePickerRow(standaloneCases[caseIdx].title, "case", String(standaloneCases[caseIdx].case_id)));
            }
          }
          if (pickerContainer.children.length === 0) {
            var noItems = document.createElement("div");
            noItems.className = "muted";
            noItems.style.fontSize = "0.82rem";
            noItems.textContent = "No cases downloaded yet.";
            pickerContainer.appendChild(noItems);
          }
        }

        if (cases.length > 0 || cols.length > 0) {
          renderPicker(cases, cols);
        } else {
          var loadMsg = document.createElement("div");
          loadMsg.className = "muted";
          loadMsg.style.fontSize = "0.82rem";
          loadMsg.textContent = "Loading...";
          pickerContainer.appendChild(loadMsg);
          Promise.all([
            invoke("list_cases"),
            invoke("list_collections").catch(function () { return []; })
          ]).then(function (results) {
            renderPicker(results[0], results[1]);
          });
        }
      }

      contentEl.appendChild(addBtn);
      contentEl.appendChild(pickerContainer);
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
  modal.appendChild(contentEl);
  modal.appendChild(closeBtn);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);

  refreshScopeEditor();
}
