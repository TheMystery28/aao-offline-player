/**
 * Plugin Picker Modal — lets user select one plugin from a list of script names.
 */
export function showPluginPickerModal(ctx, scripts, onSelect) {
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog";

  var titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.innerHTML = "<strong>Select Plugin</strong>";

  var listEl = document.createElement("div");
  listEl.style.cssText = "display:flex; flex-direction:column; gap:6px; margin:10px 0;";

  for (var i = 0; i < scripts.length; i++) {
    (function (scriptName) {
      var btn = document.createElement("button");
      btn.className = "modal-btn modal-btn-secondary";
      btn.textContent = scriptName;
      btn.style.textAlign = "left";
      btn.addEventListener("click", function () {
        document.body.removeChild(overlay);
        onSelect(scriptName);
      });
      listEl.appendChild(btn);
    })(scripts[i]);
  }

  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  cancelBtn.style.width = "100%";
  cancelBtn.addEventListener("click", function () {
    document.body.removeChild(overlay);
  });

  modal.appendChild(titleEl);
  modal.appendChild(listEl);
  modal.appendChild(cancelBtn);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);
}
