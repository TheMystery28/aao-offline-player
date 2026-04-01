import { escapeHtml, showConfirmModal, createModal, groupCasesBySequence } from '../helpers.js';

/**
 * Show the "New Collection" modal — gathers cases/sequences and opens the picker.
 */
export function showNewCollectionModal(ctx) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

  // Gather all cases and sequence groups for the picker
  invoke("list_cases").then(function (cases) {
    invoke("list_collections").catch(function () { return []; }).then(function (collections) {
      // Build sequence groups
      var grouped = groupCasesBySequence(cases);
      var sequenceGroups = grouped.sequenceGroups;
      var standalone = grouped.standalone;

      // Determine which items are already claimed
      var claimedCaseIds = {};
      var claimedSequenceTitles = {};
      for (var col = 0; col < collections.length; col++) {
        var items = collections[col].items || [];
        for (var it = 0; it < items.length; it++) {
          if (items[it].type === "case") claimedCaseIds[items[it].case_id] = true;
          else if (items[it].type === "sequence") claimedSequenceTitles[items[it].title] = true;
        }
      }

      showCollectionPickerModal(
        ctx,
        "New Collection",
        "",
        sequenceGroups,
        standalone,
        claimedCaseIds,
        claimedSequenceTitles,
        function (title, selectedItems) {
          invoke("create_collection", { title: title, items: selectedItems })
            .then(function () { ctx.loadLibrary(); })
            .catch(function (e) { statusMsg.textContent = "Error creating collection: " + e; });
        }
      );
    });
  });
}

function showCollectionPickerModal(ctx, modalTitle, existingTitle, sequenceGroups, standalone, claimedCaseIds, claimedSequenceTitles, onSave) {
  var m = createModal("<strong>" + escapeHtml(modalTitle) + "</strong>", { wide: true });

  // Title
  var titleField = document.createElement("div");
  titleField.className = "modal-field";
  var titleLabel = document.createElement("label");
  titleLabel.textContent = "Collection Title";
  var titleInput = document.createElement("input");
  titleInput.type = "text";
  titleInput.placeholder = "My Collection";
  titleInput.value = existingTitle;
  titleField.appendChild(titleLabel);
  titleField.appendChild(titleInput);

  // Picker
  var pickerField = document.createElement("div");
  pickerField.className = "modal-field";
  var pickerLabel = document.createElement("label");
  pickerLabel.textContent = "Select Items";
  var picker = document.createElement("div");
  picker.className = "collection-picker";

  // Build picker items
  var groupKeys = Object.keys(sequenceGroups);
  var checkboxes = []; // { checkbox, type, value }

  if (groupKeys.length > 0) {
    var seqLabel = document.createElement("div");
    seqLabel.className = "collection-picker-group-label";
    seqLabel.textContent = "Sequences";
    picker.appendChild(seqLabel);

    for (var g = 0; g < groupKeys.length; g++) {
      (function (seqTitle) {
        if (claimedSequenceTitles[seqTitle]) return;
        var sg = sequenceGroups[seqTitle];
        var row = document.createElement("label");
        row.className = "collection-picker-item";
        var cb = document.createElement("input");
        cb.type = "checkbox";
        var label = document.createElement("span");
        label.textContent = seqTitle;
        var meta = document.createElement("span");
        meta.className = "picker-item-meta";
        meta.textContent = sg.cases.length + "/" + sg.list.length + " parts";
        row.appendChild(cb);
        row.appendChild(label);
        row.appendChild(meta);
        picker.appendChild(row);
        checkboxes.push({ checkbox: cb, type: "sequence", value: seqTitle });
      })(groupKeys[g]);
    }
  }

  if (standalone.length > 0) {
    var caseLabel = document.createElement("div");
    caseLabel.className = "collection-picker-group-label";
    caseLabel.textContent = "Standalone Cases";
    picker.appendChild(caseLabel);

    for (var s = 0; s < standalone.length; s++) {
      (function (cs) {
        if (claimedCaseIds[cs.case_id]) return;
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
        checkboxes.push({ checkbox: cb, type: "case", value: cs.case_id });
      })(standalone[s]);
    }
  }

  pickerField.appendChild(pickerLabel);
  pickerField.appendChild(picker);

  // Buttons
  var buttons = document.createElement("div");
  buttons.className = "modal-row-buttons";

  var createBtn = document.createElement("button");
  createBtn.className = "modal-btn modal-btn-secondary";
  createBtn.textContent = modalTitle === "New Collection" ? "Create" : "Save";

  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";

  createBtn.addEventListener("click", function () {
    var title = titleInput.value.trim();
    if (!title) {
      titleInput.style.borderColor = "#a33";
      titleInput.focus();
      return;
    }
    var selectedItems = [];
    for (var i = 0; i < checkboxes.length; i++) {
      if (checkboxes[i].checkbox.checked) {
        if (checkboxes[i].type === "sequence") {
          selectedItems.push({ type: "sequence", title: checkboxes[i].value });
        } else {
          selectedItems.push({ type: "case", case_id: checkboxes[i].value });
        }
      }
    }
    if (selectedItems.length === 0) {
      picker.style.borderColor = "#a33";
      return;
    }
    m.close();
    onSave(title, selectedItems);
  });

  cancelBtn.addEventListener("click", m.close);

  buttons.appendChild(createBtn);
  buttons.appendChild(cancelBtn);

  m.content.appendChild(titleField);
  m.content.appendChild(pickerField);
  m.modal.appendChild(buttons);

  titleInput.focus();
}

