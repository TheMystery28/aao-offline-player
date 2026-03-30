/**
 * @typedef {Object} AppContext
 * @property {function(string, Object=): Promise} invoke - Tauri invoke for calling Rust commands
 * @property {new(): {onmessage: function}} Channel - Tauri Channel constructor for streaming events
 * @property {HTMLElement} statusMsg - Status message element
 * @property {HTMLElement} caseList - Case list container
 * @property {HTMLElement} emptyLibrary - Empty library placeholder
 * @property {HTMLElement} libraryLoading - Library loading indicator
 * @property {Set<number>} knownCaseIds - Currently loaded case IDs
 * @property {function(): Promise} loadLibrary - Refresh library UI
 * @property {function(number): void} startDownload - Start downloading a case
 * @property {function(number): void} startUpdate - Start updating a case
 * @property {function(number, boolean): void} updateCase - Update case with redownload option
 * @property {function(number): void} retryCase - Retry failed assets
 * @property {function(Array<number>, string): void} startSequenceDownload - Download a sequence
 * @property {function(): boolean} isDownloadInProgress - Check if download is active
 * @property {function(number, string): void} playCase - Open case in player
 * @property {function(number): void} deleteCase - Delete a case
 * @property {function(number): void} exportCase - Export a case
 * @property {function(): void} showLauncher - Return to launcher from player
 * @property {function(Object): Promise} writeGameSaves - Write saves to localStorage
 * @property {function(Object): Promise} readGameSaves - Read saves from localStorage
 * @property {function(): void} showPasteSaveModal - Show paste-save modal
 * @property {function(string): void} doImportSave - Import a save file
 * @property {function(string): void} doImportPlugin - Import a plugin file
 * @property {HTMLElement} progressContainer - Download progress container
 * @property {HTMLElement} progressPhase - Download phase text
 * @property {HTMLElement} progressBarInner - Progress bar fill element
 * @property {HTMLElement} progressText - Progress text element
 */

const { invoke, Channel } = window.__TAURI__.core;
// helpers.js imports are used by the extracted modules (library.js, collections.js), not directly here
import { initSettings } from './settings.js';
import { initImport } from './importSection.js';
import { initPlayer } from './player.js';
import { initDownload } from './download.js';
import { initSaves } from './saves.js';
import { initPlugins } from './plugins/init.js';
import { initLibrary } from './library.js';
import { initCollections } from './collections/init.js';

/**
 * One-time migration of localStorage data from the old http://localhost origin
 * to the new aao:// protocol origin. Opens a hidden iframe to the legacy tiny_http
 * server, reads game_saves and engine config, writes them to the current origin.
 * Completes silently if no old data exists or if migration was already done.
 */
function migrateLocalStorage() {
  return invoke("get_settings").then(function (settings) {
    if (settings.migration_complete) return Promise.resolve();

    return invoke("get_migration_server_url").then(function (oldUrl) {
      return new Promise(function (resolve) {
        var iframe = document.createElement("iframe");
        iframe.style.display = "none";
        var done = false;

        function cleanup() {
          if (iframe.parentNode) document.body.removeChild(iframe);
        }

        function onMsg(event) {
          if (done || !event.data || event.data.type !== "migration_data") return;
          done = true;
          window.removeEventListener("message", onMsg);
          cleanup();

          var data = event.data.data;
          if (data) {
            if (data.game_saves) {
              try { localStorage.setItem("game_saves", data.game_saves); }
              catch (e) { console.warn("[MIGRATE] Failed to write game_saves:", e); }
            }
            if (data.aao_engine_config) {
              try { localStorage.setItem("aao_engine_config", data.aao_engine_config); }
              catch (e) { console.warn("[MIGRATE] Failed to write engine_config:", e); }
            }
            console.log("[MIGRATE] Migrated localStorage from old origin");
          }

          invoke("save_settings", {
            settings: Object.assign({}, settings, { migration_complete: true })
          }).then(resolve).catch(resolve);
        }

        window.addEventListener("message", onMsg);
        setTimeout(function () {
          if (!done) {
            done = true;
            window.removeEventListener("message", onMsg);
            cleanup();
            // Mark complete even on timeout (no old data or server unreachable)
            invoke("save_settings", {
              settings: Object.assign({}, settings, { migration_complete: true })
            }).then(resolve).catch(resolve);
          }
        }, 3000);

        iframe.src = oldUrl + "/localstorage_migrate.html";
        document.body.appendChild(iframe);
      });
    });
  }).catch(function (e) {
    console.warn("[MIGRATE] Migration failed:", e);
  });
}

