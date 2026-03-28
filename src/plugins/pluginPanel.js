import { showConfirmModal } from '../helpers.js';

/**
 * Plugin Panel — the global plugins sidebar panel with toggle, attach/import buttons,
 * and the showGlobalAttachCodeModal function.
 *
 * Returns { loadGlobalPluginsPanel, pluginsPanel, pluginsToggle }.
 */
export function initPluginPanel(ctx) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;
  var getCachedCases = ctx.getCachedCases;
  var getCachedCollections = ctx.getCachedCollections;

  // DOM refs for Plugins Panel
  var pluginsToggle = document.getElementById("plugins-toggle");
  var pluginsPanel = document.getElementById("plugins-panel");
  var globalPluginsList = document.getElementById("global-plugins-list");
  var globalAttachBtn = document.getElementById("global-attach-btn");
  var globalImportBtn = document.getElementById("global-import-btn");

  function loadGlobalPluginsPanel() {
    invoke("list_global_plugins")
      .then(function (manifest) {
        var scripts = (manifest && manifest.scripts) || [];
        var plugins = (manifest && manifest.plugins) || {};
        var disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
        globalPluginsList.innerHTML = "";
        if (scripts.length === 0) {
          var empty = document.createElement("div");
          empty.className = "global-plugins-empty";
          empty.textContent = "No global plugins installed.";
          globalPluginsList.appendChild(empty);
        } else {
          for (var i = 0; i < scripts.length; i++) {
            (function (filename) {
              var isDisabled = disabledList.indexOf(filename) !== -1;
              var row = document.createElement("div");
              row.className = "global-plugin-row" + (isDisabled ? " disabled" : "");

              var toggle = document.createElement("input");
              toggle.type = "checkbox";
              toggle.checked = !isDisabled;
              toggle.style.accentColor = "#4a90d9";
              toggle.style.width = "1rem";
              toggle.style.height = "1rem";
              toggle.style.flexShrink = "0";
              toggle.addEventListener("change", function () {
                invoke("toggle_global_plugin", { filename: filename, enabled: toggle.checked })
                  .then(function () { loadGlobalPluginsPanel(); })
                  .catch(function (e) { statusMsg.textContent = "Error: " + e; });
              });

              var name = document.createElement("span");
              name.className = "plugin-name";
              name.textContent = filename;

              // Scope badge
              var pluginEntry = plugins[filename] || {};
              var scopeBadge = document.createElement("span");
              scopeBadge.className = "scope-badge";
              scopeBadge.textContent = "All cases";

              // Override summary badge
              var overrideBadge = document.createElement("span");
              overrideBadge.className = "scope-badge";
              if (isDisabled) {
                // Globally disabled - count enabled_for entries
                var ef = pluginEntry.enabled_for || {};
                var efCount = ((ef.cases || []).length) + ((ef.sequences || []).length) + ((ef.collections || []).length);
                if (efCount > 0) {
                  overrideBadge.textContent = efCount + " enabled";
                  overrideBadge.style.color = "#6a6";
                }
              } else {
                // Globally enabled - count disabled_for entries
                var df = pluginEntry.disabled_for || {};
                var dfCount = ((df.cases || []).length) + ((df.sequences || []).length) + ((df.collections || []).length);
                if (dfCount > 0) {
                  overrideBadge.textContent = dfCount + " exception" + (dfCount !== 1 ? "s" : "");
                  overrideBadge.style.color = "#a66";
                }
              }

              var paramsBtn = document.createElement("button");
              paramsBtn.className = "small-btn";
              paramsBtn.textContent = "Params";
              paramsBtn.style.cssText = "font-size:0.72rem; padding:0.1rem 0.5rem;";
              paramsBtn.addEventListener("click", function () {
                ctx.showPluginParamsModal(filename, "Global Default", "default", "");
              });

              var removeBtn = document.createElement("button");
              removeBtn.className = "plugin-remove-btn";
              removeBtn.textContent = "Remove";
              removeBtn.addEventListener("click", function () {
                showConfirmModal("Remove global plugin \"" + filename + "\"?", "Remove", function () {
                  invoke("remove_global_plugin", { filename: filename })
                    .then(function () { loadGlobalPluginsPanel(); })
                    .catch(function (e) { statusMsg.textContent = "Error: " + e; });
                });
              });

              var scopeBtn = document.createElement("button");
              scopeBtn.className = "small-btn";
              scopeBtn.textContent = "Scope";
              scopeBtn.style.cssText = "font-size:0.72rem; padding:0.1rem 0.5rem;";
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
        var errEl = document.createElement("div");
        errEl.className = "global-plugins-empty";
        errEl.textContent = "Error loading plugins: " + e;
        globalPluginsList.appendChild(errEl);
      });
  }

  function showGlobalAttachCodeModal(onDone) {
    var overlay = document.createElement("div");
    overlay.className = "modal-overlay";

    var modal = document.createElement("div");
    modal.className = "modal-dialog modal-dialog-wide";

    var titleEl = document.createElement("div");
    titleEl.className = "modal-message";
    titleEl.innerHTML = "<strong>Attach Global Plugin Code</strong>";

    var filenameField = document.createElement("div");
    filenameField.className = "modal-field";
    var filenameLabel = document.createElement("label");
    filenameLabel.textContent = "Filename";
    var filenameInput = document.createElement("input");
    filenameInput.type = "text";
    filenameInput.placeholder = "my_plugin.js";
    filenameField.appendChild(filenameLabel);
    filenameField.appendChild(filenameInput);

    var codeField = document.createElement("div");
    codeField.className = "modal-field";
    var codeLabel = document.createElement("label");
    codeLabel.textContent = "Plugin Code";
    var codeInput = document.createElement("textarea");
    codeInput.className = "attach-code-textarea";
    codeInput.placeholder = "// Paste your plugin JS code here...";
    codeField.appendChild(codeLabel);
    codeField.appendChild(codeInput);

    var userEditedFilename = false;
    filenameInput.addEventListener("input", function () {
      userEditedFilename = true;
    });

    function detectPluginName() {
      var code = codeInput.value;
      var nameMatch = code.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
      if (nameMatch) {
        var detected = nameMatch[1] + ".js";
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

    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var attachBtn = document.createElement("button");
    attachBtn.className = "modal-btn modal-btn-secondary";
    attachBtn.textContent = "Attach";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    function close() {
      document.body.removeChild(overlay);
    }

    // --- Scope picker ---
    var scopeSection = document.createElement("div");
    scopeSection.style.cssText = "margin: 0.75rem 0;";

    var scopeLabelEl = document.createElement("div");
    scopeLabelEl.style.cssText = "color:#999; font-size:0.75rem; text-transform:uppercase; letter-spacing:0.04em; margin-bottom:0.35rem;";
    scopeLabelEl.textContent = "Enable for";

    var scopeAllRadio = document.createElement("input");
    scopeAllRadio.type = "radio";
    scopeAllRadio.name = "scope-mode";
    scopeAllRadio.checked = true;
    scopeAllRadio.style.accentColor = "#4a90d9";
    var scopeAllLabel = document.createElement("label");
    scopeAllLabel.style.cssText = "display:flex; align-items:center; gap:0.4rem; color:#ccc; font-size:0.85rem; margin-bottom:0.3rem; cursor:pointer;";
    scopeAllLabel.appendChild(scopeAllRadio);
    scopeAllLabel.appendChild(document.createTextNode("All cases (disabled by default)"));

    var scopeSpecificRadio = document.createElement("input");
    scopeSpecificRadio.type = "radio";
    scopeSpecificRadio.name = "scope-mode";
    scopeSpecificRadio.style.accentColor = "#4a90d9";
    var scopeSpecificLabel = document.createElement("label");
    scopeSpecificLabel.style.cssText = "display:flex; align-items:center; gap:0.4rem; color:#ccc; font-size:0.85rem; margin-bottom:0.3rem; cursor:pointer;";
    scopeSpecificLabel.appendChild(scopeSpecificRadio);
    scopeSpecificLabel.appendChild(document.createTextNode("Enable for specific scopes"));

    var scopeChecklist = document.createElement("div");
    scopeChecklist.style.cssText = "max-height:180px; overflow-y:auto; padding:0.3rem 0; display:none;";

    function makeScopeGroupLabel(text) {
      var lbl = document.createElement("div");
      lbl.style.cssText = "color:#888; font-size:0.68rem; text-transform:uppercase; letter-spacing:0.04em; margin:0.4rem 0 0.2rem 0;";
      lbl.textContent = text;
      return lbl;
    }

    function makeScopeCheckbox(label, scopeType, scopeKey) {
      var row = document.createElement("label");
      row.style.cssText = "display:flex; align-items:center; gap:0.4rem; color:#ddd; font-size:0.82rem; padding:0.15rem 0; cursor:pointer;";
      var cb = document.createElement("input");
      cb.type = "checkbox";
      cb.style.accentColor = "#4a90d9";
      cb.dataset.scopeType = scopeType;
      cb.dataset.scopeKey = scopeKey;
      row.appendChild(cb);
      row.appendChild(document.createTextNode(label));
      return row;
    }

    var scopeChecklistPopulated = false;
    function populateScopeChecklist(cases, cols) {
      if (scopeChecklistPopulated) return;
      scopeChecklistPopulated = true;
      scopeChecklist.innerHTML = "";

      var collections = cols || [];
      var collectionCaseIds = {};
      var collectionSeqTitles = {};
      for (var ci = 0; ci < collections.length; ci++) {
        var colItems = collections[ci].items || [];
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
        var c = cases[sci];
        if (!sequenceCaseIds[c.case_id] && !collectionCaseIds[c.case_id]) {
          standaloneCases.push(c);
        }
      }

      if (collections.length > 0) {
        scopeChecklist.appendChild(makeScopeGroupLabel("Collections"));
        for (var colIdx = 0; colIdx < collections.length; colIdx++) {
          scopeChecklist.appendChild(makeScopeCheckbox(
            collections[colIdx].title,
            "collection",
            collections[colIdx].id
          ));
        }
      }

      if (seqTitles.length > 0) {
        scopeChecklist.appendChild(makeScopeGroupLabel("Sequences"));
        for (var seqIdx = 0; seqIdx < seqTitles.length; seqIdx++) {
          scopeChecklist.appendChild(makeScopeCheckbox(
            seqTitles[seqIdx],
            "sequence",
            seqTitles[seqIdx]
          ));
        }
      }

      if (standaloneCases.length > 0) {
        scopeChecklist.appendChild(makeScopeGroupLabel("Individual Cases"));
        for (var caseIdx = 0; caseIdx < standaloneCases.length; caseIdx++) {
          scopeChecklist.appendChild(makeScopeCheckbox(
            standaloneCases[caseIdx].title,
            "case",
            String(standaloneCases[caseIdx].case_id)
          ));
        }
      }

      if (scopeChecklist.children.length === 0) {
        var emptyMsg = document.createElement("div");
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
      var cachedCases = getCachedCases();
      var cachedCollections = getCachedCollections();
      if (cachedCases.length > 0 || cachedCollections.length > 0) {
        populateScopeChecklist(cachedCases, cachedCollections);
      } else {
        scopeChecklist.innerHTML = "";
        var loadMsg = document.createElement("div");
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
        });
      }
    });

    attachBtn.addEventListener("click", function () {
      var filename = filenameInput.value.trim();
      if (!filename && filenameInput.placeholder && filenameInput.placeholder !== "my_plugin.js") {
        filename = filenameInput.placeholder;
      }
      var code = codeInput.value;

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

      close();
      statusMsg.textContent = "Attaching global plugin...";
      invoke("attach_global_plugin_code", {
        code: code,
        filename: filename
      })
      .then(function () {
        // If specific scopes selected, enable for each
        var selectedScopes = [];
        if (scopeSpecificRadio.checked) {
          var checks = scopeChecklist.querySelectorAll("input[type=checkbox]:checked");
          for (var sc = 0; sc < checks.length; sc++) {
            selectedScopes.push({
              scopeType: checks[sc].dataset.scopeType,
              scopeKey: checks[sc].dataset.scopeKey
            });
          }
        }
        if (selectedScopes.length > 0) {
          var togglePromises = selectedScopes.map(function (s) {
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

    cancelBtn.addEventListener("click", close);
    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) close();
    });

    scopeSection.appendChild(scopeLabelEl);
    scopeSection.appendChild(scopeAllLabel);
    scopeSection.appendChild(scopeSpecificLabel);
    scopeSection.appendChild(scopeChecklist);

    buttons.appendChild(attachBtn);
    buttons.appendChild(cancelBtn);

    modal.appendChild(titleEl);
    modal.appendChild(filenameField);
    modal.appendChild(codeField);
    modal.appendChild(scopeSection);
    modal.appendChild(buttons);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    filenameInput.focus();
  }

  // Panel toggle listener
  pluginsToggle.addEventListener("click", function () {
    var isOpen = !pluginsPanel.classList.contains("hidden");
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
        invoke("import_aaoplug_global", { sourcePath: selected })
          .then(function (filenames) {
            statusMsg.textContent = "Imported " + filenames.length + " plugin(s) globally.";
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
