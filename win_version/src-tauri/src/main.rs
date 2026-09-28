// A tray application has no console, and a release build must not flash one
// behind the notification-area icon.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    opencode_stats_win_lib::run();
}
