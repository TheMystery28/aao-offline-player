import { parseCaseId, formatBytes, formatDuration, escapeHtml, showConfirmModal, showUpdateModal, createModal, applySpoilerBlur, removeSpoilerBlur } from './helpers.js';

/**
 * Initialise the Download section UI and event listeners.
 *
 * @param {AppContext} ctx - Context bag from the main closure
 */
export function initDownload(ctx) {
  var invoke = ctx.invoke;
  var Channel = ctx.Channel;
  var statusMsg = ctx.statusMsg;
  var loadLibrary = ctx.loadLibrary;
  var getKnownCaseIds = ctx.getKnownCaseIds;

  // DOM refs
  var downloadBtn = document.getElementById("download-btn");
  var caseIdInput = document.getElementById("case-id-input");
  var downloadResult = document.getElementById("download-result");
  var progressContainer = document.getElementById("progress-container");
  var progressPhase = document.getElementById("progress-phase");
  var progressBarInner = document.getElementById("progress-bar-inner");
  var progressText = document.getElementById("progress-text");
  var cancelDownloadBtn = document.getElementById("cancel-download-btn");

  var downloadInProgress = false;
  var downloadQueue = [];

  function createDownloadProgressHandler(options) {
    return function (msg) {
      if (options.logPrefix) console.log(options.logPrefix, JSON.stringify(msg));
      if (msg.event === "sequence_progress" && options.onSequenceProgress) {
        options.onSequenceProgress(msg.data);
      } else if (msg.event === "started") {
        progressPhase.textContent = (options.startedLabel || "Downloading") + " " + msg.data.total + " assets...";
        progressText.textContent = "0 / " + msg.data.total;
      } else if (msg.event === "progress") {
        var pct = msg.data.total > 0 ? Math.round((msg.data.completed / msg.data.total) * 100) : 0;
        progressBarInner.style.width = pct + "%";
        progressText.textContent =
          msg.data.completed + " / " + msg.data.total + " (" + pct + "%)";
        if (msg.data.current_url) {
          var fname = msg.data.current_url.split("/").pop();
          if (fname.length > 40) fname = fname.substring(0, 37) + "...";
          progressText.textContent += " — " + fname;
          applySpoilerBlur(progressText);
        }
        if (msg.data.elapsed_ms > 1000 && msg.data.bytes_downloaded > 0) {
          var speed = msg.data.bytes_downloaded / (msg.data.elapsed_ms / 1000);
          progressText.textContent += " — " + formatBytes(speed) + "/s";
          if (msg.data.completed > 0 && msg.data.completed < msg.data.total) {
            var etaMs = (msg.data.total - msg.data.completed) * (msg.data.elapsed_ms / msg.data.completed);
            progressText.textContent += " — ~" + formatDuration(etaMs) + " left";
          }
        }
      } else if (msg.event === "finished") {
        progressBarInner.style.width = "100%";
        progressPhase.textContent = options.finishedLabel || "Complete!";
        removeSpoilerBlur(progressText);
        var failedText = msg.data.failed > 0
          ? ", " + msg.data.failed + " " + (options.failedLabel || "failed")
          : "";
        progressText.textContent =
          msg.data.downloaded + " downloaded" + failedText +
          " (" + formatBytes(msg.data.total_bytes) + ")";
      } else if (msg.event === "error") {
        progressPhase.textContent = "Error";
        progressText.textContent = msg.data.message;
      }
    };
  }

  function processQueue() {
    if (downloadInProgress || downloadQueue.length === 0) return;
    var next = downloadQueue.shift();
    statusMsg.textContent = "Starting queued download...";
    if (next.type === "single") {
      startDownload(next.caseId);
    } else if (next.type === "sequence") {
      startSequenceDownload(next.caseIds, next.sequenceTitle);
    }
  }

  // Cancel button handler
  cancelDownloadBtn.addEventListener("click", function () {
    invoke("cancel_download").then(function () {
      progressPhase.textContent = "Cancelling...";
      cancelDownloadBtn.classList.add("hidden");
    });
  });

  // --- Download button listener ---

  caseIdInput.addEventListener("keydown", function (e) {
    if (e.key === "Enter") {
      downloadBtn.click();
    }
  });

  downloadBtn.addEventListener("click", function () {
    var caseId = parseCaseId(caseIdInput.value);
    if (!caseId) {
      downloadResult.textContent =
        "Please enter a valid case ID or AAO URL.";
      downloadResult.className = "result-error";
      return;
    }

    if (downloadInProgress) {
      statusMsg.textContent = "A download is already in progress.";
      return;
    }

    function proceedWithDownload() {
      // First fetch case info to check for sequence
      downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Checking for sequence...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var knownCaseIds = getKnownCaseIds();
    invoke("fetch_case_info", { caseId: caseId })
      .then(function (caseInfo) {
        var seq = caseInfo.sequence;
        if (seq && seq.list && seq.list.length > 1) {
          // This case is part of a sequence
          progressContainer.classList.add("hidden");
          var partNames = seq.list.map(function (p) { return p.title || ("Case " + p.id); });
          var msg = 'This case is part of "' + (seq.title || "Untitled Sequence") + '" (' +
            seq.list.length + " parts):\n\n";
          for (var i = 0; i < partNames.length; i++) {
            var partId = seq.list[i].id;
            var alreadyDl = knownCaseIds.indexOf(partId) !== -1;
            msg += (i + 1) + ". " + partNames[i] + (alreadyDl ? " (already downloaded)" : "") + "\n";
          }
          var allIds = seq.list.map(function (p) { return p.id; });
          var seqTitle = seq.title || "Untitled Sequence";
          var m = createModal(msg);
          m.titleEl.style.whiteSpace = "pre-wrap";

          function cancelSeqModal() {
            m.close();
            downloadBtn.disabled = false;
            caseIdInput.disabled = false;
          }

          var seqBtns = document.createElement("div");
          seqBtns.className = "modal-buttons";

          var seqAllBtn = document.createElement("button");
          seqAllBtn.className = "modal-btn modal-btn-primary";
          seqAllBtn.textContent = "Download All Parts";
          seqAllBtn.addEventListener("click", function () {
            m.close();
            startSequenceDownload(allIds, seqTitle);
          });

          var seqOneBtn = document.createElement("button");
          seqOneBtn.className = "modal-btn modal-btn-secondary";
          seqOneBtn.textContent = "This Case Only";
          seqOneBtn.addEventListener("click", function () {
            m.close();
            startDownload(caseId);
          });

          var seqCancelBtn = document.createElement("button");
          seqCancelBtn.className = "modal-btn modal-btn-cancel";
          seqCancelBtn.textContent = "Cancel";
          seqCancelBtn.addEventListener("click", cancelSeqModal);

          seqBtns.appendChild(seqAllBtn);
          seqBtns.appendChild(seqOneBtn);
          seqBtns.appendChild(seqCancelBtn);
          m.modal.appendChild(seqBtns);
        } else {
          // No sequence, download single case
          progressContainer.classList.add("hidden");
          startDownload(caseId);
        }
      })
      .catch(function (e) {
        // If fetch_case_info fails, fall back to direct download
        console.warn("[DOWNLOAD] fetch_case_info failed, falling back to direct download:", e);
        progressContainer.classList.add("hidden");
        startDownload(caseId);
      });
    }

    // Duplicate check (for single case)
    var knownCaseIds = getKnownCaseIds();
    if (knownCaseIds.indexOf(caseId) !== -1) {
      showConfirmModal(
        "Case " + caseId + " is already in your library.\nDownload again? (This will overwrite it.)",
        "Download Again",
        proceedWithDownload
      );
    } else {
      proceedWithDownload();
    }
  });

  function updateCase(caseId) {
    if (downloadInProgress) {
      statusMsg.textContent = "A download is already in progress.";
      return;
    }

    showUpdateModal(
      "What was updated for this case?",
      "Script/dialog only",
      "Re-download all assets",
      function (choice) {
        startUpdate(caseId, choice === 2);
      }
    );
  }

  function startUpdate(caseId, redownloadAssets, onDone) {
    console.log("[UPDATE] startUpdate caseId=" + caseId + " redownloadAssets=" + redownloadAssets);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    // Show progress
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = redownloadAssets
      ? "Updating case (full)..."
      : "Updating case (script only)...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = createDownloadProgressHandler({
      finishedLabel: "Update complete!",
      logPrefix: "[UPDATE EVENT]"
    });

    invoke("update_case", {
      caseId: caseId,
      redownloadAssets: redownloadAssets,
      onEvent: onEvent
    })
      .then(function (manifest) {
        console.log("[UPDATE] update_case success:", JSON.stringify({
          case_id: manifest.case_id,
          title: manifest.title,
          total_downloaded: manifest.assets.total_downloaded,
          failed_count: manifest.failed_assets ? manifest.failed_assets.length : 0
        }));
        downloadResult.innerHTML =
          '<strong>' + escapeHtml(manifest.title) + '</strong> updated!';
        downloadResult.className = "result-success";
        if (!onDone) loadLibrary(); // Skip when called from batch (e.g., Update All)
      })
      .catch(function (e) {
        console.error("[UPDATE] update_case error:", e);
        downloadResult.textContent = "Update error: " + e;
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        caseIdInput.disabled = false;
        cancelDownloadBtn.classList.add("hidden");
        if (onDone) {
          onDone();
        } else {
          setTimeout(function () {
            progressContainer.classList.add("hidden");
          }, 4000);
        }
        processQueue();
      });
  }

  function retryCase(caseId, failedAssets) {
    if (downloadInProgress) {
      statusMsg.textContent = "A download is already in progress.";
      return;
    }

    var aaoCount = 0;
    var externalCount = 0;
    if (failedAssets) {
      for (var i = 0; i < failedAssets.length; i++) {
        if (failedAssets[i].url && failedAssets[i].url.indexOf("aaonline.fr") !== -1) {
          aaoCount++;
        } else {
          externalCount++;
        }
      }
    }

    var msg = "Retry " + (failedAssets ? failedAssets.length : "") + " failed assets?\n\n";
    if (aaoCount > 0 && externalCount > 0) {
      msg += aaoCount + " failed from aaonline.fr — the site was probably temporarily down. " +
        "Retrying these should work.\n\n" +
        externalCount + " failed from external hosting — these are likely dead links " +
        "(the author's hosting is down or the files were removed). " +
        "Retrying will most likely fail again.\n\n";
    } else if (aaoCount > 0) {
      msg += "All failed assets are from aaonline.fr — the site was probably temporarily down. " +
        "Retrying should work.\n\n";
    } else {
      msg += "Failed assets are from external hosting — these are likely dead links " +
        "(the author's hosting is down or the files were removed). " +
        "Retrying will most likely fail again.\n\n";
    }
    msg += "Only previously failed assets will be retried — nothing already downloaded " +
      "will be re-downloaded.";

    showConfirmModal(msg, "Retry", function () {
    console.log("[RETRY] retryCase caseId=" + caseId);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    // Show progress
    progressContainer.classList.remove("hidden");
    progressPhase.textContent = "Retrying failed assets...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = createDownloadProgressHandler({
      startedLabel: "Retrying",
      finishedLabel: "Retry complete!",
      failedLabel: "still failed",
      logPrefix: "[RETRY EVENT]"
    });

    invoke("retry_failed_assets", { caseId: caseId, onEvent: onEvent })
      .then(function (manifest) {
        console.log("[RETRY] retry_failed_assets success:", JSON.stringify({
          case_id: manifest.case_id,
          total_downloaded: manifest.assets.total_downloaded,
          still_failed: manifest.failed_assets ? manifest.failed_assets.length : 0
        }));
        var stillFailed = manifest.failed_assets ? manifest.failed_assets.length : 0;
        if (stillFailed === 0) {
          downloadResult.textContent = "All assets downloaded successfully!";
          downloadResult.className = "result-success";
        } else {
          downloadResult.textContent = stillFailed + " asset(s) still failed (server may be down).";
          downloadResult.className = "result-error";
        }
        loadLibrary();
      })
      .catch(function (e) {
        console.error("[RETRY] retry_failed_assets error:", e);
        downloadResult.textContent = "Retry error: " + e;
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        setTimeout(function () {
          progressContainer.classList.add("hidden");
        }, 4000);
        processQueue();
      });
    });
  }

  function startSequenceDownload(caseIds, sequenceTitle) {
    if (downloadInProgress) {
      downloadQueue.push({ type: "sequence", caseIds: caseIds, sequenceTitle: sequenceTitle });
      statusMsg.textContent = "Queued for download (" + downloadQueue.length + " in queue).";
      return;
    }
    console.log("[DOWNLOAD] startSequenceDownload ids=" + JSON.stringify(caseIds) + " title=" + sequenceTitle);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    progressContainer.classList.remove("hidden");
    cancelDownloadBtn.classList.remove("hidden");
    progressPhase.textContent = 'Downloading "' + sequenceTitle + '" (' + caseIds.length + " parts)...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = createDownloadProgressHandler({
      finishedLabel: "Sequence download complete!",
      logPrefix: "[SEQUENCE EVENT]",
      onSequenceProgress: function (data) {
        progressPhase.textContent = "Part " + data.current_part + "/" + data.total_parts + ": " + data.part_title;
        progressBarInner.style.width = "0%";
        progressText.textContent = "";
      }
    });

    invoke("download_sequence", { caseIds: caseIds, onEvent: onEvent })
      .then(function (manifests) {
        console.log("[DOWNLOAD] download_sequence success: " + manifests.length + " parts");
        downloadResult.innerHTML =
          '<strong>' + escapeHtml(sequenceTitle) + '</strong> (' +
          manifests.length + ' parts) &mdash; ready to play!';
        downloadResult.className = "result-success";
        caseIdInput.value = "";
        loadLibrary();
      })
      .catch(function (e) {
        console.error("[DOWNLOAD] download_sequence error:", e);
        downloadResult.textContent = "Error: " + e;
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        caseIdInput.disabled = false;
        cancelDownloadBtn.classList.add("hidden");
        setTimeout(function () {
          progressContainer.classList.add("hidden");
        }, 4000);
        processQueue();
      });
  }

  function startDownload(caseId) {
    if (downloadInProgress) {
      downloadQueue.push({ type: "single", caseId: caseId });
      statusMsg.textContent = "Queued for download (" + downloadQueue.length + " in queue).";
      return;
    }
    console.log("[DOWNLOAD] startDownload caseId=" + caseId);
    downloadInProgress = true;
    downloadBtn.disabled = true;
    caseIdInput.disabled = true;
    downloadResult.textContent = "";
    downloadResult.className = "";

    // Show progress
    progressContainer.classList.remove("hidden");
    cancelDownloadBtn.classList.remove("hidden");
    progressPhase.textContent = "Fetching case info...";
    progressBarInner.style.width = "0%";
    progressText.textContent = "";

    var onEvent = new Channel();
    onEvent.onmessage = createDownloadProgressHandler({
      finishedLabel: "Download complete!",
      logPrefix: "[DOWNLOAD EVENT]"
    });

    invoke("download_case", { caseId: caseId, onEvent: onEvent })
      .then(function (manifest) {
        console.log("[DOWNLOAD] download_case success:", JSON.stringify({
          case_id: manifest.case_id,
          title: manifest.title,
          total_downloaded: manifest.assets.total_downloaded,
          total_size: manifest.assets.total_size_bytes,
          failed_count: manifest.failed_assets ? manifest.failed_assets.length : 0
        }));
        downloadResult.innerHTML =
          '<strong>' + escapeHtml(manifest.title) + '</strong> by ' +
          escapeHtml(manifest.author) + ' &mdash; ready to play!';
        downloadResult.className = "result-success";
        caseIdInput.value = "";
        loadLibrary();
        // Scroll the new case into view after a brief delay
        setTimeout(function () {
          var card = document.querySelector('.case-card[data-case-id="' + manifest.case_id + '"]');
          if (card) {
            card.scrollIntoView({ behavior: "smooth", block: "nearest" });
            card.classList.add("case-card-highlight");
            setTimeout(function () { card.classList.remove("case-card-highlight"); }, 2000);
          }
        }, 200);
      })
      .catch(function (e) {
        console.error("[DOWNLOAD] download_case error:", e);
        var errMsg = String(e);
        // Make common errors more readable
        if (errMsg.indexOf("404") !== -1 || errMsg.indexOf("not found") !== -1) {
          downloadResult.textContent = "Case not found. Check the ID and try again.";
        } else if (errMsg.indexOf("timeout") !== -1 || errMsg.indexOf("connection") !== -1) {
          downloadResult.textContent = "Connection failed. Check your internet and try again.";
        } else {
          downloadResult.textContent = "Error: " + errMsg;
        }
        downloadResult.className = "result-error";
      })
      .finally(function () {
        downloadInProgress = false;
        downloadBtn.disabled = false;
        caseIdInput.disabled = false;
        cancelDownloadBtn.classList.add("hidden");
        setTimeout(function () {
          progressContainer.classList.add("hidden");
        }, 4000);
        processQueue();
      });
  }

  return {
    startDownload: startDownload,
    startUpdate: startUpdate,
    updateCase: updateCase,
    retryCase: retryCase,
    startSequenceDownload: startSequenceDownload,
    processQueue: processQueue,
    isDownloadInProgress: function () { return downloadInProgress; },
    progressContainer: progressContainer,
    progressPhase: progressPhase,
    progressBarInner: progressBarInner,
    progressText: progressText
  };
}
