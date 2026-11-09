//! Runs the sample extension in tests/fixtures/extensions through the real
//! Node.js extension host. Skipped (passes) when Node.js isn't installed.

use std::path::PathBuf;
use std::time::Duration;

use vscode_rust::extensions::{host, Activation, EditorRequest, ExtensionService, Severity};

const ID: &str = "sample.hello-world";
const WAIT: Duration = Duration::from_secs(10);

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn started() -> Option<ExtensionService> {
    host::find_node()?;
    let mut svc = ExtensionService::default();
    svc.load(&[fixtures().join("extensions")]);
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
