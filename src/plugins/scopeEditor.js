import { escapeHtml, createModal } from '../helpers.js';

/**
 * Scope Editor Modal — shows all per-scope overrides for a single plugin.
 * If globally enabled: shows disabled_for exceptions.
 * If globally disabled: shows enabled_for overrides.
 */
export function showScopeEditorModal(ctx, pluginFilename) {
  const invoke = ctx.invoke;
  const statusMsg = ctx.statusMsg;
  const getCachedCases = ctx.getCachedCases;
  const getCachedCollections = ctx.getCachedCollections;

  const m = createModal("<strong>Scope &mdash; " + escapeHtml(pluginFilename) + "</strong>", { wide: true });

  const contentEl = document.createElement("div");
  contentEl.className = "scroll-panel";
  contentEl.style.maxHeight = "400px";

  function close() {
    m.close();
    ctx.loadGlobalPluginsPanel();
  }

  function resolveCollectionTitle(colId) {
    const cols = getCachedCollections();
    for (let i = 0; i < (cols || []).length; i++) {
      if (cols[i].id === colId) return cols[i].title;
    }
    return colId;
  }

  function resolveCaseTitle(caseId) {
    const cases = getCachedCases();
    const id = Number(caseId);
    for (let i = 0; i < (cases || []).length; i++) {
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
      const manifest = results[0];
      const allParams = results[1] || {};
      const descriptors = results[2]; // null if no descriptors
      const plugins = (manifest && manifest.plugins) || {};
      const entry = plugins[pluginFilename] || {};
      const scope = entry.scope || {};
      const globallyDisabled = !(scope.all === true);

      function formatParamSummary(params, descs) {
        if (!params || typeof params !== 'object') return '';
        const keys = Object.keys(params);
        if (keys.length === 0) return '';
        const parts = [];
        for (let fi = 0; fi < keys.length; fi++) {
          const k = keys[fi];
          let v = params[k];
          const lbl = (descs && descs[k] && descs[k].label) ? descs[k].label : k;
          if (typeof v === 'boolean') v = v ? 'enabled' : 'disabled';
          else if (typeof v === 'string' && v.length > 25) v = '"' + v.substring(0, 22) + '..."';
          else if (typeof v === 'string') v = '"' + v + '"';
          else v = String(v);
          parts.push(lbl + ' = ' + v);
        }
        return parts.join('  \u00b7  ');
      }

      function getSubScopeOverrides(scopeType, scopeKey) {
        const subs = [];
        const cachedCases = getCachedCases() || [];
        const cachedCollections = getCachedCollections() || [];

        if (scopeType === 'collection') {
          let col = null;
          for (let ci = 0; ci < cachedCollections.length; ci++) {
            if (cachedCollections[ci].id === scopeKey) { col = cachedCollections[ci]; break; }
          }
          if (!col) return subs;
          for (let ii = 0; ii < col.items.length; ii++) {
            const colItem = col.items[ii];
            if (colItem.type === 'sequence') {
              const seqP = allParams.by_sequence && allParams.by_sequence[colItem.title];
              if (seqP && Object.keys(seqP).length > 0) {
                subs.push({ label: 'Seq: ' + colItem.title, params: seqP });
              }
              for (let sc = 0; sc < cachedCases.length; sc++) {
                if (cachedCases[sc].sequence && cachedCases[sc].sequence.title === colItem.title) {
                  const cid = String(cachedCases[sc].case_id);
                  const cp = allParams.by_case && allParams.by_case[cid];
                  if (cp && Object.keys(cp).length > 0) {
                    subs.push({ label: 'Case: ' + cachedCases[sc].title, params: cp });
                  }
                }
              }
            } else if (colItem.type === 'case') {
              const ck = String(colItem.case_id);
              const caseP = allParams.by_case && allParams.by_case[ck];
              if (caseP && Object.keys(caseP).length > 0) {
                let cTitle = ck;
                for (let ct = 0; ct < cachedCases.length; ct++) {
                  if (cachedCases[ct].case_id === colItem.case_id) { cTitle = cachedCases[ct].title; break; }
                }
                subs.push({ label: 'Case: ' + cTitle, params: caseP });
              }
            }
          }
        } else if (scopeType === 'sequence') {
          for (let si = 0; si < cachedCases.length; si++) {
            if (cachedCases[si].sequence && cachedCases[si].sequence.title === scopeKey) {
              const seqCaseId = String(cachedCases[si].case_id);
              const seqCaseP = allParams.by_case && allParams.by_case[seqCaseId];
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
      const globalRow = document.createElement("div");
      globalRow.className = "flex-row";
      globalRow.style.cssText = "margin-bottom:0.75rem; padding-bottom:0.5rem; border-bottom:1px solid #2a2a4a;";
      const globalToggle = document.createElement("input");
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
      const globalLabelEl = document.createElement("span");
      globalLabelEl.style.cssText = "color:#ccc; font-size:0.9rem;";
      globalLabelEl.textContent = "Globally " + (globallyDisabled ? "disabled" : "enabled");
      globalRow.appendChild(globalToggle);
      globalRow.appendChild(globalLabelEl);
      contentEl.appendChild(globalRow);

      // Section label
      const sectionLabel = document.createElement("div");
      sectionLabel.className = "section-label";
      if (globallyDisabled) {
        sectionLabel.textContent = "Enabled for (overrides)";
      } else {
        sectionLabel.textContent = "Disabled for (exceptions)";
      }
      contentEl.appendChild(sectionLabel);

      // Build override list
      // In unified model, enabled_for/disabled_for are under scope
      let overrides = {};
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
      const overrideItems = [];

      const colArr = overrides.collections || [];
      for (let ci = 0; ci < colArr.length; ci++) {
        overrideItems.push({ type: "collection", key: String(colArr[ci]), label: "Collection: " + resolveCollectionTitle(String(colArr[ci])) });
      }
      const seqArr = overrides.sequences || [];
      for (let si = 0; si < seqArr.length; si++) {
        overrideItems.push({ type: "sequence", key: String(seqArr[si]), label: "Sequence: " + seqArr[si] });
      }
      const caseArr = overrides.cases || [];
      for (let cai = 0; cai < caseArr.length; cai++) {
        overrideItems.push({ type: "case", key: String(caseArr[cai]), label: "Case: " + resolveCaseTitle(caseArr[cai]) });
      }

      if (overrideItems.length === 0) {
        const emptyMsg = document.createElement("div");
        emptyMsg.className = "muted";
        emptyMsg.style.cssText = "font-size:0.82rem; padding:0.3rem 0;";
        emptyMsg.textContent = "No per-scope overrides.";
        contentEl.appendChild(emptyMsg);
      } else {
        for (let oi = 0; oi < overrideItems.length; oi++) {
          (function (item) {
            const row = document.createElement("div");
            row.className = "global-plugin-row";
            const label = document.createElement("span");
            label.className = "plugin-name";
            label.textContent = item.label;

            const paramsBtn = document.createElement("button");
            paramsBtn.className = "small-btn btn-small";
            paramsBtn.textContent = "Params";
            paramsBtn.style.marginLeft = "auto";
            paramsBtn.addEventListener("click", function () {
              let paramLevel, paramKey;
              if (item.type === "collection") {
                paramLevel = "by_collection"; paramKey = item.key;
              } else if (item.type === "sequence") {
                paramLevel = "by_sequence"; paramKey = item.key;
              } else {
                paramLevel = "by_case"; paramKey = item.key;
              }
              ctx.showPluginParamsModal(pluginFilename, item.label, paramLevel, paramKey);
            });

            const removeBtn = document.createElement("button");
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
            let scopeParams = null;
            if (item.type === 'collection') scopeParams = allParams.by_collection && allParams.by_collection[item.key];
            else if (item.type === 'sequence') scopeParams = allParams.by_sequence && allParams.by_sequence[item.key];
            else if (item.type === 'case') scopeParams = allParams.by_case && allParams.by_case[item.key];

            if (scopeParams && Object.keys(scopeParams).length > 0) {
              const summaryRow = document.createElement('div');
              summaryRow.className = 'flex-row';
              summaryRow.style.cssText = 'gap:0.3rem; padding:0.15rem 0 0 1.2rem;';
              const summaryEl = document.createElement('span');
              summaryEl.className = 'param-summary';
              const summaryText = formatParamSummary(scopeParams, descriptors);
              summaryEl.textContent = summaryText;
              summaryEl.title = summaryText;
              const resetBtn = document.createElement('button');
              resetBtn.className = 'small-btn btn-reset';
              resetBtn.textContent = 'Reset';
              resetBtn.title = 'Clear param overrides at this scope and all sub-scopes within it';
              (function(scopeType, scopeKey) {
                resetBtn.addEventListener('click', function() {
                  // Build list of all levels to clear: this level + sub-scopes
                  const clearOps = [];
                  const pLevel = scopeType === 'collection' ? 'by_collection' : scopeType === 'sequence' ? 'by_sequence' : 'by_case';
                  clearOps.push({ level: pLevel, key: scopeKey });

                  // Also clear sub-scope overrides
                  const subs = getSubScopeOverrides(scopeType, scopeKey);
                  for (let ri = 0; ri < subs.length; ri++) {
                    const subLabel = subs[ri].label;
                    if (subLabel.indexOf('Seq: ') === 0) {
                      clearOps.push({ level: 'by_sequence', key: subLabel.substring(5) });
                    } else if (subLabel.indexOf('Case: ') === 0) {
                      // Need the case ID, not the title — find it from cached cases
                      const subCases = getCachedCases() || [];
                      const subTitle = subLabel.substring(6);
                      for (let rci = 0; rci < subCases.length; rci++) {
                        if (subCases[rci].title === subTitle) {
                          clearOps.push({ level: 'by_case', key: String(subCases[rci].case_id) });
                          break;
                        }
                      }
                    }
                  }

                  let chain = Promise.resolve();
                  for (let ci = 0; ci < clearOps.length; ci++) {
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
            const subOverrides = getSubScopeOverrides(item.type, item.key);
            if (subOverrides.length > 0) {
              const subList = document.createElement('div');
              subList.style.cssText = 'padding:0.1rem 0 0.3rem 1.2rem;';
              for (let soi = 0; soi < subOverrides.length; soi++) {
                const subRow = document.createElement('div');
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
      const defaultParams = allParams['default'] || {};
      const defaultSection = document.createElement('div');
      defaultSection.style.cssText = 'margin-top:0.5rem; padding-top:0.4rem; border-top:1px solid #2a2a4a;';

      const defaultRow = document.createElement('div');
      defaultRow.className = 'global-plugin-row';
      const defaultLabel = document.createElement('span');
      defaultLabel.className = 'section-label';
      defaultLabel.style.marginBottom = '0';
      defaultLabel.textContent = 'Default';
      const defaultEditBtn = document.createElement('button');
      defaultEditBtn.className = 'small-btn btn-small';
      defaultEditBtn.textContent = 'Edit';
      defaultEditBtn.style.marginLeft = 'auto';
      defaultEditBtn.addEventListener('click', function () {
        ctx.showPluginParamsModal(pluginFilename, 'Default', 'default', '');
      });
      const defaultResetBtn = document.createElement('button');
      defaultResetBtn.className = 'small-btn btn-reset';
      defaultResetBtn.textContent = 'Reset';
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

      const defaultKeys = Object.keys(defaultParams);
      if (defaultKeys.length > 0) {
        const defSummaryEl = document.createElement('div');
        defSummaryEl.className = 'param-summary';
        defSummaryEl.style.padding = '0.15rem 0 0 1.2rem';
        defSummaryEl.textContent = formatParamSummary(defaultParams, descriptors);
        defaultSection.appendChild(defSummaryEl);
      } else {
        const defEmpty = document.createElement('div');
        defEmpty.style.cssText = 'font-size:0.72rem; color:#666; padding:0.15rem 0 0 1.2rem;';
        defEmpty.textContent = '(using plugin defaults)';
        defaultSection.appendChild(defEmpty);
      }
      contentEl.appendChild(defaultSection);

      // Add override button + inline picker
      const addBtnLabel = globallyDisabled ? "+ Enable for Scope" : "+ Add Exception";
      const addBtn = document.createElement("button");
      addBtn.className = "small-btn";
      addBtn.textContent = addBtnLabel;
      addBtn.style.cssText = "margin-top:0.5rem; font-size:0.78rem;";

      const pickerContainer = document.createElement("div");
      pickerContainer.className = "scroll-panel";
      pickerContainer.style.cssText = "max-height:180px; padding:0.3rem 0; display:none; margin-top:0.3rem;";

      let pickerBuilt = false;
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
        const cases = getCachedCases() || [];
        const cols = getCachedCollections() || [];

        function renderPicker(cases, cols) {
          pickerContainer.innerHTML = "";
          const collectionCaseIds = {};
          const collectionSeqTitles = {};
          for (let ci = 0; ci < cols.length; ci++) {
            const colItems = cols[ci].items || [];
            for (let ii = 0; ii < colItems.length; ii++) {
              if (colItems[ii].type === "case") collectionCaseIds[colItems[ii].case_id] = true;
              if (colItems[ii].type === "sequence") collectionSeqTitles[colItems[ii].title] = true;
            }
          }

          const seqTitles = [];
          const seenSeqs = {};
          const sequenceCaseIds = {};
          for (let si = 0; si < cases.length; si++) {
            const seq = cases[si].sequence;
            if (seq && seq.title) {
              sequenceCaseIds[cases[si].case_id] = true;
              if (!seenSeqs[seq.title] && !collectionSeqTitles[seq.title]) {
                seenSeqs[seq.title] = true;
                seqTitles.push(seq.title);
              }
            }
          }

          const standaloneCases = [];
          for (let sci = 0; sci < cases.length; sci++) {
            if (!sequenceCaseIds[cases[sci].case_id] && !collectionCaseIds[cases[sci].case_id]) {
              standaloneCases.push(cases[sci]);
            }
          }

          function isAlreadyOverridden(scopeType, scopeKey) {
            const fieldArr = (currentOverrides[scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections")] || []);
            for (let i = 0; i < fieldArr.length; i++) {
              if (String(fieldArr[i]) === String(scopeKey)) return true;
            }
            return false;
          }

          function makePickerRow(label, scopeType, scopeKey) {
            const row = document.createElement("label");
            row.style.cssText = "display:flex; align-items:center; gap:0.4rem; color:#ddd; font-size:0.82rem; padding:0.15rem 0; cursor:pointer;";
            const cb = document.createElement("input");
            cb.type = "checkbox";
            cb.style.accentColor = "#4a90d9";
            const alreadyDone = isAlreadyOverridden(scopeType, scopeKey);
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
            const lbl = document.createElement("div");
            lbl.style.cssText = "color:#888; font-size:0.68rem; text-transform:uppercase; letter-spacing:0.04em; margin:0.4rem 0 0.2rem 0;";
            lbl.textContent = text;
            return lbl;
          }

          if (cols.length > 0) {
            pickerContainer.appendChild(makeGroupLabel("Collections"));
            for (let colIdx = 0; colIdx < cols.length; colIdx++) {
              pickerContainer.appendChild(makePickerRow(cols[colIdx].title, "collection", cols[colIdx].id));
            }
          }
          if (seqTitles.length > 0) {
            pickerContainer.appendChild(makeGroupLabel("Sequences"));
            for (let seqIdx = 0; seqIdx < seqTitles.length; seqIdx++) {
              pickerContainer.appendChild(makePickerRow(seqTitles[seqIdx], "sequence", seqTitles[seqIdx]));
            }
          }
          if (standaloneCases.length > 0) {
            pickerContainer.appendChild(makeGroupLabel("Individual Cases"));
            for (let caseIdx = 0; caseIdx < standaloneCases.length; caseIdx++) {
              pickerContainer.appendChild(makePickerRow(standaloneCases[caseIdx].title, "case", String(standaloneCases[caseIdx].case_id)));
            }
          }
          if (pickerContainer.children.length === 0) {
            const noItems = document.createElement("div");
            noItems.className = "muted";
            noItems.style.fontSize = "0.82rem";
            noItems.textContent = "No cases downloaded yet.";
            pickerContainer.appendChild(noItems);
          }
        }

        if (cases.length > 0 || cols.length > 0) {
          renderPicker(cases, cols);
        } else {
          const loadMsg = document.createElement("div");
          loadMsg.className = "muted";
          loadMsg.style.fontSize = "0.82rem";
          loadMsg.textContent = "Loading...";
          pickerContainer.appendChild(loadMsg);
          Promise.all([
            invoke("list_cases"),
            invoke("list_collections").catch(function () { return []; })
          ]).then(function (results) {
            renderPicker(results[0], results[1]);
          }).catch(function(e) { console.error("[PLUGINS] Failed to load scope picker:", e); });
        }
      }

      contentEl.appendChild(addBtn);
      contentEl.appendChild(pickerContainer);
    }).catch(function(e) { console.error("[PLUGINS] Failed to load scope editor:", e); });
  }

  const closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";
  closeBtn.addEventListener("click", close);

  m.content.appendChild(contentEl);
  m.modal.appendChild(closeBtn);

  refreshScopeEditor();
}