/**
 * Show the "Edit Collection" modal — fetches case titles, then opens the inner editor.
 */
export function showEditCollectionModal(ctx, collection) {
  var invoke = ctx.invoke;

  // Fetch case titles for display
  invoke("list_cases").then(function (cases) {
    var caseTitles = {};
    for (var ci = 0; ci < cases.length; ci++) {
      caseTitles[cases[ci].case_id] = cases[ci].title;
    }
    _showEditCollectionModalInner(ctx, collection, caseTitles);
  });
}

function _showEditCollectionModalInner(ctx, collection, caseTitles) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

  var m = createModal("<strong>Edit Collection</strong>", { wide: true });

  // Title field
  var titleField = document.createElement("div");
  titleField.className = "modal-field";
  var titleLabel = document.createElement("label");
  titleLabel.textContent = "Collection Title";
  var titleInput = document.createElement("input");
  titleInput.type = "text";
  titleInput.value = collection.title;
  titleField.appendChild(titleLabel);
  titleField.appendChild(titleInput);

  // Current items (reorderable)
  var itemsLabel = document.createElement("label");
  itemsLabel.textContent = "Items (drag to reorder)";
  itemsLabel.style.display = "block";
  itemsLabel.style.color = "#999";
  itemsLabel.style.fontSize = "0.82rem";
  itemsLabel.style.fontWeight = "500";
  itemsLabel.style.marginBottom = "0.35rem";
  itemsLabel.style.textTransform = "uppercase";
  itemsLabel.style.letterSpacing = "0.04em";

  var editItems = [];
  for (var i = 0; i < (collection.items || []).length; i++) {
    editItems.push({ type: collection.items[i].type, title: collection.items[i].title, case_id: collection.items[i].case_id });
  }

  var editListEl = document.createElement("div");
  editListEl.className = "collection-edit-list";

  function renderEditList() {
    editListEl.innerHTML = "";
    if (editItems.length === 0) {
      var empty = document.createElement("div");
      empty.className = "collection-edit-list-empty";
      empty.textContent = "No items. Add some below.";
      editListEl.appendChild(empty);
      return;
    }
    for (var i = 0; i < editItems.length; i++) {
      (function (idx) {
        var item = editItems[idx];
        var row = document.createElement("div");
        row.className = "collection-edit-item";
        row.draggable = true;
        row.dataset.index = idx;

        var handle = document.createElement("span");
        handle.className = "drag-handle";
        handle.textContent = "\u2630"; // hamburger

        var label = document.createElement("span");
        label.className = "edit-item-label";
        label.textContent = item.type === "sequence" ? item.title : (caseTitles[item.case_id] || ("Case " + item.case_id));

        var typeTag = document.createElement("span");
        typeTag.className = "edit-item-type";
        typeTag.textContent = item.type;

        var removeBtn = document.createElement("button");
        removeBtn.className = "edit-item-remove";
        removeBtn.textContent = "\u2715"; // x
        removeBtn.title = "Remove from collection";
        removeBtn.addEventListener("click", function () {
          editItems.splice(idx, 1);
          renderEditList();
        });

        // DnD events
        row.addEventListener("dragstart", function (e) {
          e.dataTransfer.effectAllowed = "move";
          e.dataTransfer.setData("text/plain", String(idx));
        });

        row.addEventListener("dragover", function (e) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          row.classList.add("drag-over");
        });

        row.addEventListener("dragleave", function () {
          row.classList.remove("drag-over");
        });

        row.addEventListener("drop", function (e) {
          e.preventDefault();
          row.classList.remove("drag-over");
          var fromIdx = parseInt(e.dataTransfer.getData("text/plain"), 10);
          var toIdx = idx;
          if (fromIdx === toIdx) return;
          var moved = editItems.splice(fromIdx, 1)[0];
          editItems.splice(toIdx, 0, moved);
          renderEditList();
        });

        row.appendChild(handle);
        row.appendChild(label);
        row.appendChild(typeTag);
        row.appendChild(removeBtn);
        editListEl.appendChild(row);
      })(i);
    }
  }

  renderEditList();

  // Add Items button
  var addItemsBtn = document.createElement("button");
  addItemsBtn.className = "modal-add-items-btn";
  addItemsBtn.textContent = "+ Add Items";
  addItemsBtn.addEventListener("click", function () {
    showAddItemsSubModal(ctx, editItems, collection.id, function (newItems) {
      for (var n = 0; n < newItems.length; n++) {
        editItems.push(newItems[n]);
      }
      renderEditList();
    });
  });

  // Buttons
  var buttons = document.createElement("div");
  buttons.className = "modal-row-buttons";

  var saveBtn = document.createElement("button");
  saveBtn.className = "modal-btn modal-btn-secondary";
  saveBtn.textContent = "Save";

  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";

  saveBtn.addEventListener("click", function () {
    var title = titleInput.value.trim();
    if (!title) {
      titleInput.style.borderColor = "#a33";
      titleInput.focus();
      return;
    }
    m.close();
    invoke("update_collection", { id: collection.id, title: title, items: editItems })
      .then(function () { ctx.loadLibrary(); })
      .catch(function (e) { statusMsg.textContent = "Error updating collection: " + e; });
  });

  cancelBtn.addEventListener("click", m.close);

  buttons.appendChild(saveBtn);
  buttons.appendChild(cancelBtn);

  m.content.appendChild(titleField);
  m.content.appendChild(itemsLabel);
  m.content.appendChild(editListEl);
  m.content.appendChild(addItemsBtn);
  m.modal.appendChild(buttons);

  titleInput.focus();
}

