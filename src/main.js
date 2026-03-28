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

window.addEventListener("DOMContentLoaded", function () {
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
});
