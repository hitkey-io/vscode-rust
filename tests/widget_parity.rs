//! Visual-parity test harness for the `vscode_widgets` library.
//!
//! For every `(widget, state)` pair declared in
//! `vscode_widgets::testing::catalogue::STORIES`, this harness:
//!
//! 1. Spins up an in-process `egui_kittest::Harness` sized to the state's
//!    suggested canvas, pinned to 2.0 DPR to match Retina screencapture.
//! 2. Renders the state on top of `Palette::EDITOR_BG`.
//! 3. Writes the snapshot to
//!    `../test-artifacts/snapshots/widgets/<widget>/<state>.png` (the
//!    location is configured by `kittest.toml`).
//! 4. Compares to the upstream baseline at
//!    `../test-artifacts/parity-reports/widgets/vscode-elements-baselines/<widget>/<state>.png`
//!    (captured manually — see
//!    `../test-artifacts/parity-reports/widgets/README.md`).
//!
//! Commands:
//!
//! ```text
//! UPDATE_WIDGET_BASELINES=1 cargo test --test widget_parity
//! cargo test --test widget_parity -- --nocapture
//! ```
//!
//! For the parity comparison itself, we lean on `egui_kittest`'s built-in
//! snapshot diff: it writes `<name>.diff.png` heatmaps next to mismatches and
//! fails the test. Phase 0 only ships the harness skeleton — until Phase 1
//! lands at least one widget, the test below is a no-op that succeeds.

use egui_kittest::{Harness, SnapshotResults};
use vscode_rust::theme;
use vscode_rust::vscode_widgets::testing::STORIES;

fn setup(ctx: &egui::Context) {
    vscode_rust::icons::register_fonts(ctx);
    theme::apply(ctx);
}

#[test]
fn widget_catalogue_renders_without_panics() {
    if STORIES.is_empty() {
        return;
    }

    let mut results = SnapshotResults::new();

    for story in STORIES {
        for state in story.states {
            let mut initialized = false;
            let draw_fn = state.draw;
            let runner = move |ctx: &egui::Context| {
                if !initialized {
                    setup(ctx);
                    initialized = true;
                    return;
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme::Palette::EDITOR_BG))
                    .show(ctx, |ui| {
                        draw_fn(ui);
                    });
            };
            let mut harness = Harness::builder()
                .with_size(egui::Vec2::new(state.size.0, state.size.1))
                .with_pixels_per_point(2.0)
                .wgpu()
                .build(runner);
            harness.run_steps(3);
            harness.snapshot(&format!("widgets/{}/{}", story.widget, state.name));
            results.extend_harness(&mut harness);
        }
    }
}
