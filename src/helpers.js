/**
 * Parse a case ID from user input.
 * Accepts: numeric ID, or full/partial AAO URL containing trial_id=N or id_proces=N.
 * @param {string} input
 * @returns {number|null}
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

/** @param {number} bytes @returns {string} */
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

/** @param {number} ms @returns {string} */
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

/** @param {string} isoStr @returns {string} */
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

/** @param {string} text @returns {string} */
export function escapeHtml(text) {
  if (!text) return "";
  var div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

/** @param {string} str @returns {string} */
export function base64DecodeUtf8(str) {
  var raw = atob(str);
  var bytes = new Uint8Array(raw.length);
  for (var i = 0; i < raw.length; i++) {
    bytes[i] = raw.charCodeAt(i);
  }
  return new TextDecoder().decode(bytes);
}

/**
 * @param {string} titleHtml
 * @param {{ body?: HTMLElement|string, footer?: HTMLElement|string, className?: string, onClose?: function }} [options]
 * @returns {{ overlay: HTMLElement, modal: HTMLElement, close: function }}
 */
export function createModal(titleHtml, options) {
  var opts = options || {};
  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  var modal = document.createElement("div");
  modal.className = "modal-dialog" + (opts.wide ? " modal-dialog-wide" : "");
  var titleId = "modal-title-" + Date.now();
  modal.setAttribute("role", "dialog");
  modal.setAttribute("aria-modal", "true");
  modal.setAttribute("aria-labelledby", titleId);
  var titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.id = titleId;
  titleEl.innerHTML = titleHtml;
  var content = document.createElement("div");

  function close() { if (overlay.parentNode) document.body.removeChild(overlay); }
  overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });
  document.addEventListener("keydown", function onEsc(e) {
    if (e.key === "Escape") { document.removeEventListener("keydown", onEsc); close(); }
  });

  modal.appendChild(titleEl);
  modal.appendChild(content);

  // Optional buttons: [{ text, className, onClick }]
  if (opts.buttons && opts.buttons.length > 0) {
    var btnRow = document.createElement("div");
    btnRow.className = "modal-buttons";
    for (var i = 0; i < opts.buttons.length; i++) {
      (function (cfg) {
        var btn = document.createElement("button");
        btn.className = cfg.className || "modal-btn";
        btn.textContent = cfg.text;
        btn.addEventListener("click", function () { close(); if (cfg.onClick) cfg.onClick(); });
        btnRow.appendChild(btn);
      })(opts.buttons[i]);
    }
    modal.appendChild(btnRow);
  }

  // Focus trap: Tab cycles within the modal
  modal.addEventListener("keydown", function (e) {
    if (e.key !== "Tab") return;
    var focusable = modal.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;
    var first = focusable[0];
    var last = focusable[focusable.length - 1];
    if (e.shiftKey) {
      if (document.activeElement === first) { e.preventDefault(); last.focus(); }
    } else {
      if (document.activeElement === last) { e.preventDefault(); first.focus(); }
    }
  });

  overlay.appendChild(modal);
  document.body.appendChild(overlay);

  // Auto-focus first focusable element
  var initialFocus = modal.querySelector(
    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
  );
  if (initialFocus) initialFocus.focus();

  return { overlay: overlay, modal: modal, content: content, close: close, titleEl: titleEl };
}

// --- Modal Helpers ---

/** @param {Array<{url: string, error: string}>} failedAssets */
export function showFailedAssetsModal(failedAssets) {
  var m = createModal("<strong>" + failedAssets.length + " failed asset(s)</strong>", { wide: true });
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
  m.content.appendChild(list);
  var closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";
  closeBtn.style.marginTop = "0.75rem";
  closeBtn.addEventListener("click", m.close);
  m.modal.appendChild(closeBtn);
}

/** @param {string} message @param {string} btn1Label @param {string} btn2Label @param {function(string): void} callback */
export function showUpdateModal(message, btn1Label, btn2Label, callback) {
  var m = createModal(message, {
    buttons: [
      { text: btn1Label, className: "modal-btn modal-btn-primary", onClick: function () { callback(1); } },
      { text: btn2Label, className: "modal-btn modal-btn-secondary", onClick: function () { callback(2); } },
      { text: "Cancel", className: "modal-btn modal-btn-cancel" }
    ]
  });
}

/** @param {string} message @param {string} confirmLabel @param {function(): void} onConfirm @param {function(): void} [onCancel] */
export function showConfirmModal(message, confirmLabel, onConfirm, onCancel) {
  var done = false;
  var m = createModal(message);
  var buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  var yesBtn = document.createElement("button");
  yesBtn.className = "modal-btn modal-btn-primary";
  yesBtn.textContent = confirmLabel || "OK";
  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  yesBtn.addEventListener("click", function () { done = true; m.close(); if (onConfirm) onConfirm(); });
  cancelBtn.addEventListener("click", function () { done = true; m.close(); if (onCancel) onCancel(); });
  // Dismiss (click-outside / Escape) also triggers onCancel
  var origClose = m.close;
  m.close = function () { origClose(); if (!done && onCancel) onCancel(); };
  buttons.appendChild(yesBtn);
  buttons.appendChild(cancelBtn);
  m.modal.appendChild(buttons);
}

/** @param {string} message @param {string} inputLabel @param {string} defaultValue @param {string} confirmLabel @param {function(string): void} onConfirm */
export function showPromptModal(message, inputLabel, defaultValue, confirmLabel, onConfirm) {
  var m = createModal(message);
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
  m.content.appendChild(field);
  var buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  var okBtn = document.createElement("button");
  okBtn.className = "modal-btn modal-btn-primary";
  okBtn.textContent = confirmLabel || "OK";
  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  okBtn.addEventListener("click", function () {
    var val = input.value.trim();
    if (!val) { input.style.borderColor = "#a33"; input.focus(); return; }
    m.close();
    onConfirm(val);
  });
  input.addEventListener("keydown", function (e) {
    if (e.key === "Enter") okBtn.click();
  });
  cancelBtn.addEventListener("click", m.close);
  buttons.appendChild(okBtn);
  buttons.appendChild(cancelBtn);
  m.modal.appendChild(buttons);
  input.focus();
  input.select();
}
