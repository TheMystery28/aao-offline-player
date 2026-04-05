"use strict";
/*
Ace Attorney Online - Court Record Navigator

X key (keyboard) / X button (gamepad) toggles navigation focus onto the
court record evidence/profiles list. Arrows navigate, Enter/Space/A
selects (click summary), long-press Enter/Space/A triggers the Check
button when available.

Emits:
  courtrecord:nav:enter    {}                        — navigation mode activated
  courtrecord:nav:leave    {}                        — navigation mode deactivated
  courtrecord:nav:highlight {index, element, tab}    — highlight moved
  courtrecord:nav:select   {index, element, tab}     — item selected (summary clicked)
  courtrecord:nav:check    {index, element, tab}     — item check button clicked
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'courtrecord_navigator',
	dependencies : ['engine_events', 'input_registry', 'events', 'page_loaded'],
	init : function()
	{
		InputRegistry.register({ action: 'browseEvidence', label: 'browse evidence/profiles', keyboard: 'KeyX', gamepad: 'X', source: 'engine', module: 'courtrecord_navigator' });
		InputRegistry.register({ action: 'checkEvidence', label: 'check evidence (hold)', keyboard: 'Enter (hold)', gamepad: 'A (hold)', source: 'engine', module: 'courtrecord_navigator' });

		var crNavActive = false;
		var highlightIndex = -1;
		var LONG_PRESS_MS = 500;

		// Long-press state
		var longPressTimer = null;
		var longPressTriggered = false;

		// --- Helpers ---

		function isVisible(el) {
			if (!el) return false;
			if (el.offsetWidth === 0 && el.offsetHeight === 0) return false;
			return getComputedStyle(el).visibility !== 'hidden';
		}

		function getActiveTab() {
			var cr = document.getElementById('courtrecord');
			if (!cr) return null;
			if (cr.classList.contains('profiles')) return 'profiles';
			return 'evidence'; // default
		}

		function getVisibleItems() {
			var tab = getActiveTab();
			var listId = tab === 'profiles' ? 'cr_profiles_list' : 'cr_evidence_list';
			var list = document.getElementById(listId);
			if (!list) return [];
			var all = list.children;
			var visible = [];
			for (var i = 0; i < all.length; i++) {
				if (!all[i].hidden) visible.push(all[i]);
			}
			return visible;
		}

		function clearHighlight(items) {
			for (var i = 0; i < items.length; i++) {
				items[i].classList.remove('cr-nav-highlight');
			}
		}

		function setHighlight(items, idx) {
			clearHighlight(items);
			if (idx >= 0 && idx < items.length) {
				highlightIndex = idx;
				items[idx].classList.add('cr-nav-highlight');
				items[idx].scrollIntoView({ block: 'nearest' });
				EngineEvents.emit('courtrecord:nav:highlight', {
					index: idx,
					element: items[idx],
					tab: getActiveTab()
				});
			}
		}

		function getCheckButton(item) {
			// Check button: .details .buttonbar-bottom > a.bottomright
			var details = item.querySelector('.details');
			if (!details) return null;
			return details.querySelector('.buttonbar-bottom > a.bottomright');
		}

		function selectItem(items, idx) {
			if (idx < 0 || idx >= items.length) return;
			var summary = items[idx].querySelector('.summary');
			if (summary && isVisible(summary)) {
				deactivate();
				EngineEvents.emit('courtrecord:nav:select', {
					index: idx,
					element: items[idx],
					tab: getActiveTab()
				});
				summary.click();
			}
		}

		function checkItem(items, idx) {
			if (idx < 0 || idx >= items.length) return;
			var checkBtn = getCheckButton(items[idx]);
			if (checkBtn && isVisible(checkBtn)) {
				deactivate(); // exit nav mode before opening check screen
				EngineEvents.emit('courtrecord:nav:check', {
					index: idx,
					element: items[idx],
					tab: getActiveTab()
				});
				checkBtn.click();
			}
		}

		function activate() {
			crNavActive = true;
			InputManager.disableModule('option_navigator');
			var items = getVisibleItems();
			if (items.length > 0) {
				setHighlight(items, 0);
			}
			EngineEvents.emit('courtrecord:nav:enter', {});
		}

		function deactivate() {
			crNavActive = false;
			InputManager.enableModule('option_navigator');
			highlightIndex = -1;
			clearLongPress();
			var items = getVisibleItems();
			clearHighlight(items);
			EngineEvents.emit('courtrecord:nav:leave', {});
		}

		// --- Spatial grid navigation ---
		// Reads element bounding rects to navigate a flex-wrap grid spatially.

		function getCenter(el) {
			var r = el.getBoundingClientRect();
			return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
		}

		function navigateGrid(items, currentIdx, direction) {
			if (items.length === 0) return 0;
			if (currentIdx < 0) return 0;

			var cur = getCenter(items[currentIdx]);
			var bestIdx = -1;
			var bestDist = Infinity;

			for (var i = 0; i < items.length; i++) {
				if (i === currentIdx) continue;
				var c = getCenter(items[i]);
				var dx = c.x - cur.x;
				var dy = c.y - cur.y;

				// Check if candidate is in the right direction
				var valid = false;
				switch (direction) {
					case 'up':    valid = dy < -5; break;
					case 'down':  valid = dy > 5; break;
					case 'left':  valid = dx < -5; break;
					case 'right': valid = dx > 5; break;
				}
				if (!valid) continue;

				// Prefer items aligned on the cross-axis (closer = better)
				var dist;
				if (direction === 'up' || direction === 'down') {
					dist = Math.abs(dy) + Math.abs(dx) * 0.5;
				} else {
					dist = Math.abs(dx) + Math.abs(dy) * 0.5;
				}
				if (dist < bestDist) {
					bestDist = dist;
					bestIdx = i;
				}
			}

			// If no item found in direction, wrap around
			if (bestIdx === -1) {
				switch (direction) {
					case 'down':
					case 'right': return 0;
					case 'up':
					case 'left':  return items.length - 1;
				}
			}
			return bestIdx;
		}

		function moveHighlight(items, direction) {
			if (items.length === 0) return;
			if (highlightIndex < 0) {
				setHighlight(items, 0);
				return;
			}
			setHighlight(items, navigateGrid(items, highlightIndex, direction));
		}

		function clearLongPress() {
			if (longPressTimer) {
				clearTimeout(longPressTimer);
				longPressTimer = null;
			}
			// Note: longPressTriggered is NOT reset here — it must survive
			// deactivate() so key repeats are still swallowed. Reset on keyup.
		}

		// --- Keyboard handler (capture phase to intercept before InputManager) ---

		document.addEventListener('keydown', function(e) {
			if (InputManager.isModuleDisabled('courtrecord_navigator', 'keyboard')) return;
			// X key toggles CR navigation
			if (e.code === 'KeyX' && !e.ctrlKey && !e.altKey && !e.metaKey) {
				if (crNavActive) {
					deactivate();
				} else {
					activate();
				}
				e.preventDefault();
				e.stopImmediatePropagation();
				return;
			}

			// After long-press fires checkItem → deactivate(), key repeats
			// still arrive while the key is held. Swallow them to prevent
			// proceed from firing.
			if (!crNavActive && longPressTriggered) {
				if (e.code === 'Enter' || e.code === 'Space' || e.code === 'NumpadEnter') {
					e.preventDefault();
					e.stopImmediatePropagation();
				}
				return;
			}

			if (!crNavActive) return;

			var items = getVisibleItems();
			if (items.length === 0) return;

			// Arrow navigation (spatial grid)
			var dir = null;
			if (e.code === 'ArrowUp') dir = 'up';
			else if (e.code === 'ArrowDown') dir = 'down';
			else if (e.code === 'ArrowLeft') dir = 'left';
			else if (e.code === 'ArrowRight') dir = 'right';
			if (dir) {
				e.preventDefault();
				e.stopImmediatePropagation();
				moveHighlight(items, dir);
				return;
			}

			// Enter/Space — long-press detection
			if (e.code === 'Enter' || e.code === 'Space' || e.code === 'NumpadEnter') {
				e.preventDefault();
				e.stopImmediatePropagation();
				if (e.repeat) return; // ignore key repeat
				if (highlightIndex < 0 || highlightIndex >= items.length) return;

				longPressTriggered = false;
				longPressTimer = setTimeout(function() {
					longPressTriggered = true;
					checkItem(items, highlightIndex);
				}, LONG_PRESS_MS);
				return;
			}

			// Escape — deactivate CR nav, then also click any visible back button
			if (e.code === 'Escape') {
				e.preventDefault();
				e.stopImmediatePropagation();
				deactivate();
				var backIds = ['cr-item-check-back', 'back', 'examination-back'];
				for (var bi = 0; bi < backIds.length; bi++) {
					var btn = document.getElementById(backIds[bi]);
					if (btn && (btn.offsetWidth > 0 || btn.offsetHeight > 0)) {
						btn.click();
						break;
					}
				}
				return;
			}
		}, true); // capture phase — fires before InputManager

		document.addEventListener('keyup', function(e) {
			if (InputManager.isModuleDisabled('courtrecord_navigator', 'keyboard')) return;
			if (e.code === 'Enter' || e.code === 'Space' || e.code === 'NumpadEnter') {
				// Always intercept keyup if long-press was triggered (even after deactivate)
				if (longPressTriggered || crNavActive) {
					e.preventDefault();
					e.stopImmediatePropagation();
				}

				if (longPressTimer) {
					// Released before long-press threshold → quick select
					clearTimeout(longPressTimer);
					longPressTimer = null;
					if (!longPressTriggered && crNavActive) {
						var items = getVisibleItems();
						selectItem(items, highlightIndex);
					}
				}
				longPressTriggered = false;
			}
		}, true); // capture phase

		// --- Gamepad: intercept proceed action when CR nav is active ---
		// Priority -1 fires before keyboard/gamepad_controls (priority 0).
		// Sets data._consumed so gamepad_controls skips the click.

		EngineEvents.on('input:action', function(data) {
			if (!crNavActive) return;

			if (data.action === 'proceed' && data.source === 'gamepad') {
				data._consumed = true;
				// Long-press is handled in the gamepad poll below
			}
		}, -1, 'engine');

		// --- Gamepad polling ---
		// Button 2 (X) = toggle, d-pad = navigate, button 0 (A) = select/check

		var gpWasPressed = {};
		var gpALongPressTimer = null;
		var gpALongPressTriggered = false;

		function gpCheck(gpButtons, btnIdx) {
			return gpButtons[btnIdx] && gpButtons[btnIdx].pressed;
		}

		function pollGamepad() {
			if (InputManager.isModuleDisabled('courtrecord_navigator', 'gamepad')) { requestAnimationFrame(pollGamepad); return; }
			var gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
			for (var g = 0; g < gamepads.length; g++) {
				var gp = gamepads[g];
				if (!gp) continue;

				// Button 2 (X) — toggle CR navigation
				var xKey = g + '_crx';
				if (gpCheck(gp.buttons, 2)) {
					if (!gpWasPressed[xKey]) {
						gpWasPressed[xKey] = true;
						if (crNavActive) deactivate();
						else activate();
					}
				} else {
					gpWasPressed[xKey] = false;
				}

				if (!crNavActive) continue;

				var items = getVisibleItems();
				if (items.length === 0) continue;

				// D-pad navigation (spatial grid)
				var dpadActions = [
					{ btn: 12, dir: 'up' },
					{ btn: 13, dir: 'down' },
					{ btn: 14, dir: 'left' },
					{ btn: 15, dir: 'right' }
				];

				for (var d = 0; d < dpadActions.length; d++) {
					var dKey = g + '_crd_' + dpadActions[d].btn;
					if (gpCheck(gp.buttons, dpadActions[d].btn)) {
						if (!gpWasPressed[dKey]) {
							gpWasPressed[dKey] = true;
							moveHighlight(items, dpadActions[d].dir);
						}
					} else {
						gpWasPressed[dKey] = false;
					}
				}

				// Button 0 (A) — select (short) / check (long press)
				var aKey = g + '_cra';
				if (gpCheck(gp.buttons, 0)) {
					if (!gpWasPressed[aKey]) {
						gpWasPressed[aKey] = true;
						gpALongPressTriggered = false;
						gpALongPressTimer = setTimeout(function() {
							gpALongPressTriggered = true;
							var currentItems = getVisibleItems();
							checkItem(currentItems, highlightIndex);
						}, LONG_PRESS_MS);
					}
				} else {
					if (gpWasPressed[aKey]) {
						gpWasPressed[aKey] = false;
						// Released — if short press, select
						if (gpALongPressTimer) {
							clearTimeout(gpALongPressTimer);
							gpALongPressTimer = null;
						}
						if (!gpALongPressTriggered) {
							selectItem(items, highlightIndex);
						}
						gpALongPressTriggered = false;
					}
				}
			}

			requestAnimationFrame(pollGamepad);
		}

		// Always start gamepad polling — gamepadconnected may have
		// already fired before this module loaded.
		requestAnimationFrame(pollGamepad);

		// --- Deactivate on frame transitions ---

		EngineEvents.on('frame:before', function() {
			if (crNavActive) deactivate();
		}, 0, 'engine');

		// --- Reset highlight when switching tabs ---

		EngineEvents.on('config:changed', function(data) {
			if (!crNavActive) return;
			// Tab switch triggers a courtrecord class change
			// Reset highlight to first item of the new tab
			highlightIndex = -1;
			var items = getVisibleItems();
			if (items.length > 0) {
				setHighlight(items, 0);
			}
		}, 0, 'engine');
	}
}));

//END OF MODULE
Modules.complete('courtrecord_navigator');
