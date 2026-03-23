"use strict";
/**
 * Minimal test harness for AAO engine regression tests.
 * ES2017 max — no external dependencies.
 */
var TestHarness = (function() {
	var results = [];
	var currentSuite = '(default)';

	function record(pass, message) {
		results.push({
			suite: currentSuite,
			pass: pass,
			message: message
		});
		var prefix = pass ? 'PASS' : 'FAIL';
		console.log('[' + prefix + '] [' + currentSuite + '] ' + message);
	}

	return {
		suite: function(name) {
			currentSuite = name;
		},

		assert: function(condition, message) {
			record(!!condition, message);
		},

		assertEqual: function(actual, expected, message) {
			var pass = actual === expected;
			var detail = pass ? message : message + ' (expected: ' + JSON.stringify(expected) + ', got: ' + JSON.stringify(actual) + ')';
			record(pass, detail);
		},

		assertDefined: function(value, message) {
			record(typeof value !== 'undefined', message);
		},

		assertThrows: function(fn, message) {
			var threw = false;
			try {
				fn();
			} catch (e) {
				threw = true;
			}
			record(threw, message);
		},

		assertType: function(value, type, message) {
			var actual = typeof value;
			var pass = actual === type;
			var detail = pass ? message : message + ' (expected typeof: ' + type + ', got: ' + actual + ')';
			record(pass, detail);
		},

		report: function() {
			var total = results.length;
			var passed = 0;
			var failed = 0;
			var failedTests = [];

			for (var i = 0; i < results.length; i++) {
				if (results[i].pass) {
					passed++;
				} else {
					failed++;
					failedTests.push(results[i]);
				}
			}

			console.log('');
			console.log('========================================');
			console.log('TEST RESULTS: ' + passed + '/' + total + ' passed, ' + failed + ' failed');
			console.log('========================================');

			if (failedTests.length > 0) {
				console.log('');
				console.log('FAILURES:');
				for (var j = 0; j < failedTests.length; j++) {
					console.log('  [' + failedTests[j].suite + '] ' + failedTests[j].message);
				}
			}

			// Render results to the page if a container exists
			var container = document.getElementById('test-results');
			if (container) {
				var summary = document.createElement('h2');
				summary.textContent = passed + '/' + total + ' passed, ' + failed + ' failed';
				summary.style.color = failed > 0 ? '#cc0000' : '#00aa00';
				container.appendChild(summary);

				// Group by suite
				var suites = {};
				for (var k = 0; k < results.length; k++) {
					var r = results[k];
					if (!suites[r.suite]) {
						suites[r.suite] = [];
					}
					suites[r.suite].push(r);
				}

				for (var suiteName in suites) {
					var section = document.createElement('details');
					var suiteTests = suites[suiteName];
					var suitePass = 0;
					for (var m = 0; m < suiteTests.length; m++) {
						if (suiteTests[m].pass) suitePass++;
					}

					var header = document.createElement('summary');
					var allPassed = suitePass === suiteTests.length;
					header.textContent = (allPassed ? '\u2705 ' : '\u274C ') + suiteName + ' (' + suitePass + '/' + suiteTests.length + ')';
					header.style.color = allPassed ? '#00aa00' : '#cc0000';
					section.appendChild(header);

					var list = document.createElement('ul');
					for (var n = 0; n < suiteTests.length; n++) {
						var item = document.createElement('li');
						item.textContent = (suiteTests[n].pass ? '\u2705 ' : '\u274C ') + suiteTests[n].message;
						item.style.color = suiteTests[n].pass ? '#006600' : '#cc0000';
						list.appendChild(item);
					}
					section.appendChild(list);
					container.appendChild(section);
				}
			}

			return failed === 0;
		}
	};
})();
