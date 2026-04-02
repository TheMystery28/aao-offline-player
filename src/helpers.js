/**
 * Parse a case ID from user input.
 * Accepts: numeric ID, or full/partial AAO URL containing trial_id=N or id_proces=N.
 * @param {string} input
 * @returns {number|null}
 */
export function parseCaseId(input) {
  const trimmed = input.trim();
  const num = parseInt(trimmed, 10);
  if (!isNaN(num) && num > 0 && String(num) === trimmed) {
    return num;
  }
  const match = trimmed.match(/(?:trial_id|id_proces)=(\d+)/);
  if (match) {
    return parseInt(match[1], 10);
  }
  return null;
}

/** @param {number} bytes @returns {string} */
export function formatBytes(bytes) {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let b = bytes;
  while (b >= 1024 && i < units.length - 1) {
    b /= 1024;
    i++;
  }
  return b.toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

/** @param {number} ms @returns {string} */
export function formatDuration(ms) {
  const secs = Math.round(ms / 1000);
  if (secs < 60) return secs + "s";
  const mins = Math.floor(secs / 60);
  const remainSecs = secs % 60;
  if (mins < 60) return mins + "m " + remainSecs + "s";
  const hrs = Math.floor(mins / 60);
  const remainMins = mins % 60;
  return hrs + "h " + remainMins + "m";
}

/** @param {string} isoStr @returns {string} */
export function formatDate(isoStr) {
  if (!isoStr) return "";
  const months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  const parts = isoStr.split("T")[0].split("-");
  if (parts.length !== 3) return isoStr.split("T")[0];
  const y = parts[0];
  const m = parseInt(parts[1], 10) - 1;
  const d = parseInt(parts[2], 10);
  return months[m] + " " + d + ", " + y;
}

/** @param {string} text @returns {string} */
export function escapeHtml(text) {
  if (!text) return "";
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

/** @param {string} str @returns {string} */
export function base64DecodeUtf8(str) {
  const raw = atob(str);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) {
    bytes[i] = raw.charCodeAt(i);
  }
  return new TextDecoder().decode(bytes);
}

/**
 * @param {string} titleHtml
 * @param {{ body?: HTMLElement|string, footer?: HTMLElement|string, className?: string, onClose?: function }} [options]
 * @returns {{ overlay: HTMLElement, modal: HTMLElement, close: function }}
 */
/** Group cases into sequence groups + standalone. Eliminates duplicated grouping logic. */
export function groupCasesBySequence(cases) {
  const sequenceGroups = {};
  const standalone = [];
  for (let i = 0; i < cases.length; i++) {
    const c = cases[i];
    const seq = c.sequence;
    if (seq && seq.title && seq.list && seq.list.length > 1) {
      if (!sequenceGroups[seq.title]) {
        sequenceGroups[seq.title] = { list: seq.list, cases: [] };
      }
      sequenceGroups[seq.title].cases.push(c);
    } else {
      standalone.push(c);
    }
  }
  return { sequenceGroups: sequenceGroups, standalone: standalone };
}

/** Apply spoiler blur to an element if the setting is checked. */
export function applySpoilerBlur(el) {
  const blurEl = document.getElementById("settings-blur-spoilers");
  if (blurEl && blurEl.checked) {
    el.classList.add("spoiler-blur");
  } else {
    el.classList.remove("spoiler-blur");
  }
}

/** Remove spoiler blur from an element. */
export function removeSpoilerBlur(el) {
  el.classList.remove("spoiler-blur");
}

export function createModal(titleHtml, options) {
  const opts = options || {};
  const overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  const modal = document.createElement("div");
  modal.className = "modal-dialog" + (opts.wide ? " modal-dialog-wide" : "");
  const titleId = "modal-title-" + Date.now();
  modal.setAttribute("role", "dialog");
  modal.setAttribute("aria-modal", "true");
  modal.setAttribute("aria-labelledby", titleId);
  const titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.id = titleId;
  titleEl.innerHTML = titleHtml;
  const content = document.createElement("div");

  function close() { if (overlay.parentNode) document.body.removeChild(overlay); }
  overlay.addEventListener("click", function (e) { if (e.target === overlay) close(); });
  document.addEventListener("keydown", function onEsc(e) {
    if (e.key === "Escape") { document.removeEventListener("keydown", onEsc); close(); }
  });

  modal.appendChild(titleEl);
  modal.appendChild(content);

  // Optional buttons: [{ text, className, onClick }]
  if (opts.buttons && opts.buttons.length > 0) {
    const btnRow = document.createElement("div");
    btnRow.className = "modal-buttons";
    for (let i = 0; i < opts.buttons.length; i++) {
      (function (cfg) {
        const btn = document.createElement("button");
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
    const focusable = modal.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey) {
      if (document.activeElement === first) { e.preventDefault(); last.focus(); }
    } else {
      if (document.activeElement === last) { e.preventDefault(); first.focus(); }
    }
  });

  overlay.appendChild(modal);
  document.body.appendChild(overlay);

  // Auto-focus first focusable element
  const initialFocus = modal.querySelector(
    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
  );
  if (initialFocus) initialFocus.focus();

  return { overlay: overlay, modal: modal, content: content, close: close, titleEl: titleEl };
}

// --- Modal Helpers ---

/** @param {Array<{url: string, error: string}>} failedAssets */
export function showFailedAssetsModal(failedAssets) {
  const m = createModal("<strong>" + failedAssets.length + " failed asset(s)</strong>", { wide: true });
  const list = document.createElement("div");
  list.className = "plugin-list";
  for (let i = 0; i < failedAssets.length; i++) {
    const item = document.createElement("div");
    item.className = "plugin-list-item";
    const nameSpan = document.createElement("span");
    nameSpan.className = "plugin-name";
    nameSpan.textContent = failedAssets[i].url || "unknown";
    nameSpan.style.fontSize = "0.75rem";
    nameSpan.style.wordBreak = "break-all";
    const errSpan = document.createElement("span");
    errSpan.style.color = "#a66";
    errSpan.style.fontSize = "0.75rem";
    errSpan.style.flexShrink = "0";
    errSpan.textContent = failedAssets[i].error || "";
    item.appendChild(nameSpan);
    item.appendChild(errSpan);
    list.appendChild(item);
  }
  m.content.appendChild(list);
  const closeBtn = document.createElement("button");
  closeBtn.className = "modal-btn modal-btn-cancel";
  closeBtn.textContent = "Close";
  closeBtn.style.width = "100%";
  closeBtn.style.marginTop = "0.75rem";
  closeBtn.addEventListener("click", m.close);
  m.modal.appendChild(closeBtn);
}

/** @param {string} message @param {string} btn1Label @param {string} btn2Label @param {function(string): void} callback */
export function showUpdateModal(message, btn1Label, btn2Label, callback) {
  const m = createModal(message, {
    buttons: [
      { text: btn1Label, className: "modal-btn modal-btn-primary", onClick: function () { callback(1); } },
      { text: btn2Label, className: "modal-btn modal-btn-secondary", onClick: function () { callback(2); } },
      { text: "Cancel", className: "modal-btn modal-btn-cancel" }
    ]
  });
}

/** @param {string} message @param {string} confirmLabel @param {function(): void} onConfirm @param {function(): void} [onCancel] */
export function showConfirmModal(message, confirmLabel, onConfirm, onCancel) {
  let done = false;
  const m = createModal(message);
  const buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  const yesBtn = document.createElement("button");
  yesBtn.className = "modal-btn modal-btn-primary";
  yesBtn.textContent = confirmLabel || "OK";
  const cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  yesBtn.addEventListener("click", function () { done = true; m.close(); if (onConfirm) onConfirm(); });
  cancelBtn.addEventListener("click", function () { done = true; m.close(); if (onCancel) onCancel(); });
  // Dismiss (click-outside / Escape) also triggers onCancel
  const origClose = m.close;
  m.close = function () { origClose(); if (!done && onCancel) onCancel(); };
  buttons.appendChild(yesBtn);
  buttons.appendChild(cancelBtn);
  m.modal.appendChild(buttons);
}

/** @param {string} message @param {string} inputLabel @param {string} defaultValue @param {string} confirmLabel @param {function(string): void} onConfirm */
export function showPromptModal(message, inputLabel, defaultValue, confirmLabel, onConfirm) {
  const m = createModal(message);
  const field = document.createElement("div");
  field.className = "modal-field";
  const label = document.createElement("label");
  label.textContent = inputLabel;
  const input = document.createElement("input");
  input.type = "text";
  input.value = defaultValue || "";
  input.placeholder = inputLabel;
  field.appendChild(label);
  field.appendChild(input);
  m.content.appendChild(field);
  const buttons = document.createElement("div");
  buttons.className = "modal-buttons";
  const okBtn = document.createElement("button");
  okBtn.className = "modal-btn modal-btn-primary";
  okBtn.textContent = confirmLabel || "OK";
  const cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";
  okBtn.addEventListener("click", function () {
    const val = input.value.trim();
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
