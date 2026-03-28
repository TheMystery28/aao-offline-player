import { showPluginTargetModal, doImportPlugin } from './pluginTarget.js';
import { showPluginManagerModal } from './pluginManager.js';
import { showScopedPluginModal } from './scopedModal.js';
import { showScopeEditorModal } from './scopeEditor.js';
import { showPluginPickerModal } from './pluginPicker.js';
import { showPluginParamsModal } from './paramsEditor.js';
import { showAttachCodeModal } from './attachCode.js';
import { initPluginPanel } from './pluginPanel.js';

export function initPlugins(ctx) {
  // Wire cross-references onto ctx
  ctx.showPluginParamsModal = function(f,l,lv,k) { showPluginParamsModal(ctx,f,l,lv,k); };
  ctx.showScopedPluginModal = function(t,k,l) { showScopedPluginModal(ctx,t,k,l); };
  ctx.showScopeEditorModal = function(f) { showScopeEditorModal(ctx,f); };
  ctx.showPluginManagerModal = function(id,t) { showPluginManagerModal(ctx,id,t); };
  ctx.showAttachCodeModal = function(id,t,cb) { showAttachCodeModal(ctx,id,t,cb); };
  ctx.showPluginPickerModal = function(s,cb) { showPluginPickerModal(ctx,s,cb); };
  ctx.showPluginTargetModal = function(cb) { showPluginTargetModal(ctx,cb); };
  ctx.doImportPlugin = function(p) { doImportPlugin(ctx,p); };

  var panelFns = initPluginPanel(ctx);
  ctx.loadGlobalPluginsPanel = panelFns.loadGlobalPluginsPanel;

  return {
    showPluginManagerModal: ctx.showPluginManagerModal,
    showScopedPluginModal: ctx.showScopedPluginModal,
    loadGlobalPluginsPanel: ctx.loadGlobalPluginsPanel,
    showPluginParamsModal: ctx.showPluginParamsModal,
    showAttachCodeModal: ctx.showAttachCodeModal,
    showPluginTargetModal: ctx.showPluginTargetModal,
    showPluginPickerModal: ctx.showPluginPickerModal,
    doImportPlugin: ctx.doImportPlugin,
    pluginsPanel: panelFns.pluginsPanel,
    pluginsToggle: panelFns.pluginsToggle
  };
}
