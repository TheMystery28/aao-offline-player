import { formatBytes, showConfirmModal } from './helpers.js';

/**
 * @param {function(string, Object=): Promise} invoke
 * @param {new(): {onmessage: function}} Channel
 * @param {HTMLElement} statusMsg
 * @returns {{ loadSettings: function, loadStorageInfo: function }}
 */
export function initSettings(invoke, Channel, statusMsg) {
  var settingsToggle = document.getElementById("settings-toggle");
  var settingsPanel = document.getElementById("settings-panel");
  var settingsLanguage = document.getElementById("settings-language");
  var settingsConcurrency = document.getElementById("settings-concurrency");
  var concurrencyValue = document.getElementById("concurrency-value");
  var dataDirPath = document.getElementById("data-dir-path");
  var openDataDirBtn = document.getElementById("open-data-dir-btn");
  var storageText = document.getElementById("storage-text");
  var optimizeStorageBtn = document.getElementById("optimize-storage-btn");
  var clearUnusedBtn = document.getElementById("clear-unused-defaults-btn");
  var progressContainer = document.getElementById("progress-container");
  var progressPhase = document.getElementById("progress-phase");
  var progressBarInner = document.getElementById("progress-bar-inner");
  var progressText = document.getElementById("progress-text");

  var settingsSaveTimeout = null;

  function applySpoilerBlur() {
    var blurEl = document.getElementById("settings-blur-spoilers");
    if (blurEl && blurEl.checked) {
      progressText.classList.add("spoiler-blur");
    } else {
      progressText.classList.remove("spoiler-blur");
    }
  }
  function removeSpoilerBlur() {
    progressText.classList.remove("spoiler-blur");
  }

  settingsToggle.addEventListener("click", function () {
    var isOpen = !settingsPanel.classList.contains("hidden");
    if (isOpen) {
      settingsPanel.classList.add("hidden");
      settingsToggle.classList.remove("open");
    } else {
      settingsPanel.classList.remove("hidden");
      settingsToggle.classList.add("open");
      loadStorageInfo();
    }
  });

  var settingsAutoSave = document.getElementById("settings-autosave");
  var settingsBlurSpoilers = document.getElementById("settings-blur-spoilers");

  function loadSettings() {
    invoke("get_settings").then(function (settings) {
      settingsLanguage.value = settings.language;
      settingsConcurrency.value = settings.concurrent_downloads;
      concurrencyValue.textContent = settings.concurrent_downloads;
      if (settingsAutoSave) settingsAutoSave.checked = settings.auto_save;
      if (settingsBlurSpoilers) settingsBlurSpoilers.checked = settings.blur_spoilers;
    }).catch(function (e) {
      console.error("[SETTINGS] Failed to load settings:", e);
    });
  }

  function saveSettings() {
    var settings = {
      language: settingsLanguage.value,
      concurrent_downloads: parseInt(settingsConcurrency.value, 10),
      auto_save: settingsAutoSave ? settingsAutoSave.checked : true,
      blur_spoilers: settingsBlurSpoilers ? settingsBlurSpoilers.checked : true
    };
    invoke("save_settings", { settings: settings }).catch(function (e) {
      console.error("[SETTINGS] Failed to save settings:", e);
    });
  }

  function debounceSave() {
    if (settingsSaveTimeout) clearTimeout(settingsSaveTimeout);
    settingsSaveTimeout = setTimeout(saveSettings, 300);
  }

  settingsLanguage.addEventListener("change", debounceSave);

  settingsConcurrency.addEventListener("input", function () {
    concurrencyValue.textContent = settingsConcurrency.value;
    debounceSave();
  });

  if (settingsAutoSave) settingsAutoSave.addEventListener("change", debounceSave);
  if (settingsBlurSpoilers) settingsBlurSpoilers.addEventListener("change", debounceSave);

  function buildStorageSection(label, size, parts, indent) {
    var cls = indent ? "storage-toggle storage-sub" : "storage-toggle";
    var html = '<button class="' + cls + '" onclick="this.classList.toggle(\'open\');this.nextElementSibling.classList.toggle(\'open\')">';
    html += label + " — " + formatBytes(size) + "</button>";
    html += '<div class="storage-collapse">';
    for (var i = 0; i < parts.length; i++) {
      if (parts[i].length > 2 && parts[i][2]) {
        // Nested collapsible section: [label, size, subParts]
        html += buildStorageSection(parts[i][0], parts[i][1], parts[i][2], true);
      } else {
        var subCls = indent ? "storage-row storage-sub-deep" : "storage-row storage-sub";
        html += '<div class="' + subCls + '"><span>' + parts[i][0] + '</span><span>' + formatBytes(parts[i][1]) + '</span></div>';
      }
    }
    html += "</div>";
    return html;
  }

  function loadStorageInfo() {
    invoke("get_storage_info").then(function (info) {
      dataDirPath.textContent = info.data_dir;
      var storageEl = document.getElementById("storage-info");

      var html = '<div class="storage-details">';
      html += '<div class="storage-row storage-total"><span>Total</span><span>' + formatBytes(info.total_size_bytes) + '</span></div>';

      // Cases — collapsible
      var caseParts = [];
      if (info.cases_assets_bytes > 0) caseParts.push(["Assets", info.cases_assets_bytes]);
      if (info.cases_metadata_bytes > 0) caseParts.push(["Metadata", info.cases_metadata_bytes]);
      if (info.cases_plugins_bytes > 0) caseParts.push(["Plugins", info.cases_plugins_bytes]);
      var casesLabel = info.cases_count + " case" + (info.cases_count !== 1 ? "s" : "");
      html += buildStorageSection(casesLabel, info.cases_size_bytes, caseParts, false);

      // Default assets — collapsible
      if (info.defaults_size_bytes > 0) {
        var defaultParts = [];
        if (info.defaults_sprites_bytes > 0) defaultParts.push(["Sprites", info.defaults_sprites_bytes]);
        if (info.defaults_music_bytes > 0) defaultParts.push(["Music", info.defaults_music_bytes]);
        if (info.defaults_sounds_bytes > 0) defaultParts.push(["Sounds", info.defaults_sounds_bytes]);
        if (info.defaults_voices_bytes > 0) defaultParts.push(["Voices", info.defaults_voices_bytes]);
        if (info.defaults_shared_bytes > 0) {
          var sharedSub = [];
          var sharedLabel = info.defaults_shared_count + " file" + (info.defaults_shared_count !== 1 ? "s" : "");
          if (info.defaults_shared_images_bytes > 0) sharedSub.push(["Images", info.defaults_shared_images_bytes]);
          if (info.defaults_shared_audio_bytes > 0) sharedSub.push(["Audio", info.defaults_shared_audio_bytes]);
          if (info.defaults_shared_other_bytes > 0) sharedSub.push(["Other", info.defaults_shared_other_bytes]);
          if (sharedSub.length > 0) {
            defaultParts.push(["Shared — " + sharedLabel, info.defaults_shared_bytes, sharedSub]);
          } else {
            defaultParts.push(["Shared — " + sharedLabel, info.defaults_shared_bytes]);
          }
        }
        if (info.defaults_other_bytes > 0) defaultParts.push(["Other", info.defaults_other_bytes]);
        html += buildStorageSection("Default assets", info.defaults_size_bytes, defaultParts, false);
      }

      html += "</div>";
      storageEl.innerHTML = html;
      storageEl.className = "";
    }).catch(function (e) {
      console.error("[SETTINGS] Failed to load storage info:", e);
      var storageEl = document.getElementById("storage-info");
      storageEl.textContent = "Unable to compute storage info.";
    });
  }

  openDataDirBtn.addEventListener("click", function () {
    invoke("open_data_dir").catch(function (e) {
      console.error("[SETTINGS] Failed to open data dir:", e);
    });
  });

  optimizeStorageBtn.addEventListener("click", function () {
    optimizeStorageBtn.disabled = true;
    optimizeStorageBtn.textContent = "Optimizing...";
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Optimizing storage...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "Scanning cases...";

    var onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      if (msg.event === "progress") {
        var pct = msg.data.total > 0 ? Math.round((msg.data.completed / msg.data.total) * 100) : 0;
        progressBarInner.style.width = pct + "%";
        progressText.textContent = msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur();
        }
      }
    };

    invoke("optimize_storage", { onEvent: onEvent }).then(function (result) {
      optimizeStorageBtn.textContent = "Optimize Storage";
      optimizeStorageBtn.disabled = false;
      removeSpoilerBlur();
      progressContainer.classList.add("hidden");
      if (result.deduped > 0) {
        statusMsg.textContent = "Optimized: " + result.deduped + " files deduplicated, " + formatBytes(result.bytes_saved) + " saved.";
      } else {
        statusMsg.textContent = "Storage is already optimized. No duplicates found.";
      }
      loadStorageInfo();
    }).catch(function (e) {
      optimizeStorageBtn.textContent = "Optimize Storage";
      optimizeStorageBtn.disabled = false;
      progressContainer.classList.add("hidden");
      console.error("[SETTINGS] Failed to optimize storage:", e);
      statusMsg.textContent = "Error optimizing storage: " + e;
    });
  });

  clearUnusedBtn.addEventListener("click", function () {
    showConfirmModal(
      "Remove cached default assets not used by any downloaded case?\n\nThis frees disk space safely. The assets will be re-downloaded if needed later.",
      "Clear Unused",
      function () {
        clearUnusedBtn.disabled = true;
        clearUnusedBtn.textContent = "Clearing...";
        invoke("clear_unused_defaults").then(function (result) {
          clearUnusedBtn.textContent = "Clear Unused";
          clearUnusedBtn.disabled = false;
          if (result.deleted > 0) {
            statusMsg.textContent = "Cleared " + result.deleted + " unused files (" + formatBytes(result.bytes_freed) + " freed).";
          } else {
            statusMsg.textContent = "No unused default assets found.";
          }
          loadStorageInfo();
        }).catch(function (e) {
          clearUnusedBtn.textContent = "Clear Unused";
          clearUnusedBtn.disabled = false;
          console.error("[SETTINGS] Failed to clear unused defaults:", e);
          statusMsg.textContent = "Error clearing unused defaults: " + e;
        });
      }
    );
  });

  return {
    loadSettings: loadSettings,
    loadStorageInfo: loadStorageInfo
  };
}
