import { escapeHtml, createModal } from '../helpers.js';

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

  var m = createModal("<strong>Scope &mdash; " + escapeHtml(pluginFilename) + "</strong>", { wide: true });

  var contentEl = document.createElement("div");
  contentEl.style.cssText = "margin: 10px 0; max-height: 400px; overflow-y: auto;";

  function close() {
    m.close();
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
    Promise.all([
      invoke("list_global_plugins"),
      invoke("get_plugin_params", { filename: pluginFilename }),
      invoke("get_plugin_descriptors", { filename: pluginFilename }).catch(function() { return null; })
    ]).then(function (results) {
      var manifest = results[0];
      var allParams = results[1] || {};
      var descriptors = results[2]; // null if no descriptors
      var plugins = (manifest && manifest.plugins) || {};
      var entry = plugins[pluginFilename] || {};
      var scope = entry.scope || {};
      var globallyDisabled = !(scope.all === true);

      function formatParamSummary(params, descs) {
        if (!params || typeof params !== 'object') return '';
        var keys = Object.keys(params);
        if (keys.length === 0) return '';
        var parts = [];
        for (var fi = 0; fi < keys.length; fi++) {
          var k = keys[fi];
          var v = params[k];
          var lbl = (descs && descs[k] && descs[k].label) ? descs[k].label : k;
          if (typeof v === 'boolean') v = v ? 'enabled' : 'disabled';
          else if (typeof v === 'string' && v.length > 25) v = '"' + v.substring(0, 22) + '..."';
          else if (typeof v === 'string') v = '"' + v + '"';
          else v = String(v);
          parts.push(lbl + ' = ' + v);
        }
        return parts.join('  \u00b7  ');
      }

      function getSubScopeOverrides(scopeType, scopeKey) {
        var subs = [];
        var cachedCases = getCachedCases() || [];
        var cachedCollections = getCachedCollections() || [];

        if (scopeType === 'collection') {
          var col = null;
          for (var ci = 0; ci < cachedCollections.length; ci++) {
            if (cachedCollections[ci].id === scopeKey) { col = cachedCollections[ci]; break; }
          }
          if (!col) return subs;
          for (var ii = 0; ii < col.items.length; ii++) {
            var colItem = col.items[ii];
            if (colItem.type === 'sequence') {
              var seqP = allParams.by_sequence && allParams.by_sequence[colItem.title];
              if (seqP && Object.keys(seqP).length > 0) {
                subs.push({ label: 'Seq: ' + colItem.title, params: seqP });
              }
              for (var sc = 0; sc < cachedCases.length; sc++) {
                if (cachedCases[sc].sequence && cachedCases[sc].sequence.title === colItem.title) {
                  var cid = String(cachedCases[sc].case_id);
                  var cp = allParams.by_case && allParams.by_case[cid];
                  if (cp && Object.keys(cp).length > 0) {
                    subs.push({ label: 'Case: ' + cachedCases[sc].title, params: cp });
                  }
                }
              }
            } else if (colItem.type === 'case') {
              var ck = String(colItem.case_id);
              var caseP = allParams.by_case && allParams.by_case[ck];
              if (caseP && Object.keys(caseP).length > 0) {
                var cTitle = ck;
                for (var ct = 0; ct < cachedCases.length; ct++) {
                  if (cachedCases[ct].case_id === colItem.case_id) { cTitle = cachedCases[ct].title; break; }
                }
                subs.push({ label: 'Case: ' + cTitle, params: caseP });
              }
            }
          }
        } else if (scopeType === 'sequence') {
          for (var si = 0; si < cachedCases.length; si++) {
            if (cachedCases[si].sequence && cachedCases[si].sequence.title === scopeKey) {
              var seqCaseId = String(cachedCases[si].case_id);
              var seqCaseP = allParams.by_case && allParams.by_case[seqCaseId];
              if (seqCaseP && Object.keys(seqCaseP).length > 0) {
                subs.push({ label: 'Case: ' + cachedCases[si].title, params: seqCaseP });
              }
            }
          }
        }
        return subs;
      }

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
        invoke("toggle_plugin_for_scope", { filename: pluginFilename, scopeType: "global", scopeKey: "", enabled: globalToggle.checked })
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
      // In unified model, enabled_for/disabled_for are under scope
      var overrides = {};
      if (globallyDisabled) {
        // Show what's explicitly enabled
        overrides = {
          cases: scope.enabled_for || [],
          sequences: scope.enabled_for_sequences || [],
          collections: scope.enabled_for_collections || []
        };
      } else {
        // Show what's explicitly disabled
        overrides = {
          cases: scope.disabled_for || [],
          sequences: [],
          collections: []
        };
      }
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

            var paramsBtn = document.createElement("button");
            paramsBtn.className = "small-btn";
            paramsBtn.textContent = "Params";
            paramsBtn.style.cssText = "font-size:0.72rem; padding:1px 5px; margin-left:auto;";
            paramsBtn.addEventListener("click", function () {
              var paramLevel, paramKey;
              if (item.type === "collection") {
                paramLevel = "by_collection"; paramKey = item.key;
              } else if (item.type === "sequence") {
                paramLevel = "by_sequence"; paramKey = item.key;
              } else {
                paramLevel = "by_case"; paramKey = item.key;
              }
              ctx.showPluginParamsModal(pluginFilename, item.label, paramLevel, paramKey);
            });

            var removeBtn = document.createElement("button");
            removeBtn.className = "plugin-remove-btn";
            removeBtn.textContent = "Remove";
            removeBtn.addEventListener("click", function () {
              invoke("toggle_plugin_for_scope", {
                filename: pluginFilename,
                scopeType: item.type,
                scopeKey: item.key,
                enabled: !globallyDisabled
              }).then(refreshScopeEditor)
                .catch(function (e) { statusMsg.textContent = "Error: " + e; });
            });
            row.appendChild(label);
            row.appendChild(paramsBtn);
            row.appendChild(removeBtn);
            contentEl.appendChild(row);

            // Inline param summary for this scope level
            var scopeParams = null;
            if (item.type === 'collection') scopeParams = allParams.by_collection && allParams.by_collection[item.key];
            else if (item.type === 'sequence') scopeParams = allParams.by_sequence && allParams.by_sequence[item.key];
            else if (item.type === 'case') scopeParams = allParams.by_case && allParams.by_case[item.key];

            if (scopeParams && Object.keys(scopeParams).length > 0) {
              var summaryRow = document.createElement('div');
              summaryRow.style.cssText = 'display:flex; align-items:center; gap:0.3rem; padding:0.15rem 0 0 1.2rem;';
              var summaryEl = document.createElement('span');
              summaryEl.style.cssText = 'font-size:0.72rem; color:#9ab; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1;';
              var summaryText = formatParamSummary(scopeParams, descriptors);
              summaryEl.textContent = summaryText;
              summaryEl.title = summaryText;
              var resetBtn = document.createElement('button');
              resetBtn.className = 'small-btn';
              resetBtn.textContent = 'Reset';
              resetBtn.style.cssText = 'font-size:0.62rem; padding:0 4px; color:#a88; flex-shrink:0;';
              resetBtn.title = 'Clear param overrides at this scope and all sub-scopes within it';
              (function(scopeType, scopeKey) {
                resetBtn.addEventListener('click', function() {
                  // Build list of all levels to clear: this level + sub-scopes
                  var clearOps = [];
                  var pLevel = scopeType === 'collection' ? 'by_collection' : scopeType === 'sequence' ? 'by_sequence' : 'by_case';
                  clearOps.push({ level: pLevel, key: scopeKey });

                  // Also clear sub-scope overrides
                  var subs = getSubScopeOverrides(scopeType, scopeKey);
                  for (var ri = 0; ri < subs.length; ri++) {
                    var subLabel = subs[ri].label;
                    if (subLabel.indexOf('Seq: ') === 0) {
                      clearOps.push({ level: 'by_sequence', key: subLabel.substring(5) });
                    } else if (subLabel.indexOf('Case: ') === 0) {
                      // Need the case ID, not the title — find it from cached cases
                      var subCases = getCachedCases() || [];
                      var subTitle = subLabel.substring(6);
                      for (var rci = 0; rci < subCases.length; rci++) {
                        if (subCases[rci].title === subTitle) {
                          clearOps.push({ level: 'by_case', key: String(subCases[rci].case_id) });
                          break;
                        }
                      }
                    }
                  }

                  var chain = Promise.resolve();
                  for (var ci = 0; ci < clearOps.length; ci++) {
                    (function(op) {
                      chain = chain.then(function() {
                        return invoke('set_global_plugin_params', { filename: pluginFilename, level: op.level, key: op.key, params: {} });
                      });
                    })(clearOps[ci]);
                  }
                  chain.then(refreshScopeEditor)
                    .catch(function(e) { statusMsg.textContent = 'Error: ' + e; });
                });
              })(item.type, item.key);
              summaryRow.appendChild(summaryEl);
              summaryRow.appendChild(resetBtn);
              contentEl.appendChild(summaryRow);
            }

            // Sub-scope overrides (cases/sequences inside this scope with their own params)
            var subOverrides = getSubScopeOverrides(item.type, item.key);
            if (subOverrides.length > 0) {
              var subList = document.createElement('div');
              subList.style.cssText = 'padding:0.1rem 0 0.3rem 1.2rem;';
              for (var soi = 0; soi < subOverrides.length; soi++) {
                var subRow = document.createElement('div');
                subRow.style.cssText = 'font-size:0.68rem; color:#8a8; padding:0.05rem 0;';
                subRow.textContent = '\u251C ' + subOverrides[soi].label + ': ' + formatParamSummary(subOverrides[soi].params, descriptors);
                subList.appendChild(subRow);
              }
              contentEl.appendChild(subList);
            }
          })(overrideItems[oi]);
        }
      }

      // Inline default params display
      var defaultParams = allParams['default'] || {};
      var defaultSection = document.createElement('div');
      defaultSection.style.cssText = 'margin-top:0.5rem; padding-top:0.4rem; border-top:1px solid #2a2a4a;';

      var defaultRow = document.createElement('div');
      defaultRow.className = 'global-plugin-row';
      var defaultLabel = document.createElement('span');
      defaultLabel.style.cssText = 'color:#999; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.04em;';
      defaultLabel.textContent = 'Default';
      var defaultEditBtn = document.createElement('button');
      defaultEditBtn.className = 'small-btn';
      defaultEditBtn.textContent = 'Edit';
      defaultEditBtn.style.cssText = 'font-size:0.72rem; padding:1px 5px; margin-left:auto;';
      defaultEditBtn.addEventListener('click', function () {
        ctx.showPluginParamsModal(pluginFilename, 'Default', 'default', '');
      });
      var defaultResetBtn = document.createElement('button');
      defaultResetBtn.className = 'small-btn';
      defaultResetBtn.textContent = 'Reset';
      defaultResetBtn.style.cssText = 'font-size:0.62rem; padding:0 4px; color:#a88;';
      defaultResetBtn.title = 'Clear all default param overrides';
      defaultResetBtn.addEventListener('click', function() {
        invoke('set_global_plugin_params', { filename: pluginFilename, level: 'default', key: '', params: {} })
          .then(refreshScopeEditor)
          .catch(function(e) { statusMsg.textContent = 'Error: ' + e; });
      });
      defaultRow.appendChild(defaultLabel);
      defaultRow.appendChild(defaultEditBtn);
      defaultRow.appendChild(defaultResetBtn);
      defaultSection.appendChild(defaultRow);

      var defaultKeys = Object.keys(defaultParams);
      if (defaultKeys.length > 0) {
        var defSummaryEl = document.createElement('div');
        defSummaryEl.style.cssText = 'font-size:0.72rem; color:#9ab; padding:0.15rem 0 0 1.2rem;';
        defSummaryEl.textContent = formatParamSummary(defaultParams, descriptors);
        defaultSection.appendChild(defSummaryEl);
      } else {
        var defEmpty = document.createElement('div');
        defEmpty.style.cssText = 'font-size:0.72rem; color:#666; padding:0.15rem 0 0 1.2rem;';
        defEmpty.textContent = '(using plugin defaults)';
        defaultSection.appendChild(defEmpty);
      }
      contentEl.appendChild(defaultSection);

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
            buildPicker(overrides, globallyDisabled);
          }
        } else {
          pickerContainer.style.display = "none";
          addBtn.textContent = addBtnLabel;
        }
      });

      function buildPicker(currentOverrides, isGloballyDisabled) {
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

  m.content.appendChild(contentEl);
  m.modal.appendChild(closeBtn);

  refreshScopeEditor();
}
