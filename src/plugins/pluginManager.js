import { escapeHtml, showConfirmModal, createModal } from '../helpers.js';

/**
 * Plugin Manager Modal — shows case plugins + global plugins for a single case.
 */
export function showPluginManagerModal(ctx, caseId, caseTitle) {
  const invoke = ctx.invoke;
  const statusMsg = ctx.statusMsg;
  const loadLibrary = ctx.loadLibrary;

  const m = createModal("<strong>Plugins &mdash; " + escapeHtml(caseTitle) + "</strong>", { wide: true });

  const listContainer = document.createElement("div");
  listContainer.className = "plugin-list";

  const actionsRow = document.createElement("div");
  actionsRow.className = "plugin-actions-row";

  const importBtn = document.createElement("button");
  importBtn.className = "modal-btn modal-btn-primary";
  importBtn.textContent = "Import .aaoplug";

  const attachBtn = document.createElement("button");
  attachBtn.className = "modal-btn modal-btn-secondary";
  attachBtn.textContent = "Attach Code";

  actionsRow.appendChild(importBtn);
  actionsRow.appendChild(attachBtn);

  const closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";

  function close() {
    m.close();
    loadLibrary();
  }

  function refreshList() {
    invoke("list_plugins", { caseId: caseId })
      .then(function (manifest) {
        const scripts = (manifest && manifest.scripts) || [];
        const disabledList = (manifest && Array.isArray(manifest.disabled)) ? manifest.disabled : [];
        listContainer.innerHTML = "";
        if (scripts.length === 0) {
          const empty = document.createElement("div");
          empty.className = "plugin-list-empty";
          empty.textContent = "No plugins installed.";
          listContainer.appendChild(empty);
        } else {
          for (let i = 0; i < scripts.length; i++) {
            (function (filename) {
              const isDisabled = disabledList.indexOf(filename) !== -1;
              const item = document.createElement("div");
              item.className = "plugin-list-item" + (isDisabled ? " disabled" : "");

              const toggle = document.createElement("input");
              toggle.type = "checkbox";
              toggle.checked = !isDisabled;
              toggle.title = isDisabled ? "Enable plugin" : "Disable plugin";
              toggle.style.accentColor = "#4a90d9";
              toggle.style.width = "1rem";
              toggle.style.height = "1rem";
              toggle.style.flexShrink = "0";
              toggle.addEventListener("change", function () {
                invoke("toggle_plugin", { caseId: caseId, filename: filename, enabled: toggle.checked })
                  .then(function () { refreshList(); })
                  .catch(function (e) { statusMsg.textContent = "Error toggling plugin: " + e; });
              });

              const name = document.createElement("span");
              name.className = "plugin-name";
              name.textContent = filename;

              const removeBtn = document.createElement("button");
              removeBtn.className = "plugin-remove-btn";
              removeBtn.textContent = "Remove";
              removeBtn.addEventListener("click", function () {
                showConfirmModal(
                  "Remove plugin \"" + filename + "\"?",
                  "Remove",
                  function () {
                    invoke("remove_plugin", { caseId: caseId, filename: filename })
                      .then(function () { refreshList(); })
                      .catch(function (e) { statusMsg.textContent = "Error removing plugin: " + e; });
                  }
                );
              });

              const paramsBtn = document.createElement("button");
              paramsBtn.className = "small-btn";
              paramsBtn.textContent = "Params";
              paramsBtn.className += " btn-small";
              paramsBtn.style.marginLeft = "auto";
              paramsBtn.addEventListener("click", function () {
                ctx.showPluginParamsModal(filename, "Case " + caseId, "by_case", String(caseId));
              });

              item.appendChild(toggle);
              item.appendChild(name);
              item.appendChild(paramsBtn);
              item.appendChild(removeBtn);
              listContainer.appendChild(item);
            })(scripts[i]);
          }
        }
      })
      .catch(function (e) {
        listContainer.innerHTML = "";
        const errEl = document.createElement("div");
        errEl.className = "plugin-list-empty";
        errEl.textContent = "Error loading plugins: " + e;
        listContainer.appendChild(errEl);
      });
  }

  importBtn.addEventListener("click", function () {
    invoke("pick_import_file")
      .then(function (selected) {
        if (!selected) return;
        if (!selected.toLowerCase().endsWith(".aaoplug")) {
          statusMsg.textContent = "Please select a .aaoplug file.";
          return;
        }
        statusMsg.textContent = "Installing plugin...";
        invoke("import_plugin", {
          sourcePath: selected,
          targetCaseIds: [caseId]
        })
        .then(function () {
          statusMsg.textContent = "Plugin installed.";
          refreshGlobalList();
          refreshList();
        })
        .catch(function (e) {
          statusMsg.textContent = "Plugin import error: " + e;
        });
      })
      .catch(function (e) {
        statusMsg.textContent = "Could not open file picker: " + e;
      });
  });

  attachBtn.addEventListener("click", function () {
    ctx.showAttachCodeModal(caseId, caseTitle, function () {
      refreshGlobalList();
      refreshList();
    });
  });

  closeBtn.addEventListener("click", close);

  // Global plugins section
  const globalLabel = document.createElement("div");
  globalLabel.className = "section-label";
  globalLabel.textContent = "Global Plugins";

  const globalListContainer = document.createElement("div");
  globalListContainer.className = "plugin-list";

  function refreshGlobalList() {
    Promise.all([
      invoke("list_global_plugins"),
      invoke("list_plugins", { caseId: caseId })
    ]).then(function (results) {
      const globalManifest = results[0];
      const caseState = results[1];

      const scripts = (globalManifest && globalManifest.scripts) || [];
      const plugins = (globalManifest && globalManifest.plugins) || {};

      const activeForCase = {};
      const caseScripts = (caseState && caseState.scripts) || [];
      for (let a = 0; a < caseScripts.length; a++) {
        activeForCase[caseScripts[a]] = true;
      }

      globalListContainer.innerHTML = "";
      if (scripts.length === 0) {
        const empty = document.createElement("div");
        empty.className = "plugin-list-empty";
        empty.textContent = "No plugins installed.";
        globalListContainer.appendChild(empty);
      } else {
        for (let i = 0; i < scripts.length; i++) {
          (function (filename) {
            const pe = plugins[filename] || {};
            const scope = pe.scope || {};
            const isActiveForCase = !!activeForCase[filename];
            const isDisabled = !isActiveForCase;

            const item = document.createElement("div");
            item.className = "plugin-list-item" + (isDisabled ? " disabled" : "");

            const toggle = document.createElement("input");
            toggle.type = "checkbox";
            toggle.checked = isActiveForCase;
            toggle.title = isActiveForCase ? "Disable for this case" : "Enable for this case";
            toggle.style.accentColor = "#4a90d9";
            toggle.style.width = "1rem";
            toggle.style.height = "1rem";
            toggle.style.flexShrink = "0";
            toggle.addEventListener("change", function () {
              invoke("toggle_plugin_for_scope", {
                filename: filename,
                scopeType: "case",
                scopeKey: String(caseId),
                enabled: toggle.checked
              })
                .then(function () { refreshGlobalList(); refreshList(); })
                .catch(function (e) { statusMsg.textContent = "Error: " + e; });
            });

            const name = document.createElement("span");
            name.className = "plugin-name";
            name.textContent = filename;

            const badge = document.createElement("span");
            badge.style.cssText = "font-size:10px;color:#888;margin-left:4px;flex-shrink:0;";
            if (scope.all === true) {
              badge.textContent = isActiveForCase ? "(global)" : "(global, excluded)";
            } else if (isActiveForCase) {
              badge.textContent = "(enabled)";
            }

            const removeBtn = document.createElement("button");
            removeBtn.className = "plugin-remove-btn";
            removeBtn.textContent = "Remove";
            removeBtn.addEventListener("click", function () {
              showConfirmModal("Remove global plugin \"" + filename + "\"?", "Remove", function () {
                invoke("remove_global_plugin", { filename: filename })
                  .then(function () { refreshGlobalList(); refreshList(); })
                  .catch(function (e) { statusMsg.textContent = "Error: " + e; });
              });
            });

            item.appendChild(toggle);
            item.appendChild(name);
            item.appendChild(badge);
            item.appendChild(removeBtn);
            globalListContainer.appendChild(item);
          })(scripts[i]);
        }
      }
    }).catch(function(e) { console.error("[PLUGINS] Failed to refresh plugin manager:", e); });
  }

  const caseLabel = document.createElement("div");
  caseLabel.className = "section-label";
  caseLabel.style.marginTop = "0.75rem";
  caseLabel.textContent = "Case Plugins";

  m.content.appendChild(globalLabel);
  m.content.appendChild(globalListContainer);
  m.content.appendChild(caseLabel);
  m.content.appendChild(listContainer);
  m.content.appendChild(actionsRow);
  m.modal.appendChild(closeBtn);

  refreshGlobalList();
  refreshList();
}
