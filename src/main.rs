#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use vscode_rust::{app, icons, menubar};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cli_workspace: Option<PathBuf> = args.get(1).map(PathBuf::from);
    let mut cli_search: Option<String> = None;
    let mut cli_files: Vec<PathBuf> = Vec::new();
    let mut cli_no_welcome = false;
    let mut cli_scm = false;
    let mut i = 2usize;
    while i < args.len() {
        if args[i] == "--search" {
            cli_search = args.get(i + 1).cloned();
            i += 2;
        } else if args[i] == "--no-welcome" {
            cli_no_welcome = true;
            i += 1;
        } else if args[i] == "--scm" {
            cli_scm = true;
            i += 1;
        } else {
            cli_files.push(PathBuf::from(&args[i]));
            i += 1;
        }
    }

    // Install the native menubar BEFORE eframe takes over the event loop.
    // On macOS this calls `init_for_nsapp` which registers the menu globally.
    let installed_menu = menubar::install();
    let menu_ids = installed_menu.ids.clone();

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([720.0, 480.0])
        .with_title("vscode-rust")
        .with_title_shown(false)
        .with_titlebar_shown(false)
        .with_fullsize_content_view(true)
        .with_titlebar_buttons_shown(true)
        .with_icon(icons::app_icon());

    let native_options = eframe::NativeOptions {
        viewport,
        // Suppress winit's default macOS menu so muda's is the only one.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_default_menu(false);
        })),
        ..Default::default()
    };

    eframe::run_native(
        "vscode-rust",
        native_options,
        Box::new(move |cc| {
            let mut a = app::App::new(cc);
            a.attach_menu_ids(menu_ids);
            if let Some(ws) = cli_workspace.clone() {
                a.bootstrap_workspace(ws);
            }
            for f in cli_files {
                a.bootstrap_open_file(f);
            }
            if let Some(q) = cli_search.clone() {
                a.bootstrap_search(q);
            }
            if cli_no_welcome {
                a.bootstrap_hide_welcome();
            }
            if cli_scm {
                a.bootstrap_scm();
            }
            // Keep the InstalledMenu alive for the duration of the app.
            std::mem::forget(installed_menu);
            Ok(Box::new(a))
        }),
    )
}