/**
 * Sub-modal for adding items to an existing collection being edited.
 * Shows uncollected items (not in any collection, and not in the current edit list).
 */
export function showAddItemsSubModal(ctx, currentEditItems, currentCollectionId, onAdd) {
  var invoke = ctx.invoke;

  invoke("list_cases").then(function (cases) {
    invoke("list_collections").catch(function () { return []; }).then(function (collections) {
      var grouped = groupCasesBySequence(cases);
      var sequenceGroups = grouped.sequenceGroups;
      var standalone = grouped.standalone;

      // Items claimed by OTHER collections (exclude current collection being edited)
      var claimedCaseIds = {};
      var claimedSequenceTitles = {};
      for (var col = 0; col < collections.length; col++) {
        if (collections[col].id === currentCollectionId) continue;
        var items = collections[col].items || [];
        for (var it = 0; it < items.length; it++) {
          if (items[it].type === "case") claimedCaseIds[items[it].case_id] = true;
          else if (items[it].type === "sequence") claimedSequenceTitles[items[it].title] = true;
        }
      }

      // Items already in the edit list
      for (var ei = 0; ei < currentEditItems.length; ei++) {
        if (currentEditItems[ei].type === "case") claimedCaseIds[currentEditItems[ei].case_id] = true;
        else if (currentEditItems[ei].type === "sequence") claimedSequenceTitles[currentEditItems[ei].title] = true;
      }

      var m = createModal("Select items to add:", { wide: true });

      var picker = document.createElement("div");
      picker.className = "collection-picker";

      var groupKeys = Object.keys(sequenceGroups);
      var checkboxes = [];

      if (groupKeys.length > 0) {
        var seqLabel = document.createElement("div");
        seqLabel.className = "collection-picker-group-label";
        seqLabel.textContent = "Sequences";
        picker.appendChild(seqLabel);

        for (var g = 0; g < groupKeys.length; g++) {
          (function (seqTitle) {
            if (claimedSequenceTitles[seqTitle]) return;
            var sg = sequenceGroups[seqTitle];
            var row = document.createElement("label");
            row.className = "collection-picker-item";
            var cb = document.createElement("input");
            cb.type = "checkbox";
            var lbl = document.createElement("span");
            lbl.textContent = seqTitle;
            var meta = document.createElement("span");
            meta.className = "picker-item-meta";
            meta.textContent = sg.cases.length + "/" + sg.list.length + " parts";
            row.appendChild(cb);
            row.appendChild(lbl);
            row.appendChild(meta);
            picker.appendChild(row);
            checkboxes.push({ checkbox: cb, type: "sequence", value: seqTitle });
          })(groupKeys[g]);
        }
      }

      if (standalone.length > 0) {
        var caseLabel = document.createElement("div");
        caseLabel.className = "collection-picker-group-label";
        caseLabel.textContent = "Standalone Cases";
        picker.appendChild(caseLabel);

        for (var s = 0; s < standalone.length; s++) {
          (function (cs) {
            if (claimedCaseIds[cs.case_id]) return;
            var row = document.createElement("label");
            row.className = "collection-picker-item";
            var cb = document.createElement("input");
            cb.type = "checkbox";
            var lbl = document.createElement("span");
            lbl.textContent = cs.title;
            var meta = document.createElement("span");
            meta.className = "picker-item-meta";
            meta.textContent = "ID " + cs.case_id;
            row.appendChild(cb);
            row.appendChild(lbl);
            row.appendChild(meta);
            picker.appendChild(row);
            checkboxes.push({ checkbox: cb, type: "case", value: cs.case_id, title: cs.title });
          })(standalone[s]);
        }
      }

      var buttons = document.createElement("div");
      buttons.className = "modal-row-buttons";

      var addBtn = document.createElement("button");
      addBtn.className = "modal-btn modal-btn-secondary";
      addBtn.textContent = "Add Selected";

      var cancelBtn = document.createElement("button");
      cancelBtn.className = "modal-btn modal-btn-cancel";
      cancelBtn.textContent = "Cancel";

      addBtn.addEventListener("click", function () {
        var newItems = [];
        for (var i = 0; i < checkboxes.length; i++) {
          if (checkboxes[i].checkbox.checked) {
            if (checkboxes[i].type === "sequence") {
              newItems.push({ type: "sequence", title: checkboxes[i].value });
            } else {
              newItems.push({ type: "case", case_id: checkboxes[i].value, title: checkboxes[i].title });
            }
          }
        }
        m.close();
        if (newItems.length > 0) onAdd(newItems);
      });

      cancelBtn.addEventListener("click", m.close);

      buttons.appendChild(addBtn);
      buttons.appendChild(cancelBtn);

      m.content.appendChild(picker);
      m.modal.appendChild(buttons);
    });
  });
}
