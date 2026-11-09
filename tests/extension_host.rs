//! Runs the sample extension in tests/fixtures/extensions through the real
//! Node.js extension host. Skipped (passes) when Node.js isn't installed.

use std::path::PathBuf;
use std::time::Duration;

use vscode_rust::extensions::{host, Activation, EditorRequest, ExtensionService, LanguageFeature, ProviderResult, Severity};

const ID: &str = "sample.hello-world";
const WAIT: Duration = Duration::from_secs(10);

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// A folder holding only the checked-in sample extension, so whatever else
/// lands in `tests/fixtures/extensions` (an install made while trying the
/// app) never runs in tests.
fn sample_extensions_dir() -> PathBuf {
    let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extensions/sample.hello-world-0.0.1");
    let dir = std::env::temp_dir().join(format!("vscode-rust-sample-ext-{}", std::process::id()));
    let link = dir.join("sample.hello-world-0.0.1");
    if !link.exists() {
        std::fs::create_dir_all(&dir).unwrap();
        // Tests run in parallel: another one may have just made it.
        if let Err(e) = std::os::unix::fs::symlink(&sample, &link) {
            assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists, "{e}");
        }
    }
    dir
}

fn started() -> Option<ExtensionService> {
    host::find_node()?;
    let mut svc = ExtensionService::default();
    svc.load(&[sample_extensions_dir()]);
    svc.start(Some(&fixtures().join("workspace")), || {});
    assert!(
        svc.wait_until(WAIT, |s| s.activation.get(ID) == Some(&Activation::Active)),
        "extension never activated; log: {:#?}",
        svc.log
    );
    Some(svc)
}

#[test]
fn activates_on_startup_and_shows_a_status_bar_item() {
    let Some(mut svc) = started() else { return };
    assert!(svc.wait_until(WAIT, |s| !s.status_items.is_empty()), "log: {:#?}", svc.log);
    let item = svc.status_items.values().next().unwrap();
    assert_eq!(item.text, "$(smiley) Hello");
    assert!(item.right);
    assert_eq!(item.command.as_deref(), Some("helloWorld.sayHello"));
    assert!(svc.log.iter().any(|l| l.contains("hello-world activated")));
}

#[test]
fn command_reads_configuration_and_workspace_and_notifies() {
    let Some(mut svc) = started() else { return };
    svc.execute("helloWorld.sayHello", serde_json::json!([]));
    assert!(svc.wait_until(WAIT, |s| !s.notifications.is_empty()), "log: {:#?}", svc.log);
    let n = &svc.notifications[0];
    assert_eq!(n.severity, Severity::Info);
    assert_eq!(n.text, "Hello from workspace!");
    assert_eq!(n.source.as_deref(), Some("Hello World Sample"));
}

#[test]
fn message_buttons_answer_the_extension() {
    let Some(mut svc) = started() else { return };
    svc.execute("helloWorld.askChoice", serde_json::json!([]));
    assert!(svc.wait_until(WAIT, |s| !s.notifications.is_empty()));
    let n = svc.notifications[0].clone();
    assert_eq!(n.actions, vec!["Yes".to_string(), "No".to_string()]);
    svc.close_notification(n.id, Some(1));
    assert!(
        svc.wait_until(WAIT, |s| s.notifications.iter().any(|n| n.text == "You picked No")),
        "log: {:#?}",
        svc.log
    );
}

#[test]
fn failing_command_reports_an_error() {
    let Some(mut svc) = started() else { return };
    svc.execute("helloWorld.fail", serde_json::json!([]));
    assert!(svc.wait_until(WAIT, |s| s
        .notifications
        .iter()
        .any(|n| n.severity == Severity::Error && n.text.contains("boom"))));
}

fn active_doc() -> (PathBuf, String) {
    let path = fixtures().join("workspace/README.md");
    (path, "# Sample TODO site\nTODO: write docs\n".to_string())
}

