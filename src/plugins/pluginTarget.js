import { createModal, groupCasesBySequence } from '../helpers.js';

/**
 * Plugin Target Modal — lets user select which cases to install a plugin into.
 * doImportPlugin — imports a .aaoplug file using the target modal.
 */
export function showPluginTargetModal(ctx, onConfirm) {
  const invoke = ctx.invoke;

  invoke("list_cases").then(function (cases) {
    const m = createModal("<strong>Select cases to install plugin</strong>", { wide: true });

    const selectAllLabel = document.createElement("label");
    selectAllLabel.className = "collection-picker-item";
    const selectAllCb = document.createElement("input");
    selectAllCb.type = "checkbox";
    const selectAllText = document.createElement("span");
    selectAllText.textContent = "Select All";
    selectAllText.style.fontWeight = "600";
    selectAllLabel.appendChild(selectAllCb);
    selectAllLabel.appendChild(selectAllText);

    const picker = document.createElement("div");
    picker.className = "collection-picker";

    const checkboxes = [];

    // Group by sequence
    const grouped = groupCasesBySequence(cases);
    const sequenceGroups = {};
    let groupKeys = Object.keys(grouped.sequenceGroups);
    for (let g = 0; g < groupKeys.length; g++) {
      sequenceGroups[groupKeys[g]] = grouped.sequenceGroups[groupKeys[g]].cases;
    }
    const standalone = grouped.standalone;

    groupKeys = Object.keys(sequenceGroups);
    if (groupKeys.length > 0) {
      const seqLabel = document.createElement("div");
      seqLabel.className = "collection-picker-group-label";
      seqLabel.textContent = "Sequences";
      picker.appendChild(seqLabel);

      for (let g = 0; g < groupKeys.length; g++) {
        const seqCases = sequenceGroups[groupKeys[g]];
        for (let sc = 0; sc < seqCases.length; sc++) {
          (function (cs) {
            const row = document.createElement("label");
            row.className = "collection-picker-item";
            const cb = document.createElement("input");
            cb.type = "checkbox";
            const label = document.createElement("span");
            label.textContent = cs.title;
            const meta = document.createElement("span");
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
      const caseLabel = document.createElement("div");
      caseLabel.className = "collection-picker-group-label";
      caseLabel.textContent = "Standalone Cases";
      picker.appendChild(caseLabel);

      for (let s = 0; s < standalone.length; s++) {
        (function (cs) {
          const row = document.createElement("label");
          row.className = "collection-picker-item";
          const cb = document.createElement("input");
          cb.type = "checkbox";
          const label = document.createElement("span");
          label.textContent = cs.title;
          const meta = document.createElement("span");
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
      for (let j = 0; j < checkboxes.length; j++) {
        checkboxes[j].checkbox.checked = selectAllCb.checked;
      }
    });

    const buttons = document.createElement("div");
    buttons.className = "modal-row-buttons";

    const installBtn = document.createElement("button");
    installBtn.className = "modal-btn modal-btn-primary";
    installBtn.textContent = "Install";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "modal-btn modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    installBtn.addEventListener("click", function () {
      const selected = [];
      for (let j = 0; j < checkboxes.length; j++) {
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
  const invoke = ctx.invoke;
  const statusMsg = ctx.statusMsg;
  const loadLibrary = ctx.loadLibrary;

  const importResult = document.getElementById("import-result");
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
