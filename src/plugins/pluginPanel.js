import { showConfirmModal, createModal } from '../helpers.js';

/**
 * Plugin Panel — the global plugins sidebar panel with toggle, attach/import buttons,
 * and the showGlobalAttachCodeModal function.
 *
 * Returns { loadGlobalPluginsPanel, pluginsPanel, pluginsToggle }.
 */
export function initPluginPanel(ctx) {
  const invoke = ctx.invoke;
  const statusMsg = ctx.statusMsg;
  const getCachedCases = ctx.getCachedCases;
  const getCachedCollections = ctx.getCachedCollections;

  // DOM refs for Plugins Panel
  const pluginsToggle = document.getElementById("plugins-toggle");
  const pluginsPanel = document.getElementById("plugins-panel");
  const globalPluginsList = document.getElementById("global-plugins-list");
  const globalAttachBtn = document.getElementById("global-attach-btn");
  const globalImportBtn = document.getElementById("global-import-btn");

  function loadGlobalPluginsPanel() {
    invoke("list_global_plugins")
      .then(function (manifest) {
        const scripts = (manifest && manifest.scripts) || [];
        const plugins = (manifest && manifest.plugins) || {};
        // No more top-level disabled array — each plugin has scope.all
        globalPluginsList.innerHTML = "";
        if (scripts.length === 0) {
          const empty = document.createElement("div");
          empty.className = "global-plugins-empty";
          empty.textContent = "No global plugins installed.";
          globalPluginsList.appendChild(empty);
        } else {
          for (let i = 0; i < scripts.length; i++) {
            (function (filename) {
              const pluginEntry = plugins[filename] || {};
              const scope = pluginEntry.scope || {};
              const isDisabled = !(scope.all === true);
              const row = document.createElement("div");
              row.className = "global-plugin-row" + (isDisabled ? " disabled" : "");

              const toggle = document.createElement("input");
              toggle.type = "checkbox";
              toggle.checked = !isDisabled;
              toggle.style.accentColor = "#4a90d9";
              toggle.style.width = "1rem";
              toggle.style.height = "1rem";
              toggle.style.flexShrink = "0";
              toggle.addEventListener("change", function () {
                invoke("toggle_plugin_for_scope", { filename: filename, scopeType: "global", scopeKey: "", enabled: toggle.checked })
                  .then(function () { loadGlobalPluginsPanel(); })
                  .catch(function (e) { statusMsg.textContent = "Error: " + e; });
              });

              const name = document.createElement("span");
              name.className = "plugin-name";
              name.textContent = filename;

              // Scope badge
              const scopeBadge = document.createElement("span");
              scopeBadge.className = "scope-badge";
              // Build scope summary
              if (scope.all) {
                scopeBadge.textContent = "All cases";
              } else {
                const scopeParts = [];
                const ef = scope.enabled_for || [];
                const efs = scope.enabled_for_sequences || [];
                const efc = scope.enabled_for_collections || [];
                if (ef.length > 0) scopeParts.push(ef.length + " case" + (ef.length !== 1 ? "s" : ""));
                if (efs.length > 0) scopeParts.push(efs.length + " seq");
                if (efc.length > 0) scopeParts.push(efc.length + " col");
                scopeBadge.textContent = scopeParts.length > 0 ? scopeParts.join(", ") : "Disabled";
                if (scopeParts.length === 0) scopeBadge.style.color = "#888";
              }

              const overrideBadge = document.createElement("span");
              overrideBadge.className = "scope-badge";
              if (pluginEntry.origin) {
                overrideBadge.textContent = pluginEntry.origin;
                overrideBadge.style.color = "#888";
                overrideBadge.style.fontSize = "0.7rem";
              }

              const paramsBtn = document.createElement("button");
              paramsBtn.className = "small-btn btn-small";
              paramsBtn.textContent = "Params";
              paramsBtn.addEventListener("click", function () {
                ctx.showPluginParamsModal(filename, "Global Default", "default", "");
              });

              const removeBtn = document.createElement("button");
              removeBtn.className = "plugin-remove-btn";
              removeBtn.textContent = "Remove";
              removeBtn.addEventListener("click", function () {
                showConfirmModal("Remove global plugin \"" + filename + "\"?", "Remove", function () {
                  invoke("remove_global_plugin", { filename: filename })  // backward-compat command
                    .then(function () { loadGlobalPluginsPanel(); })
                    .catch(function (e) { statusMsg.textContent = "Error: " + e; });
                });
              });

              const scopeBtn = document.createElement("button");
              scopeBtn.className = "small-btn btn-small";
              scopeBtn.textContent = "Scope";
              scopeBtn.addEventListener("click", (function (fn) {
                return function () { ctx.showScopeEditorModal(fn); };
              })(filename));

              row.appendChild(toggle);
              row.appendChild(name);
              row.appendChild(scopeBadge);
              if (overrideBadge.textContent) row.appendChild(overrideBadge);
              row.appendChild(scopeBtn);
              row.appendChild(paramsBtn);
              row.appendChild(removeBtn);
              globalPluginsList.appendChild(row);
            })(scripts[i]);
          }
        }
      })
      .catch(function (e) {
        globalPluginsList.innerHTML = "";
        const errEl = document.createElement("div");
        errEl.className = "global-plugins-empty";
        errEl.textContent = "Error loading plugins: " + e;
        globalPluginsList.appendChild(errEl);
      });
  }

  function showGlobalAttachCodeModal(onDone) {
    const m = createModal("<strong>Attach Global Plugin Code</strong>", { wide: true });

    const filenameField = document.createElement("div");
    filenameField.className = "modal-field";
    const filenameLabel = document.createElement("label");
    filenameLabel.textContent = "Filename";
    const filenameInput = document.createElement("input");
    filenameInput.type = "text";
    filenameInput.placeholder = "my_plugin.js";
    filenameField.appendChild(filenameLabel);
    filenameField.appendChild(filenameInput);

    const codeField = document.createElement("div");
    codeField.className = "modal-field";
    const codeLabel = document.createElement("label");
    codeLabel.textContent = "Plugin Code";
    const codeInput = document.createElement("textarea");
    codeInput.className = "attach-code-textarea";
    codeInput.placeholder = "// Paste your plugin JS code here...";
    codeField.appendChild(codeLabel);
    codeField.appendChild(codeInput);

    let userEditedFilename = false;
    filenameInput.addEventListener("input", function () {
      userEditedFilename = true;
    });

    function detectPluginName() {
      const code = codeInput.value;
      const nameMatch = code.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
      if (nameMatch) {
        const detected = nameMatch[1] + ".js";
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

    const buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    const attachBtn = document.createElement("button");
    attachBtn.className = "modal-btn modal-btn-secondary";
    attachBtn.textContent = "Attach";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    // --- Scope picker ---
    const scopeSection = document.createElement("div");
    scopeSection.style.cssText = "margin: 0.75rem 0;";

    const scopeLabelEl = document.createElement("div");
    scopeLabelEl.className = "section-label";
    scopeLabelEl.textContent = "Enable for";

    const scopeAllRadio = document.createElement("input");
    scopeAllRadio.type = "radio";
    scopeAllRadio.name = "scope-mode";
    scopeAllRadio.checked = true;
    scopeAllRadio.style.accentColor = "#4a90d9";
    const scopeAllLabel = document.createElement("label");
    scopeAllLabel.className = "checkbox-label";
    scopeAllLabel.style.cssText = "gap:0.4rem; font-size:0.85rem; padding:0; margin-bottom:0.3rem;";
    scopeAllLabel.appendChild(scopeAllRadio);
    scopeAllLabel.appendChild(document.createTextNode("All cases (disabled by default)"));

    const scopeSpecificRadio = document.createElement("input");
    scopeSpecificRadio.type = "radio";
    scopeSpecificRadio.name = "scope-mode";
    scopeSpecificRadio.style.accentColor = "#4a90d9";
    const scopeSpecificLabel = document.createElement("label");
    scopeSpecificLabel.className = "checkbox-label";
    scopeSpecificLabel.style.cssText = "gap:0.4rem; font-size:0.85rem; padding:0; margin-bottom:0.3rem;";
    scopeSpecificLabel.appendChild(scopeSpecificRadio);
    scopeSpecificLabel.appendChild(document.createTextNode("Enable for specific scopes"));

    const scopeChecklist = document.createElement("div");
    scopeChecklist.className = "scroll-panel";
    scopeChecklist.style.cssText = "max-height:180px; padding:0.3rem 0; display:none;";

    function makeScopeGroupLabel(text) {
      const lbl = document.createElement("div");
      lbl.style.cssText = "color:#888; font-size:0.68rem; text-transform:uppercase; letter-spacing:0.04em; margin:0.4rem 0 0.2rem 0;";
      lbl.textContent = text;
      return lbl;
    }

    function makeScopeCheckbox(label, scopeType, scopeKey) {
      const row = document.createElement("label");
      row.style.cssText = "display:flex; align-items:center; gap:0.4rem; color:#ddd; font-size:0.82rem; padding:0.15rem 0; cursor:pointer;";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.style.accentColor = "#4a90d9";
      cb.dataset.scopeType = scopeType;
      cb.dataset.scopeKey = scopeKey;
      row.appendChild(cb);
      row.appendChild(document.createTextNode(label));
      return row;
    }

    let scopeChecklistPopulated = false;
    function populateScopeChecklist(cases, cols) {
      if (scopeChecklistPopulated) return;
      scopeChecklistPopulated = true;
      scopeChecklist.innerHTML = "";

      const collections = cols || [];
      const collectionCaseIds = {};
      const collectionSeqTitles = {};
      for (let ci = 0; ci < collections.length; ci++) {
        const colItems = collections[ci].items || [];
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
        const c = cases[sci];
        if (!sequenceCaseIds[c.case_id] && !collectionCaseIds[c.case_id]) {
          standaloneCases.push(c);
        }
      }

      if (collections.length > 0) {
        scopeChecklist.appendChild(makeScopeGroupLabel("Collections"));
        for (let colIdx = 0; colIdx < collections.length; colIdx++) {
          scopeChecklist.appendChild(makeScopeCheckbox(
            collections[colIdx].title,
            "collection",
            collections[colIdx].id
          ));
        }
      }

      if (seqTitles.length > 0) {
        scopeChecklist.appendChild(makeScopeGroupLabel("Sequences"));
        for (let seqIdx = 0; seqIdx < seqTitles.length; seqIdx++) {
          scopeChecklist.appendChild(makeScopeCheckbox(
            seqTitles[seqIdx],
            "sequence",
            seqTitles[seqIdx]
          ));
        }
      }

      if (standaloneCases.length > 0) {
        scopeChecklist.appendChild(makeScopeGroupLabel("Individual Cases"));
        for (let caseIdx = 0; caseIdx < standaloneCases.length; caseIdx++) {
          scopeChecklist.appendChild(makeScopeCheckbox(
            standaloneCases[caseIdx].title,
            "case",
            String(standaloneCases[caseIdx].case_id)
          ));
        }
      }

      if (scopeChecklist.children.length === 0) {
        const emptyMsg = document.createElement("div");
        emptyMsg.className = "muted";
        emptyMsg.style.fontSize = "0.82rem";
        emptyMsg.textContent = "No cases downloaded yet.";
        scopeChecklist.appendChild(emptyMsg);
      }
    }

    scopeAllRadio.addEventListener("change", function () {
      scopeChecklist.style.display = "none";
    });
    scopeSpecificRadio.addEventListener("change", function () {
      scopeChecklist.style.display = "block";
      // Lazy populate: use cached data or fetch if empty
      const cachedCases = getCachedCases();
      const cachedCollections = getCachedCollections();
      if (cachedCases.length > 0 || cachedCollections.length > 0) {
        populateScopeChecklist(cachedCases, cachedCollections);
      } else {
        scopeChecklist.innerHTML = "";
        const loadMsg = document.createElement("div");
        loadMsg.className = "muted";
        loadMsg.style.fontSize = "0.82rem";
        loadMsg.textContent = "Loading...";
        scopeChecklist.appendChild(loadMsg);
        Promise.all([
          invoke("list_cases"),
          invoke("list_collections").catch(function () { return []; })
        ]).then(function (results) {
          scopeChecklistPopulated = false;
          populateScopeChecklist(results[0], results[1]);
        }).catch(function(e) { console.error("[PLUGINS] Failed to load scope picker:", e); });
      }
    });

    attachBtn.addEventListener("click", function () {
      let filename = filenameInput.value.trim();
      if (!filename && filenameInput.placeholder && filenameInput.placeholder !== "my_plugin.js") {
        filename = filenameInput.placeholder;
      }
      const code = codeInput.value;

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

      m.close();
      statusMsg.textContent = "Attaching global plugin...";
      invoke("attach_plugin_code", {
        code: code,
        filename: filename,
        targetCaseIds: [],
        origin: "global"
      })
      .then(function () {
        // If specific scopes selected, enable for each
        const selectedScopes = [];
        if (scopeSpecificRadio.checked) {
          const checks = scopeChecklist.querySelectorAll("input[type=checkbox]:checked");
          for (let sc = 0; sc < checks.length; sc++) {
            selectedScopes.push({
              scopeType: checks[sc].dataset.scopeType,
              scopeKey: checks[sc].dataset.scopeKey
            });
          }
        }
        if (selectedScopes.length > 0) {
          const togglePromises = selectedScopes.map(function (s) {
            return invoke("toggle_plugin_for_scope", {
              filename: filename,
              scopeType: s.scopeType,
              scopeKey: s.scopeKey,
              enabled: true
            });
          });
          Promise.all(togglePromises).then(function () {
            statusMsg.textContent = "Plugin \"" + filename + "\" attached and enabled for " + selectedScopes.length + " scope(s).";
            if (onDone) onDone(filename);
          }).catch(function (e) {
            statusMsg.textContent = "Plugin attached but scope error: " + e;
            if (onDone) onDone(filename);
          });
        } else {
          statusMsg.textContent = "Global plugin \"" + filename + "\" attached (disabled by default).";
          if (onDone) onDone(filename);
        }
      })
      .catch(function (e) {
        statusMsg.textContent = "Error attaching plugin: " + e;
      });
    });

    cancelBtn.addEventListener("click", m.close);

    scopeSection.appendChild(scopeLabelEl);
    scopeSection.appendChild(scopeAllLabel);
    scopeSection.appendChild(scopeSpecificLabel);
    scopeSection.appendChild(scopeChecklist);

    buttons.appendChild(attachBtn);
    buttons.appendChild(cancelBtn);

    m.content.appendChild(filenameField);
    m.content.appendChild(codeField);
    m.content.appendChild(scopeSection);
    m.modal.appendChild(buttons);

    filenameInput.focus();
  }

  // Panel toggle listener
  pluginsToggle.addEventListener("click", function () {
    const isOpen = !pluginsPanel.classList.contains("hidden");
    if (isOpen) {
      pluginsPanel.classList.add("hidden");
      pluginsToggle.classList.remove("open");
    } else {
      pluginsPanel.classList.remove("hidden");
      pluginsToggle.classList.add("open");
      loadGlobalPluginsPanel();
    }
  });

  // Attach button listener
  globalAttachBtn.addEventListener("click", function () {
    showGlobalAttachCodeModal(function () { loadGlobalPluginsPanel(); });
  });

  // Import button listener
  globalImportBtn.addEventListener("click", function () {
    invoke("pick_import_file")
      .then(function (selected) {
        if (!selected) return;
        if (!selected.toLowerCase().endsWith(".aaoplug")) {
          statusMsg.textContent = "Please select a .aaoplug file.";
          return;
        }
        statusMsg.textContent = "Importing global plugin...";
        invoke("import_plugin", { sourcePath: selected, targetCaseIds: [], origin: "global" })
          .then(function () {
            statusMsg.textContent = "Plugin imported globally.";
            loadGlobalPluginsPanel();
          })
          .catch(function (e) {
            statusMsg.textContent = "Error importing plugin: " + e;
          });
      })
      .catch(function (e) {
        statusMsg.textContent = "Could not open file picker: " + e;
      });
  });

  // Expose showGlobalAttachCodeModal on ctx so scopedModal can use it
  ctx.showGlobalAttachCodeModal = showGlobalAttachCodeModal;

  return {
    loadGlobalPluginsPanel: loadGlobalPluginsPanel,
    pluginsPanel: pluginsPanel,
    pluginsToggle: pluginsToggle
  };
}
