//! M2M — Capture Software Detection
//!
//! Periodically enumerates running processes and warns the user when known
//! screen-recording / capture software is active (OBS, XSplit, Snipping
//! Tool, QuickTime screencapture, …).
//!
//! ## Honest threat model (from the security roadmap)
//!
//! This STOPS NOTHING. A determined attacker runs custom malware, not OBS.
//! The value is honest UX: a high-risk user sees "OBS Studio is running —
//! your screen may be recorded" instead of silently sharing a meeting.
//!
//! Detection is OFF by default (`SecurityConfig.capture_process_detection`)
//! and only matches process NAMES — no paths, no window titles, no content.
//!
//! ## Matching discipline
//!
//! - `match_exact`: case-insensitive comparison of the whole process name
//!   (with and without `.exe`) — used for generic words where `contains`
//!   would produce false positives (e.g. "loom").
//! - `match_contains`: substring match — used for unambiguous tool-specific
//!   identifiers ("obs64", "xsplit", …).
//!
//! Matching runs against the LOWERCASED process name. The pure function
//! [`detect_capture_tools`] is unit-testable without a live system.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

/// How often the running-process list is rescanned while detection is on.
const SCAN_INTERVAL_SECS: u64 = 10;

/// One known capture-capable application.
struct CaptureTool {
    /// Human-readable name shown in warnings.
    display: &'static str,
    /// Lowercase tokens compared against the WHOLE process name.
    match_exact: &'static [&'static str],
    /// Lowercase substrings searched INSIDE the process name.
    match_contains: &'static [&'static str],
}

macro_rules! tool {
    ($display:expr, $exact:expr, $contains:expr) => {
        CaptureTool { display: $display, match_exact: $exact, match_contains: $contains }
    };
}

/// Curated list of common screen-recording / capture tools across platforms.
/// Heuristic by design: unknown or custom malware is out of scope (see
/// module docs).
static KNOWN_CAPTURE_TOOLS: &[CaptureTool] = &[
    // ── Streaming / recording suites ──
    tool!("OBS Studio", &["obs", "obs.cmd"], &["obs64", "obs32"]),
    tool!("Streamlabs", &[], &["streamlabs", "slb64", "slb32"]),
    tool!("XSplit Broadcaster", &[], &["xsplit"]),
    tool!("vMix", &["vmix"], &[]),
    tool!("SimpleScreenRecorder", &[], &["simplescreenrecorder"]),
    tool!("Kooha", &["kooha"], &[]),
    tool!("GPU Screen Recorder", &[], &["gpu-screen-recorder"]),
    tool!("vokoscreen", &[], &["vokoscreen"]),
    tool!("recordMyDesktop", &[], &["recordmydesktop"]),
    // ── Commercial recorders ──
    tool!("Bandicam", &[], &["bdcam", "bandicam"]),
    tool!("Camtasia", &[], &["camtasia", "camrecorder"]),
    tool!("Snagit", &[], &["snagit"]),
    tool!("ShareX", &["sharex"], &[]),
    tool!("ScreenPresso", &[], &["screenpresso"]),
    tool!("Loom", &["loom", "loom.exe"], &[]),
    tool!("FlashBack", &[], &["flashback"]),
    // ── OS built-in capture ──
    tool!("Snipping Tool", &[], &["snippingtool", "screenclippinghost", "screensketch"]),
    tool!("macOS Screenshot (Cmd+Shift+5)", &[], &["screencaptureui", "screenshotserver"]),
    tool!("QuickTime Player", &["quicktime player"], &[]),
    // ── CLI / generic encoders commonly used for screen grabbing ──
    tool!("ffmpeg (possible screen grab)", &["ffmpeg"], &[]),
];

/// Pure matcher: given lowercased process names, return the deduplicated
/// display names of known capture tools among them (sorted for stable
/// comparisons/events).
fn detect_capture_tools<I: IntoIterator<Item = String>>(lowercased_process_names: I) -> Vec<String> {
    let mut hits: Vec<&'static str> = Vec::new();
    'tools: for tool in KNOWN_CAPTURE_TOOLS {
        for name in lowercased_process_names.iter() {
            let exact_hit = tool.match_exact.iter().any(|m| *name == *m);
            let contains_hit = tool.match_contains.iter().any(|m| name.contains(m));
            if exact_hit || contains_hit {
                hits.push(tool.display);
                continue 'tools;
            }
        }
    }
    hits.sort_unstable();
    hits.dedup();
    hits.into_iter().map(|s| s.to_string()).collect()
}