#[test]
fn decorations_follow_the_active_editor() {
    let Some(mut svc) = started() else { return };
    let (path, text) = active_doc();
    svc.sync_documents(&[(path.clone(), "markdown", &text, false)], Some((path.clone(), [0, 0, 0, 0])));
    assert!(
        svc.wait_until(WAIT, |s| s.decorations.get(&path).is_some_and(|d| d.values().any(|r| r.len() == 2))),
        "log: {:#?}",
        svc.log
    );
    let ranges = svc.decorations[&path].values().next().unwrap().clone();
    assert_eq!(ranges[0].range, [0, 9, 0, 13]);
    assert_eq!(ranges[1].range, [1, 0, 1, 4]);
    let style = svc.decoration_types.values().next().unwrap();
    assert_eq!(style.background, Some(egui::Color32::from_rgb(0xff, 0xbd, 0x2a)));

    // Editing the document re-runs the extension's decorator.
    let edited = format!("{text}TODO again\n");
    svc.sync_documents(&[(path.clone(), "markdown", &edited, true)], Some((path.clone(), [0, 0, 0, 0])));
    assert!(svc.wait_until(WAIT, |s| s.decorations[&path].values().any(|r| r.len() == 3)));
}

#[test]
fn extension_edits_come_back_as_editor_requests() {
    let Some(mut svc) = started() else { return };
    let (path, text) = active_doc();
    svc.sync_documents(&[(path.clone(), "markdown", &text, false)], Some((path.clone(), [0, 0, 0, 0])));
    svc.execute("helloWorld.upperFirstLine", serde_json::json!([]));
    let mut edit = None;
    for _ in 0..200 {
        for r in svc.take_requests() {
            if let EditorRequest::ApplyEdits { request_id, path: p, edits } = r {
                edit = Some((request_id, p, edits));
            }
        }
        if edit.is_some() {
            break;
        }
        svc.wait_until(Duration::from_millis(50), |_| false);
    }
    let (request_id, p, edits) = edit.expect("an edit request");
    assert_eq!(p, path);
    let mut new_text = text.clone();
    vscode_rust::extensions::text::apply_edits(&mut new_text, &edits);
    assert!(new_text.starts_with("# SAMPLE TODO SITE\n"));
    svc.answer(request_id, serde_json::json!(true));
}

#[test]
fn quick_pick_and_input_box_answers_reach_the_extension() {
    let Some(mut svc) = started() else { return };
    svc.execute("helloWorld.pick", serde_json::json!([]));
    let mut answered = 0;
    for _ in 0..200 {
        for r in svc.take_requests() {
            match r {
                EditorRequest::QuickPick { request_id, items, placeholder, .. } => {
                    assert_eq!(items.len(), 3);
                    assert_eq!(placeholder.as_deref(), Some("Pick a fruit"));
                    svc.answer(request_id, serde_json::json!(1));
                    answered += 1;
                }
                EditorRequest::InputBox { request_id, value, prompt, .. } => {
                    assert_eq!(value, "Ada");
                    assert_eq!(prompt.as_deref(), Some("Your name"));
                    svc.answer(request_id, serde_json::json!("Grace"));
                    answered += 1;
                }
                _ => {}
            }
        }
        if svc.notifications.iter().any(|n| n.text == "Picked Banana for Grace") {
            break;
        }
        svc.wait_until(Duration::from_millis(50), |_| false);
    }
    assert_eq!(answered, 2);
    assert!(svc.notifications.iter().any(|n| n.text == "Picked Banana for Grace"), "log: {:#?}", svc.log);
}

