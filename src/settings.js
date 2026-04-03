import { formatBytes, showConfirmModal, applySpoilerBlur, removeSpoilerBlur } from './helpers.js';

const THEME_PRESETS = {
  'default': {},
  'gba': {
    '--bg-body': '#1a0033',
    '--bg-input': '#200040',
    '--bg-dark': '#100020',
    '--bg-hover': '#2a0055',
    '--bg-btn-primary': '#4a0080',
    '--text-primary': '#f0e8ff',
    '--text-secondary': '#cc99ff',
    '--text-muted': '#9966cc',
    '--text-dimmed': '#663399',
    '--accent-blue': '#9933ff',
    '--accent-blue-dark': '#6600cc',
    '--accent-purple': '#ff66aa',
    '--border-color': '#3d0066',
    '--border-focus': '#9933ff',
    '--danger-text': '#ff4466'
  },
  'ds': {
    '--bg-body': '#c8d0d8',
    '--bg-input': '#d8e0e8',
    '--bg-dark': '#b0b8c0',
    '--bg-hover': '#d0d8e0',
    '--bg-btn-primary': '#003399',
    '--text-primary': '#101820',
    '--text-secondary': '#304050',
    '--text-hover': '#202830',
    '--text-muted': '#374555',
    '--text-dimmed': '#3d4f5f',
    '--accent-blue': '#0044cc',
    '--accent-blue-dark': '#003399',
    '--accent-purple': '#6600cc',
    '--border-color': '#6a7880',
    '--border-focus': '#0044cc',
    '--danger-text': '#cc2200'
  }
};

function applyLauncherTheme(themeName) {
  const preset = THEME_PRESETS[themeName] || THEME_PRESETS['default'];
  let el = document.getElementById('launcher-theme-style');
  if (!el) {
    el = document.createElement('style');
    el.id = 'launcher-theme-style';
    document.head.appendChild(el);
  }
  const vars = Object.keys(preset);
  el.textContent = vars.length > 0
    ? ':root {\n' + vars.map(function(v) { return '  ' + v + ': ' + preset[v] + ';'; }).join('\n') + '\n}'
    : '';
}

function applyCustomLauncherCSS(css) {
  let el = document.getElementById('launcher-custom-style');
  if (!el) {
    el = document.createElement('style');
    el.id = 'launcher-custom-style';
  }
  // Always move to end of <head> so it comes after launcher-theme-style and wins the cascade.
  document.head.appendChild(el);
  el.textContent = css || '';
}

function formatRelativeTime(unixSecs) {
  if (!unixSecs) return "Never run";
  var diff = Math.floor(Date.now() / 1000) - unixSecs;
  if (diff < 60) return "Just now";
  if (diff < 3600) return Math.floor(diff / 60) + " min ago";
  if (diff < 86400) return Math.floor(diff / 3600) + " hours ago";
  return Math.floor(diff / 86400) + " days ago";
}

/**
 * @param {function(string, Object=): Promise} invoke
 * @param {new(): {onmessage: function}} Channel
 * @param {HTMLElement} statusMsg
 * @returns {{ loadSettings: function, loadStorageInfo: function }}
 */
