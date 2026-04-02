import { createModal } from '../helpers.js';

/**
 * Plugin Picker Modal — lets user select one plugin from a list of script names.
 */
export function showPluginPickerModal(ctx, scripts, onSelect) {
  const m = createModal("<strong>Select Plugin</strong>");

  const listEl = document.createElement("div");
  listEl.style.cssText = "display:flex; flex-direction:column; gap:6px; margin:10px 0;";

  for (let i = 0; i < scripts.length; i++) {
    (function (scriptName) {
      const btn = document.createElement("button");
      btn.className = "modal-btn modal-btn-secondary";
      btn.textContent = scriptName;
      btn.style.textAlign = "left";
      btn.addEventListener("click", function () {
        m.close();
        onSelect(scriptName);
      });
      listEl.appendChild(btn);
    })(scripts[i]);
  }

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  cancelBtn.style.width = "100%";
  cancelBtn.addEventListener("click", m.close);

  m.content.appendChild(listEl);
  m.modal.appendChild(cancelBtn);
}
