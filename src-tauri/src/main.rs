// Real code lives in lib.rs so tests and future mobile targets can pull it in.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used)]

fn main() {
    queryben_lib::run();
}