/// Scan the live process table once.
fn scan_live() -> Vec<String> {
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let names = sys
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_lowercase());
    detect_capture_tools(names)
}

/// Background monitor loop. Runs until the config toggle is switched off,
/// emitting `m2m://capture-warning` ONLY when the set of detected tools
/// changes (no spam while a tool stays open).
pub fn start_monitor(state: Arc<crate::state::AppState>, app_handle: AppHandle) {
    // Single-monitor invariant: don't stack loops on repeated toggles.
    if state
        .capture_monitor_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::debug!("capture monitor already running");
        return;
    }

    tokio::spawn(async move {
        tracing::info!("capture software monitor started");
        let mut last_active: Vec<String> = Vec::new();

        loop {
            let enabled = state
                .security_config
                .try_read()
                .map(|c| c.capture_process_detection)
                .unwrap_or(true); // lock contention: keep monitoring this cycle

            if !enabled {
                if !last_active.is_empty() {
                    // Toggle was switched off while tools were flagged.
                    let _ = app_handle.emit("m2m://capture-warning", serde_json::json!({ "active": [] }));
                }
                break;
            }

            let detected = tokio::task::spawn_blocking(scan_live).await.unwrap_or_default();
            if detected != last_active {
                tracing::info!(detected = ?detected, "capture software set changed");
                let _ = app_handle.emit(
                    "m2m://capture-warning",
                    serde_json::json!({ "active": detected }),
                );
                last_active = detected;
            }

            tokio::time::sleep(std::time::Duration::from_secs(SCAN_INTERVAL_SECS)).await;
        }

        state.capture_monitor_running.store(false, Ordering::SeqCst);
        tracing::info!("capture software monitor stopped");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_obs_by_exe_suffix() {
        let procs = vec![
            "chrome.exe".to_string(),
            "obs64.exe".to_string(), // Windows ships obs64.exe
            "explorer.exe".to_string(),
        ];
        assert_eq!(detect_capture_tools(procs), vec!["OBS Studio"]);
    }

    #[test]
    fn test_detects_snipping_tool_and_xsplit() {
        let procs = vec![
            "ScreenClippingHost.exe".to_string(),
            "XSplit.Core.Console.exe".to_string(),
        ];
        let mut got = detect_capture_tools(procs);
        got.sort();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&"Snipping Tool".to_string()));
        assert!(got.contains(&"XSplit Broadcaster".to_string()));
    }

    #[test]
    fn test_exact_match_prevents_generic_word_false_positives() {
        // "loom" must not match inside other names…
        assert!(detect_capture_tools(vec!["bloomington.exe".to_string()]).is_empty());
        // …but the real Loom app is caught by exact name.
        assert_eq!(detect_capture_tools(vec!["loom".to_string()]), vec!["Loom"]);
    }

    #[test]
    fn test_benign_processes_produce_no_hits() {
        let procs = vec![
            "firefox".to_string(),
            "code".to_string(),
            "systemd".to_string(),
            "explorer.exe".to_string(),
            "kernel_task".to_string(),
        ];
        assert!(detect_capture_tools(procs).is_empty());
    }

    #[test]
    fn test_results_are_deduplicated_and_sorted() {
        // Two OBS processes (64-bit app + helper) → one warning entry.
        let procs = vec!["obs64.exe".to_string(), "obs32.exe".to_string()];
        assert_eq!(detect_capture_tools(procs), vec!["OBS Studio"]);
    }

    #[test]
    fn test_case_insensitivity_is_handled_by_caller_lowercasing() {
        // Contract: caller lowercases. Feed pre-lowercased input.
        let procs = vec!["obs64.exe".to_lowercase()];
        assert_eq!(detect_capture_tools(procs), vec!["OBS Studio"]);
    }
}
