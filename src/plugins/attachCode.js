import { escapeHtml, createModal } from '../helpers.js';

/**
 * Attach Code Modal — lets user paste JS code and attach it as a case-level plugin.
 */
export function showAttachCodeModal(ctx, caseId, caseTitle, onDone) {
  const invoke = ctx.invoke;
  const statusMsg = ctx.statusMsg;

  const m = createModal("<strong>Attach Plugin Code &mdash; " + escapeHtml(caseTitle) + "</strong>", { wide: true });

  const filenameField = document.createElement("div");
  filenameField.className = "modal-field";
  const filenameLabel = document.createElement("label");
  filenameLabel.textContent = "Filename";
  const filenameInput = document.createElement("input");
  filenameInput.type = "text";
  filenameInput.placeholder = "my_plugin.js";
  filenameField.appendChild(filenameLabel);
  filenameField.appendChild(filenameInput);

  const codeField = document.createElement("div");
  codeField.className = "modal-field";
  const codeLabel = document.createElement("label");
  codeLabel.textContent = "Plugin Code";
  const codeInput = document.createElement("textarea");
  codeInput.className = "attach-code-textarea";
  codeInput.placeholder = "// Paste your plugin JS code here...";
  codeField.appendChild(codeLabel);
  codeField.appendChild(codeInput);

  // Auto-detect plugin name from pasted code
  let userEditedFilename = false;
  filenameInput.addEventListener("input", function () {
    userEditedFilename = true;
  });

  function detectPluginName() {
    const code = codeInput.value;
    const nameMatch = code.match(/EnginePlugins\.register\s*\(\s*\{[^}]*name\s*:\s*['"]([^'"]+)['"]/);
    if (nameMatch) {
      const detected = nameMatch[1] + ".js";
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

  const buttons = document.createElement("div");
  buttons.className = "modal-row-buttons";

  const attachBtn = document.createElement("button");
  attachBtn.className = "modal-btn modal-btn-secondary";
  attachBtn.textContent = "Attach";

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "modal-btn modal-btn-cancel";
  cancelBtn.textContent = "Cancel";

  attachBtn.addEventListener("click", function () {
    let filename = filenameInput.value.trim();
    if (!filename && filenameInput.placeholder && filenameInput.placeholder !== "my_plugin.js") {
      filename = filenameInput.placeholder;
    }
    const code = codeInput.value;

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

    m.close();
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

  cancelBtn.addEventListener("click", m.close);

  buttons.appendChild(attachBtn);
  buttons.appendChild(cancelBtn);

  m.content.appendChild(filenameField);
  m.content.appendChild(codeField);
  m.modal.appendChild(buttons);

  filenameInput.focus();
}
