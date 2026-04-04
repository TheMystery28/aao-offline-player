"use strict";
/*
Ace Attorney Online - Option Navigator

Keyboard (1-9, arrows, Enter) and gamepad (d-pad, A) navigation for
option lists (MultipleChoices) and investigation menus.

Emits:
  options:highlight  { index, element, mode }   — highlight moved
  options:select     { index, element, mode }   — option selected
  options:enter      { mode }                   — options panel became active
  options:leave      { mode }                   — options panel became inactive
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'option_navigator',
	dependencies : ['engine_events', 'input_registry', 'events', 'page_loaded'],
	init : function()
	{
		InputRegistry.register({ action: 'selectOption', label: 'select option (1-9)', keyboard: '1-9', gamepad: '—', source: 'engine' });
		InputRegistry.register({ action: 'navigateOptions', label: 'navigate options', keyboard: 'Arrows', gamepad: 'D-Pad', source: 'engine' });

		var highlightIndex = -1;
		var bottomScreen = document.getElementById('screen-bottom');

		// --- Helpers ---

		function getMode() {
			if (!bottomScreen) return null;
			if (bottomScreen.classList.contains('options')) return 'options';
			if (bottomScreen.classList.contains('options-investigation')) return 'investigation';
			return null;
		}

		function getOptionElements(mode) {
			if (mode === 'options') {
				var container = document.getElementById('options');
				return container ? Array.prototype.slice.call(container.children) : [];
			}
			if (mode === 'investigation') {
				var ids = [
					'options-investigation-examine',
					'options-investigation-move',
					'options-investigation-talk',
					'options-investigation-present'
				];
				var elems = [];
				for (var i = 0; i < ids.length; i++) {
					var el = document.getElementById(ids[i]);
					if (el) elems.push(el);
				}
				return elems;
			}
			return [];
		}

		function clearHighlight(elems) {
			for (var i = 0; i < elems.length; i++) {
				elems[i].classList.remove('option-nav-highlight');
			}
		}

		function setHighlight(elems, idx) {
			clearHighlight(elems);
			if (idx >= 0 && idx < elems.length) {
				highlightIndex = idx;
				elems[idx].classList.add('option-nav-highlight');
				if (elems[idx].scrollIntoView) {
					elems[idx].scrollIntoView({ block: 'nearest' });
				}
				EngineEvents.emit('options:highlight', {
					index: idx,
					element: elems[idx],
					mode: getMode()
				});
			}
		}

		function selectOption(el, idx) {
			if (el) {
				var mode = getMode();
				highlightIndex = -1;
				EngineEvents.emit('options:select', {
					index: idx,
					element: el,
					mode: mode
				});
				el.click();
			}
		}

		// --- Investigation grid navigation ---
		// Layout (2x2): [0=examine, 1=move] / [2=talk, 3=present]

		function invRow(idx) { return idx < 2 ? 0 : 1; }
		function invCol(idx) { return idx % 2; }
		function invIdx(row, col) { return row * 2 + col; }

		function moveInvestigation(direction, elems) {
			var count = elems.length;
			if (count === 0) return;
			if (highlightIndex < 0 || highlightIndex >= count) {
				setHighlight(elems, 0);
				return;
			}
			var row = invRow(highlightIndex);
			var col = invCol(highlightIndex);
			switch (direction) {
				case 'up':    row = row === 0 ? 1 : 0; break;
				case 'down':  row = row === 1 ? 0 : 1; break;
				case 'left':  col = col === 0 ? 1 : 0; break;
				case 'right': col = col === 1 ? 0 : 1; break;
			}
			var target = invIdx(row, col);
			if (target < count) setHighlight(elems, target);
		}

		// --- Keyboard handler ---

		document.addEventListener('keydown', function(e) {
			if (InputManager.isModuleDisabled('option_navigator')) return;
			var mode = getMode();
			if (!mode) return;

			var elems = getOptionElements(mode);
			if (elems.length === 0) return;

			// Number keys 1-9 (numrow + numpad) — direct selection
			var digitMatch = e.code.match(/^(?:Digit|Numpad)([1-9])$/);
			if (digitMatch) {
				var idx = parseInt(digitMatch[1], 10) - 1;
				if (idx < elems.length) {
					e.preventDefault();
					selectOption(elems[idx], idx);
				}
				return;
			}

			// Arrow navigation
			if (mode === 'options') {
				if (e.code === 'ArrowDown' || e.code === 'ArrowRight') {
					e.preventDefault();
					if (highlightIndex < 0) {
						setHighlight(elems, 0);
					} else {
						setHighlight(elems, (highlightIndex + 1) % elems.length);
					}
					return;
				}
				if (e.code === 'ArrowUp' || e.code === 'ArrowLeft') {
					e.preventDefault();
					if (highlightIndex < 0) {
						setHighlight(elems, elems.length - 1);
					} else {
						setHighlight(elems, (highlightIndex - 1 + elems.length) % elems.length);
					}
					return;
				}
			}

			if (mode === 'investigation') {
				var dir = null;
				if (e.code === 'ArrowUp') dir = 'up';
				else if (e.code === 'ArrowDown') dir = 'down';
				else if (e.code === 'ArrowLeft') dir = 'left';
				else if (e.code === 'ArrowRight') dir = 'right';
				if (dir) {
					e.preventDefault();
					moveInvestigation(dir, elems);
					return;
				}
			}

			// Enter/Space to select highlighted option
			if (e.code === 'Enter' || e.code === 'Space' || e.code === 'NumpadEnter') {
				if (highlightIndex >= 0 && highlightIndex < elems.length) {
					e.preventDefault();
					selectOption(elems[highlightIndex], highlightIndex);
					return;
				}
			}
		});

		// --- Gamepad d-pad polling ---
		// Standard gamepad: button 12=up, 13=down, 14=left, 15=right

		var gpWasPressed = {};

		function gpCheck(gpButtons, btnIdx) {
			return gpButtons[btnIdx] && gpButtons[btnIdx].pressed;
		}

		function pollGamepad() {
			if (InputManager.isModuleDisabled('option_navigator')) { requestAnimationFrame(pollGamepad); return; }
			var mode = getMode();
			if (mode) {
				var gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
				for (var g = 0; g < gamepads.length; g++) {
					var gp = gamepads[g];
					if (!gp) continue;

					var elems = getOptionElements(mode);
					if (elems.length === 0) continue;

					// D-pad buttons
					var dpadActions = [
						{ btn: 12, dir: 'up' },
						{ btn: 13, dir: 'down' },
						{ btn: 14, dir: 'left' },
						{ btn: 15, dir: 'right' }
					];

					for (var d = 0; d < dpadActions.length; d++) {
						var key = g + '_dpad_' + dpadActions[d].btn;
						if (gpCheck(gp.buttons, dpadActions[d].btn)) {
							if (!gpWasPressed[key]) {
								gpWasPressed[key] = true;
								if (mode === 'options') {
									if (dpadActions[d].dir === 'down' || dpadActions[d].dir === 'right') {
										if (highlightIndex < 0) setHighlight(elems, 0);
										else setHighlight(elems, (highlightIndex + 1) % elems.length);
									} else if (dpadActions[d].dir === 'up' || dpadActions[d].dir === 'left') {
										if (highlightIndex < 0) setHighlight(elems, elems.length - 1);
										else setHighlight(elems, (highlightIndex - 1 + elems.length) % elems.length);
									}
								} else if (mode === 'investigation') {
									moveInvestigation(dpadActions[d].dir, elems);
								}
							}
						} else {
							gpWasPressed[key] = false;
						}
					}

					// Button 0 (A/proceed) — select highlighted option
					var aKey = g + '_select';
					if (gpCheck(gp.buttons, 0)) {
						if (!gpWasPressed[aKey]) {
							gpWasPressed[aKey] = true;
							if (highlightIndex >= 0 && highlightIndex < elems.length) {
								selectOption(elems[highlightIndex], highlightIndex);
							}
						}
					} else {
						gpWasPressed[aKey] = false;
					}
				}
			}

			requestAnimationFrame(pollGamepad);
		}

		// Always start gamepad polling — gamepadconnected may have
		// already fired before this module loaded.
		requestAnimationFrame(pollGamepad);

		// --- Reset highlight on mode transitions ---

		var lastMode = null;

		EngineEvents.on('frame:after', function() {
			var mode = getMode();
			if (mode !== lastMode) {
				if (lastMode) {
					highlightIndex = -1;
					var oldElems = getOptionElements(lastMode);
					clearHighlight(oldElems);
					EngineEvents.emit('options:leave', { mode: lastMode });
				}
				if (mode) {
					EngineEvents.emit('options:enter', { mode: mode });
				}
				lastMode = mode;
			}
		}, 0, 'engine');
	}
}));

//END OF MODULE
Modules.complete('option_navigator');
