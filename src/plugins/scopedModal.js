import { escapeHtml, createModal } from '../helpers.js';

/**
 * Scoped Plugin Modal — shows all global plugins with per-scope enable/disable toggle + params.
 * scopeType: "sequence" | "collection" | "case"
 * scopeKey: sequence title, collection ID, or case ID string
 * scopeLabel: display name like 'Sequence "My Seq"'
 */
export function showScopedPluginModal(ctx, scopeType, scopeKey, scopeLabel) {
  const invoke = ctx.invoke;
  const statusMsg = ctx.statusMsg;

  const m = createModal("<strong>Plugins &mdash; " + escapeHtml(scopeLabel) + "</strong>", { wide: true });

  const listContainer = document.createElement("div");
  listContainer.className = "scroll-panel";

  function refreshScopedList() {
    invoke("list_global_plugins").then(function (manifest) {
      const scripts = (manifest && manifest.scripts) || [];
      const plugins = (manifest && manifest.plugins) || {};
      listContainer.innerHTML = "";

      if (scripts.length === 0) {
        const empty = document.createElement("div");
        empty.className = "muted";
        empty.textContent = "No plugins installed.";
        listContainer.appendChild(empty);
        return;
      }

      for (let i = 0; i < scripts.length; i++) {
        (function (filename) {

          const pe = plugins[filename] || {};
          const globallyDisabled = !((pe.scope || {}).all === true);
          const pluginEntry = plugins[filename] || {};

          // Determine effective state for this scope
          let isEnabledForScope = false;
          let stateLabel = "";
          if (globallyDisabled) {
            // Check enabled_for
            const ef = pluginEntry.enabled_for || {};
            const fieldName = scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections");
            const arr = ef[fieldName] || [];
            const matchVal = scopeType === "case" ? Number(scopeKey) : scopeKey;
            isEnabledForScope = false;
            for (let ei = 0; ei < arr.length; ei++) {
              if (scopeType === "case" ? arr[ei] === matchVal : arr[ei] === matchVal) {
                isEnabledForScope = true;
                break;
              }
            }
            stateLabel = isEnabledForScope ? "enabled (override)" : "disabled (global)";
          } else {
            // Check disabled_for
            const df = pluginEntry.disabled_for || {};
            const fieldName2 = scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections");
            const arr2 = df[fieldName2] || [];
            const matchVal2 = scopeType === "case" ? Number(scopeKey) : scopeKey;
            let isDisabledForScope = false;
            for (let di = 0; di < arr2.length; di++) {
              if (scopeType === "case" ? arr2[di] === matchVal2 : arr2[di] === matchVal2) {
                isDisabledForScope = true;
                break;
              }
            }
            isEnabledForScope = !isDisabledForScope;
            stateLabel = isDisabledForScope ? "disabled (override)" : "enabled (global)";
          }

          const row = document.createElement("div");
          row.className = "global-plugin-row";

          const toggle = document.createElement("input");
          toggle.type = "checkbox";
          toggle.checked = isEnabledForScope;
          toggle.style.accentColor = "#4a90d9";
          toggle.style.width = "1rem";
          toggle.style.height = "1rem";
          toggle.style.flexShrink = "0";
          toggle.addEventListener("change", function () {
            invoke("toggle_plugin_for_scope", {
              filename: filename,
              scopeType: scopeType,
              scopeKey: scopeKey,
              enabled: toggle.checked
            }).then(function () {
              refreshScopedList();
            }).catch(function (e) {
              statusMsg.textContent = "Error: " + e;
            });
          });

          const name = document.createElement("span");
          name.className = "plugin-name";
          name.textContent = filename;

          const badge = document.createElement("span");
          badge.className = "scope-badge";
          badge.textContent = stateLabel;

          const paramsBtn = document.createElement("button");
          paramsBtn.className = "small-btn";
          paramsBtn.textContent = "Params";
          paramsBtn.className += " btn-small";
          paramsBtn.addEventListener("click", (function (fn) {
            return function () {
              ctx.showPluginParamsModal(fn, scopeLabel, "by_" + scopeType, scopeKey);
            };
          })(filename));

          row.appendChild(toggle);
          row.appendChild(name);
          row.appendChild(badge);
          row.appendChild(paramsBtn);
          listContainer.appendChild(row);
        })(scripts[i]);
      }
    }).catch(function(e) { console.error("[PLUGINS] Failed to load scoped list:", e); });
  }

  const closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";
  closeBtn.addEventListener("click", m.close);

  const scopedAttachBtn = document.createElement("button");
  scopedAttachBtn.className = "small-btn";
  scopedAttachBtn.textContent = "Attach Code";
  scopedAttachBtn.style.cssText = "width:100%; margin:0.5rem 0;";
  scopedAttachBtn.addEventListener("click", function () {
    m.close();
    ctx.showGlobalAttachCodeModal(function (attachedFilename) {
      if (!attachedFilename) return;
      invoke("toggle_plugin_for_scope", {
        filename: attachedFilename,
        scopeType: scopeType,
        scopeKey: scopeKey,
        enabled: true
      }).then(function () {
        ctx.showScopedPluginModal(scopeType, scopeKey, scopeLabel);
      }).catch(function (e) {
        statusMsg.textContent = "Error: " + e;
      });
    });
  });

  m.content.appendChild(listContainer);
  m.content.appendChild(scopedAttachBtn);
  m.modal.appendChild(closeBtn);

  refreshScopedList();
}
