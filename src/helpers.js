/**
 * Parse a case ID from user input.
 * Accepts: numeric ID, or full/partial AAO URL containing trial_id=N or id_proces=N.
 */
export function parseCaseId(input) {
  var trimmed = input.trim();
  var num = parseInt(trimmed, 10);
  if (!isNaN(num) && num > 0 && String(num) === trimmed) {
    return num;
  }
  var match = trimmed.match(/(?:trial_id|id_proces)=(\d+)/);
  if (match) {
    return parseInt(match[1], 10);
  }
  return null;
}

export function formatBytes(bytes) {
  if (bytes === 0) return "0 B";
  var units = ["B", "KB", "MB", "GB"];
  var i = 0;
  var b = bytes;
  while (b >= 1024 && i < units.length - 1) {
    b /= 1024;
    i++;
  }
  return b.toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

export function formatDuration(ms) {
  var secs = Math.round(ms / 1000);
  if (secs < 60) return secs + "s";
  var mins = Math.floor(secs / 60);
  var remainSecs = secs % 60;
  if (mins < 60) return mins + "m " + remainSecs + "s";
  var hrs = Math.floor(mins / 60);
  var remainMins = mins % 60;
  return hrs + "h " + remainMins + "m";
}

export function formatDate(isoStr) {
  if (!isoStr) return "";
  var months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  var parts = isoStr.split("T")[0].split("-");
  if (parts.length !== 3) return isoStr.split("T")[0];
  var y = parts[0];
  var m = parseInt(parts[1], 10) - 1;
  var d = parseInt(parts[2], 10);
  return months[m] + " " + d + ", " + y;
}

export function escapeHtml(text) {
  if (!text) return "";
  var div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

export function base64DecodeUtf8(str) {
  var raw = atob(str);
  var bytes = new Uint8Array(raw.length);
  for (var i = 0; i < raw.length; i++) {
    bytes[i] = raw.charCodeAt(i);
  }
  return new TextDecoder().decode(bytes);
}

export function createModal(titleHtml, options) {
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog" + ((options && options.wide) ? " modal-dialog-wide" : "");
  var titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.innerHTML = titleHtml;
  var content = document.createElement("div");
  function close() { if (overlay.parentNode) document.body.removeChild(overlay); }
  overlay.addEventListener("click", function(e) { if (e.target === overlay) close(); });
  modal.appendChild(titleEl);
  modal.appendChild(content);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);
  return { overlay: overlay, modal: modal, content: content, close: close, titleEl: titleEl };
}

// --- Modal Helpers ---

export function showFailedAssetsModal(failedAssets) {
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog modal-dialog-wide";
  var titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.innerHTML = "<strong>" + failedAssets.length + " failed asset(s)</strong>";
  var list = document.createElement("div");
  list.className = "plugin-list";
  for (var i = 0; i < failedAssets.length; i++) {
    var item = document.createElement("div");
    item.className = "plugin-list-item";
    var nameSpan = document.createElement("span");
    nameSpan.className = "plugin-name";
    nameSpan.textContent = failedAssets[i].url || "unknown";
    nameSpan.style.fontSize = "0.75rem";
    nameSpan.style.wordBreak = "break-all";
    var errSpan = document.createElement("span");
    errSpan.style.color = "#a66";
    errSpan.style.fontSize = "0.75rem";
    errSpan.style.flexShrink = "0";
    errSpan.textContent = failedAssets[i].error || "";
    item.appendChild(nameSpan);
    item.appendChild(errSpan);
    list.appendChild(item);
  }
  var closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";
  closeBtn.style.marginTop = "0.75rem";
  function close() { document.body.removeChild(overlay); }
  closeBtn.addEventListener("click", close);
  overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });
  modal.appendChild(titleEl);
  modal.appendChild(list);
  modal.appendChild(closeBtn);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);
}

export function showUpdateModal(message, btn1Label, btn2Label, callback) {
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog";
  var msg = document.createElement("p");
  msg.className = "modal-message";
  msg.textContent = message;
  var buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  var btn1 = document.createElement("button");
  btn1.className = "modal-btn modal-btn-primary";
  btn1.textContent = btn1Label;
  var btn2 = document.createElement("button");
  btn2.className = "modal-btn modal-btn-secondary";
  btn2.textContent = btn2Label;
  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  function close() { document.body.removeChild(overlay); }
  btn1.addEventListener("click", function () { close(); callback(1); });
  btn2.addEventListener("click", function () { close(); callback(2); });
  cancelBtn.addEventListener("click", close);
  overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });
  buttons.appendChild(btn1);
  buttons.appendChild(btn2);
  buttons.appendChild(cancelBtn);
  modal.appendChild(msg);
  modal.appendChild(buttons);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);
}

export function showConfirmModal(message, confirmLabel, onConfirm, onCancel) {
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog";
  var msg = document.createElement("p");
  msg.className = "modal-message";
  msg.textContent = message;
  var buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  var yesBtn = document.createElement("button");
  yesBtn.className = "modal-btn modal-btn-primary";
  yesBtn.textContent = confirmLabel || "OK";
  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  function close() { document.body.removeChild(overlay); }
  yesBtn.addEventListener("click", function () { close(); if (onConfirm) onConfirm(); });
  cancelBtn.addEventListener("click", function () { close(); if (onCancel) onCancel(); });
  overlay.addEventListener("click", function (e) {
    if (e.target === overlay) { close(); if (onCancel) onCancel(); }
  });
  buttons.appendChild(yesBtn);
  buttons.appendChild(cancelBtn);
  modal.appendChild(msg);
  modal.appendChild(buttons);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);
}

export function showPromptModal(message, inputLabel, defaultValue, confirmLabel, onConfirm) {
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog";
  var msg = document.createElement("p");
  msg.className = "modal-message";
  msg.textContent = message;
  var field = document.createElement("div");
  field.className = "modal-field";
  var label = document.createElement("label");
  label.textContent = inputLabel;
  var input = document.createElement("input");
  input.type = "text";
  input.value = defaultValue || "";
  input.placeholder = inputLabel;
  field.appendChild(label);
  field.appendChild(input);
  var buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  var okBtn = document.createElement("button");
  okBtn.className = "modal-btn modal-btn-primary";
  okBtn.textContent = confirmLabel || "OK";
  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  function close() { document.body.removeChild(overlay); }
  okBtn.addEventListener("click", function () {
    var val = input.value.trim();
    if (!val) { input.style.borderColor = "#a33"; input.focus(); return; }
    close();
    onConfirm(val);
  });
  input.addEventListener("keydown", function (e) {
    if (e.key === "Enter") okBtn.click();
  });
  cancelBtn.addEventListener("click", close);
  overlay.addEventListener("click", function (e) {
    if (e.target === overlay) close();
  });
  buttons.appendChild(okBtn);
  buttons.appendChild(cancelBtn);
  modal.appendChild(msg);
  modal.appendChild(field);
  modal.appendChild(buttons);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);
  input.focus();
  input.select();
}
