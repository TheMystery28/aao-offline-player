import { escapeHtml, createModal } from '../helpers.js';

/**
 * Scoped Plugin Modal — shows all global plugins with per-scope enable/disable toggle + params.
 * scopeType: "sequence" | "collection" | "case"
 * scopeKey: sequence title, collection ID, or case ID string
 * scopeLabel: display name like 'Sequence "My Seq"'
 */
export function showScopedPluginModal(ctx, scopeType, scopeKey, scopeLabel) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

  var m = createModal("<strong>Plugins &mdash; " + escapeHtml(scopeLabel) + "</strong>", { wide: true });

  var listContainer = document.createElement("div");
  listContainer.style.cssText = "margin: 10px 0; max-height: 350px; overflow-y: auto;";

  function refreshScopedList() {
    invoke("list_global_plugins").then(function (manifest) {
      var scripts = (manifest && manifest.scripts) || [];
      var plugins = (manifest && manifest.plugins) || {};
      listContainer.innerHTML = "";

      if (scripts.length === 0) {
        var empty = document.createElement("div");
        empty.className = "muted";
        empty.textContent = "No plugins installed.";
        listContainer.appendChild(empty);
        return;
      }

      for (var i = 0; i < scripts.length; i++) {
        (function (filename) {
          var pe = plugins[filename] || {};
          var globallyDisabled = !((pe.scope || {}).all === true);
          var pluginEntry = plugins[filename] || {};

          // Determine effective state for this scope
          var isEnabledForScope = false;
          var stateLabel = "";
          if (globallyDisabled) {
            // Check enabled_for
            var ef = pluginEntry.enabled_for || {};
            var fieldName = scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections");
            var arr = ef[fieldName] || [];
            var matchVal = scopeType === "case" ? Number(scopeKey) : scopeKey;
            isEnabledForScope = false;
            for (var ei = 0; ei < arr.length; ei++) {
              if (scopeType === "case" ? arr[ei] === matchVal : arr[ei] === matchVal) {
                isEnabledForScope = true;
                break;
              }
            }
            stateLabel = isEnabledForScope ? "enabled (override)" : "disabled (global)";
          } else {
            // Check disabled_for
            var df = pluginEntry.disabled_for || {};
            var fieldName2 = scopeType === "case" ? "cases" : (scopeType === "sequence" ? "sequences" : "collections");
            var arr2 = df[fieldName2] || [];
            var matchVal2 = scopeType === "case" ? Number(scopeKey) : scopeKey;
            var isDisabledForScope = false;
            for (var di = 0; di < arr2.length; di++) {
              if (scopeType === "case" ? arr2[di] === matchVal2 : arr2[di] === matchVal2) {
                isDisabledForScope = true;
                break;
              }
            }
            isEnabledForScope = !isDisabledForScope;
            stateLabel = isDisabledForScope ? "disabled (override)" : "enabled (global)";
          }

          var row = document.createElement("div");
          row.className = "global-plugin-row";

          var toggle = document.createElement("input");
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

          var name = document.createElement("span");
          name.className = "plugin-name";
          name.textContent = filename;

          var badge = document.createElement("span");
          badge.className = "scope-badge";
          badge.textContent = stateLabel;

          var paramsBtn = document.createElement("button");
          paramsBtn.className = "small-btn";
          paramsBtn.textContent = "Params";
          paramsBtn.style.cssText = "font-size:0.72rem; padding:0.1rem 0.5rem;";
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
    });
  }

  var closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";
  closeBtn.addEventListener("click", m.close);

  var scopedAttachBtn = document.createElement("button");
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
