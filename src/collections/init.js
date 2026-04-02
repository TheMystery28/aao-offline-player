import { appendCollectionGroup, appendSequenceGroupInto, appendCaseCardInto,
         findFirstPlayableInCollection, getCollectionCaseIds, exportCollection } from './rendering.js';
import { showNewCollectionModal, showEditCollectionModal } from './modals.js';

/**
 * Initialise the Collections section: collection rendering, modals, and actions.
 *
 * @param {AppContext} ctx - Shared context bag. Functions from other modules
 *   (library, plugins, download, saves, player) are accessed through ctx
 *   at call-time, so they may be attached after this init runs.
 *
 * Required at init:
 *   invoke, Channel, statusMsg, caseList
 *
 * Required at call-time (via ctx):
 *   showPlayer, findLastSequenceSave, showSavesPluginsModal, showExportOptionsModal,
 *   showPluginManagerModal, showScopedPluginModal, loadGlobalPluginsPanel,
 *   pluginsPanel, pluginsToggle, startSequenceDownload, downloadInProgress,
 *   updateCase, retryCase, copyTrialLink,
 *   progressContainer, progressPhase, progressBarInner, progressText,
 *   appendSequencePart (from library.js), playCase, deleteCase, exportCase,
 *   loadLibrary
 */
export function initCollections(ctx) {
  ctx.appendCollectionGroup = function (col, cases, seqGroups, q) {
    appendCollectionGroup(ctx, col, cases, seqGroups, q);
  };
  ctx.appendSequenceGroupInto = function (container, title, list, cases, q) {
    appendSequenceGroupInto(ctx, container, title, list, cases, q);
  };
  ctx.appendCaseCardInto = function (container, c) {
    appendCaseCardInto(ctx, container, c);
  };
  ctx.exportCollection = function (col, ids) {
    exportCollection(ctx, col, ids);
  };
  ctx.showEditCollectionModal = function (col) {
    showEditCollectionModal(ctx, col);
  };

  const newCollectionBtn = document.getElementById("new-collection-btn");
  if (newCollectionBtn) {
    newCollectionBtn.addEventListener("click", function () {
      showNewCollectionModal(ctx);
    });
  }

  return {
    appendCollectionGroup: ctx.appendCollectionGroup,
    appendSequenceGroupInto: ctx.appendSequenceGroupInto,
    appendCaseCardInto: ctx.appendCaseCardInto,
    exportCollection: ctx.exportCollection,
    showEditCollectionModal: ctx.showEditCollectionModal
  };
}
