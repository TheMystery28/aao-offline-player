import { escapeHtml, createModal } from '../helpers.js';

/**
 * Plugin Params Modal — editable key-value params for a plugin at any cascade level.
 * level: "default" | "by_collection" | "by_sequence" | "by_case"
 * key: collection_id, sequence_title, or case_id (string). Empty for default.
 */
export function showPluginParamsModal(ctx, pluginFilename, levelLabel, level, key) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

  var m = createModal("<strong>Plugin Params &mdash; " + escapeHtml(pluginFilename) + "</strong><br>" +
    "<small>Level: " + escapeHtml(levelLabel) + "</small>");

  var content = document.createElement("div");
  content.style.cssText = "margin: 10px 0; max-height: 300px; overflow-y: auto;";

  var loadingMsg = document.createElement("div");
  loadingMsg.textContent = "Loading...";
  loadingMsg.className = "muted";
  content.appendChild(loadingMsg);

  var paramsData = {};
  var descriptorsCache = null; // filled after loading

  function renderParams(params) {
    content.innerHTML = "";
    // Merge descriptor keys into display (show all known params, not just overridden ones)
    var allKeys = Object.keys(params);
    if (descriptorsCache && typeof descriptorsCache === "object") {
      var descKeys = Object.keys(descriptorsCache);
      for (var dk = 0; dk < descKeys.length; dk++) {
        if (allKeys.indexOf(descKeys[dk]) === -1) {
          allKeys.push(descKeys[dk]);
        }
      }
    }
    if (allKeys.length === 0) {
      var emptyMsg = document.createElement("div");
      emptyMsg.className = "muted";
      emptyMsg.textContent = "No params set at this level. Add new params below.";
      content.appendChild(emptyMsg);
    }
    for (var i = 0; i < allKeys.length; i++) {
      (function(paramKey) {
        var row = document.createElement("div");
        row.style.cssText = "display:flex; align-items:center; gap:6px; margin:4px 0;";

        var desc = descriptorsCache && descriptorsCache[paramKey] ? descriptorsCache[paramKey] : null;
        var val = params[paramKey];
        // If no value set but descriptor has a default, show the default
        if (val === undefined && desc && desc["default"] !== undefined) {
          val = desc["default"];
        }

        var keyLabel = document.createElement("span");
        keyLabel.style.cssText = "min-width:100px; font-size:13px; color:#ccc;";
        keyLabel.textContent = (desc && desc.label) ? desc.label : paramKey;
        keyLabel.title = paramKey;

        var input;
        var paramType = desc ? desc.type : null;

        if (paramType === "number" || (paramType === null && typeof val === "number")) {
          // Number with optional range
          if (desc && (desc.min !== undefined || desc.max !== undefined)) {
            input = document.createElement("input");
            input.type = "range";
            input.min = String(desc.min !== undefined ? desc.min : 0);
            input.max = String(desc.max !== undefined ? desc.max : 100);
            input.step = String(desc.step !== undefined ? desc.step : 1);
            input.value = String(val !== undefined ? val : 0);
            var valSpan = document.createElement("span");
            valSpan.style.cssText = "min-width:30px; font-size:12px; color:#aaa;";
            valSpan.textContent = " " + input.value;
            input.addEventListener("input", function() {
              valSpan.textContent = " " + input.value;
              paramsData[paramKey] = parseFloat(input.value);
            });
            row.appendChild(keyLabel);
            row.appendChild(input);
            row.appendChild(valSpan);
          } else {
            input = document.createElement("input");
            input.type = "number";
            input.value = String(val !== undefined ? val : 0);
            input.step = "any";
            input.style.cssText = "width:80px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
            input.addEventListener("input", function() { paramsData[paramKey] = parseFloat(input.value) || 0; });
            row.appendChild(keyLabel);
            row.appendChild(input);
          }
        } else if (paramType === "checkbox" || (paramType === null && typeof val === "boolean")) {
          input = document.createElement("input");
          input.type = "checkbox";
          input.checked = !!val;
          input.addEventListener("change", function() { paramsData[paramKey] = input.checked; });
          row.appendChild(keyLabel);
          row.appendChild(input);
        } else if (paramType === "select" && desc && desc.options) {
          input = document.createElement("select");
          input.style.cssText = "background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
          var opts = desc.options || [];
          for (var oi = 0; oi < opts.length; oi++) {
            var opt = document.createElement("option");
            if (typeof opts[oi] === "object") {
              opt.value = opts[oi].value;
              opt.textContent = opts[oi].label;
            } else {
              opt.value = String(opts[oi]);
              opt.textContent = String(opts[oi]);
            }
            input.appendChild(opt);
          }
          input.value = String(val !== undefined ? val : "");
          input.addEventListener("change", function() { paramsData[paramKey] = input.value; });
          row.appendChild(keyLabel);
          row.appendChild(input);
        } else {
          // Text (default fallback)
          input = document.createElement("input");
          input.type = "text";
          input.value = String(val !== undefined ? val : "");
          input.style.cssText = "width:120px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
          input.addEventListener("input", function() { paramsData[paramKey] = input.value; });
          row.appendChild(keyLabel);
          row.appendChild(input);
        }

        // Only show delete button for params actually overridden at this level
        if (params[paramKey] !== undefined) {
          var delBtn = document.createElement("button");
          delBtn.textContent = "x";
          delBtn.className = "small-btn danger-btn";
          delBtn.style.cssText = "padding:1px 6px; font-size:11px;";
          delBtn.addEventListener("click", function() {
            delete paramsData[paramKey];
            renderParams(paramsData);
          });
          row.appendChild(delBtn);
        }
        content.appendChild(row);
      })(allKeys[i]);
    }
  }

  // Add new param row
  var addRow = document.createElement("div");
  addRow.style.cssText = "display:flex; gap:6px; margin-top:8px;";
  var addKeyInput = document.createElement("input");
  addKeyInput.type = "text";
  addKeyInput.placeholder = "param name";
  addKeyInput.style.cssText = "width:100px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
  var addValInput = document.createElement("input");
  addValInput.type = "text";
  addValInput.placeholder = "value";
  addValInput.style.cssText = "width:80px; background:rgba(0,0,0,0.3); color:#ddd; border:1px solid rgba(255,255,255,0.15); border-radius:3px; padding:2px 4px;";
  var addBtn = document.createElement("button");
  addBtn.className = "small-btn";
  addBtn.textContent = "+ Add";
  addBtn.addEventListener("click", function() {
    var k = addKeyInput.value.trim();
    if (!k) return;
    var v = addValInput.value.trim();
    // Auto-detect type
    if (v === "true") paramsData[k] = true;
    else if (v === "false") paramsData[k] = false;
    else if (v !== "" && !isNaN(Number(v))) paramsData[k] = Number(v);
    else paramsData[k] = v;
    addKeyInput.value = "";
    addValInput.value = "";
    renderParams(paramsData);
  });
  addRow.appendChild(addKeyInput);
  addRow.appendChild(addValInput);
  addRow.appendChild(addBtn);

  var btns = document.createElement("div");
  btns.className = "modal-buttons";

  var saveBtn = document.createElement("button");
  saveBtn.className = "modal-btn modal-btn-primary";
  saveBtn.textContent = "Save";
  saveBtn.addEventListener("click", function() {
    invoke("set_global_plugin_params", {
      filename: pluginFilename,
      level: level,
      key: key,
      params: paramsData
    }).then(function() {
      m.close();
      statusMsg.textContent = "Plugin params saved for " + levelLabel + ".";
    }).catch(function(e) {
      statusMsg.textContent = "Error saving params: " + e;
    });
  });

  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  cancelBtn.addEventListener("click", m.close);

  btns.appendChild(saveBtn);
  btns.appendChild(cancelBtn);

  m.content.appendChild(content);
  m.content.appendChild(addRow);
  m.modal.appendChild(btns);

  // Load descriptors + current params at this level
  Promise.all([
    invoke("get_plugin_descriptors", { filename: pluginFilename }).catch(function() { return null; }),
    invoke("get_plugin_params", { filename: pluginFilename })
  ]).then(function(results) {
    descriptorsCache = results[0]; // null if no descriptors
    var allParams = results[1];
    if (level === "default") {
      paramsData = (allParams && allParams["default"]) ? JSON.parse(JSON.stringify(allParams["default"])) : {};
    } else if (allParams && allParams[level] && allParams[level][key]) {
      paramsData = JSON.parse(JSON.stringify(allParams[level][key]));
    } else {
      paramsData = {};
    }
    renderParams(paramsData);
  }).catch(function(e) {
    content.innerHTML = "";
    var errMsg = document.createElement("div");
    errMsg.textContent = "Error loading params: " + e;
    errMsg.style.color = "#ff6b6b";
    content.appendChild(errMsg);
  });
}