/// Prints how much of vscode.d.ts the host implements (run with
/// `--nocapture` to read it) and keeps the core namespaces from regressing.
#[test]
fn api_coverage_report() {
    let Some(mut svc) = started() else { return };
    svc.request_api_coverage();
    assert!(svc.wait_until(WAIT, |s| s.api_coverage.is_some()));
    let cov = svc.api_coverage.clone().unwrap();
    let mut total = 0;
    let mut real = 0;
    let mut ns: Vec<_> = cov["namespaces"].as_object().unwrap().iter().collect();
    ns.sort_by_key(|(k, _)| k.to_string());
    for (name, v) in ns {
        let t = v["total"].as_u64().unwrap();
        let stubbed: Vec<&str> = v["stubbed"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
        total += t;
        real += t - stubbed.len() as u64;
        println!("COVERAGE vscode.{name}: {}/{t} implemented; stubs: {}", t - stubbed.len() as u64, stubbed.join(", "));
    }
    let ct = cov["classes"]["total"].as_u64().unwrap();
    let cs: Vec<&str> = cov["classes"]["stubbed"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    println!("COVERAGE functions/values: {real}/{total}; classes: {}/{ct} (stubs: {}); enums: all {}", ct - cs.len() as u64, cs.join(", "), cov["enums"]["total"]);
    for core in ["commands", "workspace", "window"] {
        let stubbed = cov["namespaces"][core]["stubbed"].as_array().unwrap();
        assert!(
            !stubbed.iter().any(|m| m == "registerCommand" || m == "getConfiguration" || m == "showInformationMessage"),
            "core API regressed in {core}"
        );
    }
}

fn ask(svc: &mut ExtensionService, kind: LanguageFeature, path: &std::path::Path, line: usize, ch: usize) -> ProviderResult {
    let id = svc.request_language_feature(kind, path, line, ch, None).expect("a provider");
    let mut result = None;
    for _ in 0..200 {
        if let Some(r) = svc.take_result(id) {
            result = Some(r);
            break;
        }
        svc.wait_until(Duration::from_millis(30), |_| false);
    }
    result.expect("an answer")
}

#[test]
fn language_features_answer_through_the_host() {
    let Some(mut svc) = started() else { return };
    let path = fixtures().join("workspace/README.md");
    let text = "TODO: see package\nFIXME later\nBUG here\n";
    svc.sync_documents(&[(path.clone(), "markdown", text, false)], Some((path.clone(), [0, 0, 0, 0])));

    // Diagnostics arrive on open: one warning, one error.
    assert!(svc.wait_until(WAIT, |s| s.diagnostics_for(&path).len() == 2), "log: {:#?}", svc.log);
    assert_eq!(svc.problem_counts(), (1, 1));
    let warn = svc.diagnostics_for(&path).into_iter().find(|d| d.severity == 1).unwrap().clone();
    assert_eq!(warn.range, [1, 0, 1, 5]);
    assert_eq!(warn.message, "FIXME found");

    match ask(&mut svc, LanguageFeature::Completion, &path, 2, 3) {
        ProviderResult::Completion { items, .. } => {
            let f = items.iter().find(|i| i.label == "helloWorld").expect("helloWorld");
            assert_eq!(f.kind, "Function");
            assert_eq!(f.insert_text, "helloWorld(name)");
            assert_eq!(f.range, [2, 0, 2, 3]); // the word "BUG" up to the caret
            assert_eq!(f.documentation.as_deref(), Some("Inserts a **greeting**."));
        }
        other => panic!("{other:?}"),
    }
    match ask(&mut svc, LanguageFeature::Hover, &path, 0, 1) {
        ProviderResult::Hover { contents, range } => {
            assert_eq!(contents, vec!["**TODO** marks unfinished work".to_string()]);
            assert_eq!(range, Some([0, 0, 0, 4]));
        }
        other => panic!("{other:?}"),
    }
    match ask(&mut svc, LanguageFeature::Definition, &path, 0, 12) {
        ProviderResult::Definition(locs) => {
            assert_eq!(locs.len(), 1);
            assert!(locs[0].path.ends_with("package.json"));
            assert_eq!(locs[0].range, [1, 2, 1, 2]);
        }
        other => panic!("{other:?}"),
    }

    // Fixing the text clears the problems.
    svc.sync_documents(&[(path.clone(), "markdown", "all good\n", true)], Some((path.clone(), [0, 0, 0, 0])));
    assert!(svc.wait_until(WAIT, |s| s.diagnostics_for(&path).is_empty()));
}
