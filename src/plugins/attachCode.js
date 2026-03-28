import { escapeHtml } from '../helpers.js';

/**
 * Attach Code Modal — lets user paste JS code and attach it as a case-level plugin.
 */
export function showAttachCodeModal(ctx, caseId, caseTitle, onDone) {
  var invoke = ctx.invoke;
  var statusMsg = ctx.statusMsg;

  var overlay = document.createElement("div");
  overlay.className = "modal-overlay";

  var modal = document.createElement("div");
  modal.className = "modal-dialog modal-dialog-wide";

  var titleEl = document.createElement("div");
  titleEl.className = "modal-message";
  titleEl.innerHTML = "<strong>Attach Plugin Code &mdash; " + escapeHtml(caseTitle) + "</strong>";

  var filenameField = document.createElement("div");
  filenameField.className = "modal-field";
  var filenameLabel = document.createElement("label");
  filenameLabel.textContent = "Filename";
  var filenameInput = document.createElement("input");
  filenameInput.type = "text";
  filenameInput.placeholder = "my_plugin.js";
  filenameField.appendChild(filenameLabel);
  filenameField.appendChild(filenameInput);

  var codeField = document.createElement("div");
  codeField.className = "modal-field";
  var codeLabel = document.createElement("label");
  codeLabel.textContent = "Plugin Code";
  var codeInput = document.createElement("textarea");
  codeInput.className = "attach-code-textarea";
  codeInput.placeholder = "// Paste your plugin JS code here...";
  codeField.appendChild(codeLabel);
  codeField.appendChild(codeInput);

  // Auto-detect plugin name from pasted code
  var userEditedFilename = false;
  filenameInput.addEventListener("input", function () {
    userEditedFilename = true;
  });

  function detectPluginName() {
    var code = codeInput.value;
    var nameMatch = code.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
    if (nameMatch) {
      var detected = nameMatch[1] + ".js";
      filenameInput.placeholder = detected;
      if (!userEditedFilename) {
        filenameInput.value = detected;
      }
    }
  }

  codeInput.addEventListener("input", detectPluginName);
  codeInput.addEventListener("paste", function () {
    setTimeout(detectPluginName, 0);
  });

  var buttons = document.createElement("div");
  buttons.className = "modal-row-buttons";

  var attachBtn = document.createElement("button");
  attachBtn.className = "modal-btn modal-btn-secondary";
  attachBtn.textContent = "Attach";

  var cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";

  function close() {
    document.body.removeChild(overlay);
  }

  attachBtn.addEventListener("click", function () {
    var filename = filenameInput.value.trim();
    if (!filename && filenameInput.placeholder && filenameInput.placeholder !== "my_plugin.js") {
      filename = filenameInput.placeholder;
    }
    var code = codeInput.value;

    if (!filename) {
      filenameInput.style.borderColor = "#a33";
      filenameInput.focus();
      return;
    }
    if (!filename.toLowerCase().endsWith(".js")) {
      filename = filename + ".js";
    }
    if (!code) {
      codeInput.style.borderColor = "#a33";
      codeInput.focus();
      return;
    }

    close();
    statusMsg.textContent = "Attaching plugin...";
    invoke("attach_plugin_code", {
      code: code,
      filename: filename,
      targetCaseIds: [caseId]
    })
    .then(function () {
      statusMsg.textContent = "Plugin \"" + filename + "\" attached.";
      if (onDone) onDone();
    })
    .catch(function (e) {
      statusMsg.textContent = "Error attaching plugin: " + e;
    });
  });

  cancelBtn.addEventListener("click", close);
  overlay.addEventListener("click", function (e) {
    if (e.target === overlay) close();
  });

  buttons.appendChild(attachBtn);
  buttons.appendChild(cancelBtn);

  modal.appendChild(titleEl);
  modal.appendChild(filenameField);
  modal.appendChild(codeField);
  modal.appendChild(buttons);
  overlay.appendChild(modal);
  document.body.appendChild(overlay);

  filenameInput.focus();
}