export function initSettings(invoke, Channel, statusMsg) {
  const settingsToggle = document.getElementById("settings-toggle");
  const settingsPanel = document.getElementById("settings-panel");
  const settingsLanguage = document.getElementById("settings-language");
  const settingsTheme = document.getElementById("settings-theme");
  const settingsCustomCss = document.getElementById("settings-custom-css");
  const settingsConcurrency = document.getElementById("settings-concurrency");
  const concurrencyValue = document.getElementById("concurrency-value");
  const dataDirPath = document.getElementById("data-dir-path");
  const openDataDirBtn = document.getElementById("open-data-dir-btn");
  const storageText = document.getElementById("storage-text");
  const optimizeStorageBtn = document.getElementById("optimize-storage-btn");
  const lastOptimizedText = document.getElementById("last-optimized-text");
  const progressContainer = document.getElementById("progress-container");
  const progressPhase = document.getElementById("progress-phase");
  const progressBarInner = document.getElementById("progress-bar-inner");
  const progressText = document.getElementById("progress-text");

  let settingsSaveTimeout = null;
  let lastOptimizedAt = null;

  function refreshLastOptimizedText() {
    lastOptimizedText.textContent = "Last run: " + formatRelativeTime(lastOptimizedAt);
  }

  settingsToggle.addEventListener("click", function () {
    const isOpen = !settingsPanel.classList.contains("hidden");
    if (isOpen) {
      settingsPanel.classList.add("hidden");
      settingsToggle.classList.remove("open");
    } else {
      settingsPanel.classList.remove("hidden");
      settingsToggle.classList.add("open");
      loadStorageInfo();
      refreshLastOptimizedText();
    }
  });

  const settingsAutoSave = document.getElementById("settings-autosave");
  const settingsBlurSpoilers = document.getElementById("settings-blur-spoilers");

  function loadSettings() {
    invoke("get_settings").then(function (settings) {
      settingsLanguage.value = settings.language;
      settingsConcurrency.value = settings.concurrent_downloads;
      concurrencyValue.textContent = settings.concurrent_downloads;
      if (settingsAutoSave) settingsAutoSave.checked = settings.auto_save;
      if (settingsBlurSpoilers) settingsBlurSpoilers.checked = settings.blur_spoilers;
      if (settingsTheme) settingsTheme.value = settings.theme || "default";
      applyLauncherTheme(settingsTheme ? settingsTheme.value : "default");
      if (settingsCustomCss) settingsCustomCss.value = settings.custom_css || "";
      applyCustomLauncherCSS(settings.custom_css || "");
      lastOptimizedAt = settings.last_optimized_at;
      refreshLastOptimizedText();
    }).catch(function (e) {
      console.error("[SETTINGS] Failed to load settings:", e);
    });
  }

  function saveSettings() {
    const settings = {
      language: settingsLanguage.value,
      concurrent_downloads: parseInt(settingsConcurrency.value, 10),
      auto_save: settingsAutoSave ? settingsAutoSave.checked : true,
      blur_spoilers: settingsBlurSpoilers ? settingsBlurSpoilers.checked : true,
      theme: settingsTheme ? settingsTheme.value : "default",
      custom_css: settingsCustomCss ? settingsCustomCss.value : ""
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
  if (settingsTheme) {
    settingsTheme.addEventListener("change", function () {
      applyLauncherTheme(settingsTheme.value);
      applyCustomLauncherCSS(settingsCustomCss ? settingsCustomCss.value : "");
      debounceSave();
    });
  }

  if (settingsCustomCss) {
    settingsCustomCss.addEventListener("input", function () {
      applyCustomLauncherCSS(settingsCustomCss.value);
      debounceSave();
    });
  }

  settingsConcurrency.addEventListener("input", function () {
    concurrencyValue.textContent = settingsConcurrency.value;
    debounceSave();
  });

  if (settingsAutoSave) settingsAutoSave.addEventListener("change", debounceSave);
  if (settingsBlurSpoilers) settingsBlurSpoilers.addEventListener("change", debounceSave);

  function buildStorageSection(label, size, parts, indent) {
    const cls = indent ? "storage-toggle storage-sub" : "storage-toggle";
    let html = '<button class="' + cls + '" onclick="this.classList.toggle(\'open\');this.nextElementSibling.classList.toggle(\'open\')">';
    html += label + " — " + formatBytes(size) + "</button>";
    html += '<div class="storage-collapse">';
    for (let i = 0; i < parts.length; i++) {
      if (parts[i].length > 2 && parts[i][2]) {
        // Nested collapsible section: [label, size, subParts]
        html += buildStorageSection(parts[i][0], parts[i][1], parts[i][2], true);
      } else {
        const subCls = indent ? "storage-row storage-sub-deep" : "storage-row storage-sub";
        html += '<div class="' + subCls + '"><span>' + parts[i][0] + '</span><span>' + formatBytes(parts[i][1]) + '</span></div>';
      }
    }
    html += "</div>";
    return html;
  }

  function loadStorageInfo() {
    invoke("get_storage_info").then(function (info) {
      dataDirPath.textContent = info.data_dir;
      const storageEl = document.getElementById("storage-info");

      let html = '<div class="storage-details">';
      html += '<div class="storage-row storage-total"><span>Total</span><span>' + formatBytes(info.total_size_bytes) + '</span></div>';

      // Cases — collapsible
      const caseParts = [];
      if (info.cases_assets_bytes > 0) caseParts.push(["Assets", info.cases_assets_bytes]);
      if (info.cases_metadata_bytes > 0) caseParts.push(["Metadata", info.cases_metadata_bytes]);
      if (info.cases_plugins_bytes > 0) caseParts.push(["Plugins", info.cases_plugins_bytes]);
      const casesLabel = info.cases_count + " case" + (info.cases_count !== 1 ? "s" : "");
      html += buildStorageSection(casesLabel, info.cases_size_bytes, caseParts, false);

      // Default assets — collapsible
      if (info.defaults_size_bytes > 0) {
        const defaultParts = [];
        if (info.defaults_sprites_bytes > 0) defaultParts.push(["Sprites", info.defaults_sprites_bytes]);
        if (info.defaults_music_bytes > 0) defaultParts.push(["Music", info.defaults_music_bytes]);
        if (info.defaults_sounds_bytes > 0) defaultParts.push(["Sounds", info.defaults_sounds_bytes]);
        if (info.defaults_voices_bytes > 0) defaultParts.push(["Voices", info.defaults_voices_bytes]);
        if (info.defaults_shared_bytes > 0) {
          const sharedSub = [];
          const sharedLabel = info.defaults_shared_count + " file" + (info.defaults_shared_count !== 1 ? "s" : "");
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
      const storageEl = document.getElementById("storage-info");
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

    const onEvent = new Channel();
    onEvent.onmessage = function (msg) {
      if (msg.event === "progress") {
        const pct = msg.data.total > 0 ? Math.round((msg.data.completed / msg.data.total) * 100) : 0;
        progressBarInner.style.width = pct + "%";
        progressText.textContent = msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          let fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur(progressText);
        }
      }
    };

    invoke("optimize_storage", { onEvent: onEvent }).then(function (result) {
      optimizeStorageBtn.textContent = "Optimize & Fix";
      optimizeStorageBtn.disabled = false;
      removeSpoilerBlur(progressText);
      progressContainer.classList.add("hidden");
      lastOptimizedAt = result.last_optimized_at;
      refreshLastOptimizedText();
      if (result.deduped > 0) {
        statusMsg.textContent = "Optimized: " + result.deduped + " files deduplicated, " + formatBytes(result.bytes_saved) + " saved.";
      } else {
        statusMsg.textContent = "Storage is already optimized. No duplicates found.";
      }
      loadStorageInfo();
    }).catch(function (e) {
      optimizeStorageBtn.textContent = "Optimize & Fix";
      optimizeStorageBtn.disabled = false;
      progressContainer.classList.add("hidden");
      console.error("[SETTINGS] Failed to optimize storage:", e);
      statusMsg.textContent = "Error optimizing storage: " + e;
    });
  });

  function getTheme() {
    return settingsTheme ? settingsTheme.value : "default";
  }

  return {
    loadSettings: loadSettings,
    loadStorageInfo: loadStorageInfo,
    getTheme: getTheme
  };
}