window.addEventListener("DOMContentLoaded", function () {
  // Run one-time localStorage migration before initializing the app.
  // This ensures old saves are available in the new protocol origin.
  migrateLocalStorage().then(initApp);

  function initApp() {
  var statusMsg = document.getElementById("status-msg");
  var caseList = document.getElementById("case-list");
  var emptyLibrary = document.getElementById("empty-library");
  var libraryLoading = document.getElementById("library-loading");

  // Track known case IDs for duplicate detection
  var knownCaseIds = [];

  // --- Shared context bag ---
  // All modules access cross-module functions through this object at call-time.
  // This resolves circular dependencies (library <-> collections).
  var ctx = {
    invoke: invoke,
    Channel: Channel,
    statusMsg: statusMsg,
    caseList: caseList,
    emptyLibrary: emptyLibrary,
    libraryLoading: libraryLoading,
    knownCaseIds: knownCaseIds
  };

  // --- Saves ---
  var savesFns = initSaves({
    invoke: invoke,
    statusMsg: statusMsg,
    loadLibrary: function () { ctx.loadLibrary(); }
  });
  ctx.readGameSaves = savesFns.readGameSaves;
  ctx.writeGameSaves = savesFns.writeGameSaves;
  ctx.findLastSequenceSave = savesFns.findLastSequenceSave;
  ctx.findLastSequenceSaveBridge = savesFns.findLastSequenceSaveBridge;
  ctx.exportSave = savesFns.exportSave;
  ctx.showSavesPluginsModal = savesFns.showSavesPluginsModal;
  ctx.showExportOptionsModal = savesFns.showExportOptionsModal;
  ctx.showPasteSaveModal = savesFns.showPasteSaveModal;
  ctx.doImportSave = savesFns.doImportSave;
  ctx.doExportSave = savesFns.doExportSave;
  ctx.copyTrialLink = savesFns.copyTrialLink;

  // --- Player ---
  var playerFns = initPlayer({
    invoke: invoke,
    statusMsg: statusMsg,
    loadLibrary: function () { ctx.loadLibrary(); },
    writeGameSaves: function (saves) { return ctx.writeGameSaves(saves); },
    nextBridgeId: function () { return savesFns.nextBridgeId("backup"); }
  });
  ctx.showPlayer = playerFns.showPlayer;
  ctx.showLauncher = playerFns.showLauncher;

  // --- Plugins ---
  var pluginsFns = initPlugins({
    invoke: invoke,
    statusMsg: statusMsg,
    loadLibrary: function () { ctx.loadLibrary(); },
    getCachedCases: function () { return ctx.getCachedCases(); },
    getCachedCollections: function () { return ctx.getCachedCollections(); }
  });
  ctx.showPluginManagerModal = pluginsFns.showPluginManagerModal;
  ctx.showScopedPluginModal = pluginsFns.showScopedPluginModal;
  ctx.loadGlobalPluginsPanel = pluginsFns.loadGlobalPluginsPanel;
  ctx.showPluginParamsModal = pluginsFns.showPluginParamsModal;
  ctx.showAttachCodeModal = pluginsFns.showAttachCodeModal;
  ctx.showPluginTargetModal = pluginsFns.showPluginTargetModal;
  ctx.showPluginPickerModal = pluginsFns.showPluginPickerModal;
  ctx.doImportPlugin = pluginsFns.doImportPlugin;
  ctx.pluginsPanel = pluginsFns.pluginsPanel;
  ctx.pluginsToggle = pluginsFns.pluginsToggle;

  // --- Download ---
  var downloadFns = initDownload({
    invoke: invoke,
    Channel: Channel,
    statusMsg: statusMsg,
    loadLibrary: function () { ctx.loadLibrary(); },
    getKnownCaseIds: function () { return knownCaseIds; }
  });
  ctx.startDownload = downloadFns.startDownload;
  ctx.startUpdate = downloadFns.startUpdate;
  ctx.updateCase = downloadFns.updateCase;
  ctx.retryCase = downloadFns.retryCase;
  ctx.startSequenceDownload = downloadFns.startSequenceDownload;
  ctx.downloadInProgress = downloadFns.isDownloadInProgress;
  ctx.progressContainer = downloadFns.progressContainer;
  ctx.progressPhase = downloadFns.progressPhase;
  ctx.progressBarInner = downloadFns.progressBarInner;
  ctx.progressText = downloadFns.progressText;

  // --- Library ---
  var libraryFns = initLibrary(ctx);
  ctx.loadLibrary = libraryFns.loadLibrary;
  ctx.getCachedCases = libraryFns.getCachedCases;
  ctx.getCachedCollections = libraryFns.getCachedCollections;
  ctx.appendSequencePart = libraryFns.appendSequencePart;
  ctx.playCase = libraryFns.playCase;
  ctx.deleteCase = libraryFns.deleteCase;
  ctx.exportCase = libraryFns.exportCase;

  // --- Collections ---
  var collectionsFns = initCollections(ctx);
  ctx.appendCollectionGroup = collectionsFns.appendCollectionGroup;
  ctx.appendSequenceGroupInto = collectionsFns.appendSequenceGroupInto;
  ctx.appendCaseCardInto = collectionsFns.appendCaseCardInto;
  ctx.exportCollection = collectionsFns.exportCollection;
  ctx.showEditCollectionModal = collectionsFns.showEditCollectionModal;

  // --- Settings ---
  var settingsFns = initSettings(invoke, Channel, statusMsg);
  ctx.loadSettings = settingsFns.loadSettings;
  ctx.loadStorageInfo = settingsFns.loadStorageInfo;

  // --- Import ---
  initImport({
    invoke: invoke,
    Channel: Channel,
    statusMsg: statusMsg,
    loadLibrary: function () { ctx.loadLibrary(); },
    showPasteSaveModal: ctx.showPasteSaveModal,
    writeGameSaves: ctx.writeGameSaves,
    doImportSave: ctx.doImportSave,
    doImportPlugin: ctx.doImportPlugin,
    progressContainer: ctx.progressContainer,
    progressPhase: ctx.progressPhase,
    progressBarInner: ctx.progressBarInner,
    progressText: ctx.progressText,
    isDownloadInProgress: ctx.downloadInProgress
  });

  // --- Init ---
  ctx.loadLibrary();
  settingsFns.loadSettings();
  } // end initApp
});
