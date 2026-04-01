import { createModal, groupCasesBySequence } from '../helpers.js';

/**
 * Plugin Target Modal — lets user select which cases to install a plugin into.
 * doImportPlugin — imports a .aaoplug file using the target modal.
 */
export function showPluginTargetModal(ctx, onConfirm) {
  var invoke = ctx.invoke;

  invoke("list_cases").then(function (cases) {
    var m = createModal("<strong>Select cases to install plugin</strong>", { wide: true });

    var selectAllLabel = document.createElement("label");
    selectAllLabel.className = "collection-picker-item";
    var selectAllCb = document.createElement("input");
    selectAllCb.type = "checkbox";
    var selectAllText = document.createElement("span");
    selectAllText.textContent = "Select All";
    selectAllText.style.fontWeight = "600";
    selectAllLabel.appendChild(selectAllCb);
    selectAllLabel.appendChild(selectAllText);

    var picker = document.createElement("div");
    picker.className = "collection-picker";

    var checkboxes = [];

    // Group by sequence
    var grouped = groupCasesBySequence(cases);
    var sequenceGroups = {};
    var groupKeys = Object.keys(grouped.sequenceGroups);
    for (var g = 0; g < groupKeys.length; g++) {
      sequenceGroups[groupKeys[g]] = grouped.sequenceGroups[groupKeys[g]].cases;
    }
    var standalone = grouped.standalone;

    var groupKeys = Object.keys(sequenceGroups);
    if (groupKeys.length > 0) {
      var seqLabel = document.createElement("div");
      seqLabel.className = "collection-picker-group-label";
      seqLabel.textContent = "Sequences";
      picker.appendChild(seqLabel);

      for (var g = 0; g < groupKeys.length; g++) {
        var seqCases = sequenceGroups[groupKeys[g]];
        for (var sc = 0; sc < seqCases.length; sc++) {
          (function (cs) {
            var row = document.createElement("label");
            row.className = "collection-picker-item";
            var cb = document.createElement("input");
            cb.type = "checkbox";
            var label = document.createElement("span");
            label.textContent = cs.title;
            var meta = document.createElement("span");
            meta.className = "picker-item-meta";
            meta.textContent = "ID " + cs.case_id;
            row.appendChild(cb);
            row.appendChild(label);
            row.appendChild(meta);
            picker.appendChild(row);
            checkboxes.push({ checkbox: cb, caseId: cs.case_id });
          })(seqCases[sc]);
        }
      }
    }

    if (standalone.length > 0) {
      var caseLabel = document.createElement("div");
      caseLabel.className = "collection-picker-group-label";
      caseLabel.textContent = "Standalone Cases";
      picker.appendChild(caseLabel);

      for (var s = 0; s < standalone.length; s++) {
        (function (cs) {
          var row = document.createElement("label");
          row.className = "collection-picker-item";
          var cb = document.createElement("input");
          cb.type = "checkbox";
          var label = document.createElement("span");
          label.textContent = cs.title;
          var meta = document.createElement("span");
          meta.className = "picker-item-meta";
          meta.textContent = "ID " + cs.case_id;
          row.appendChild(cb);
          row.appendChild(label);
          row.appendChild(meta);
          picker.appendChild(row);
          checkboxes.push({ checkbox: cb, caseId: cs.case_id });
        })(standalone[s]);
      }
    }

    selectAllCb.addEventListener("change", function () {
      for (var j = 0; j < checkboxes.length; j++) {
        checkboxes[j].checkbox.checked = selectAllCb.checked;
      }
    });

    var buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    var installBtn = document.createElement("button");
    installBtn.className = "modal-btn modal-btn-primary";
    installBtn.textContent = "Install";

    var cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    installBtn.addEventListener("click", function () {
      var selected = [];
      for (var j = 0; j < checkboxes.length; j++) {
        if (checkboxes[j].checkbox.checked) {
          selected.push(checkboxes[j].caseId);
        }
      }
      if (selected.length === 0) {
        picker.style.borderColor = "#a33";
        return;
      }
      m.close();
      onConfirm(selected);
    });

    cancelBtn.addEventListener("click", m.close);

    buttons.appendChild(installBtn);
    buttons.appendChild(cancelBtn);

    m.content.appendChild(selectAllLabel);
    m.content.appendChild(picker);
    m.modal.appendChild(buttons);
  });
}

export function doImportPlugin(ctx, pluginPath) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;
  var loadLibrary = ctx.loadLibrary;

  var importResult = document.getElementById("import-result");
  ctx.showPluginTargetModal(function (caseIds) {
    if (importResult) {
      importResult.textContent = "";
      importResult.className = "";
    }
    statusMsg.textContent = "Installing plugin to " + caseIds.length + " case(s)...";

    invoke("import_plugin", {
      sourcePath: pluginPath,
      targetCaseIds: caseIds
    })
    .then(function (importedIds) {
      if (importResult) {
        importResult.innerHTML = "Plugin installed to <strong>" +
          importedIds.length + " case(s)</strong>";
        importResult.className = "result-success";
      }
      statusMsg.textContent = "";
      loadLibrary();
    })
    .catch(function (e) {
      if (importResult) {
        importResult.textContent = "Plugin import error: " + e;
        importResult.className = "result-error";
      }
      statusMsg.textContent = "";
    });
  });
}
