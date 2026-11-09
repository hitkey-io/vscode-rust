//! Runs the sample extension in tests/fixtures/extensions through the real
//! Node.js extension host. Skipped (passes) when Node.js isn't installed.

use std::path::PathBuf;
use std::time::Duration;

use vscode_rust::extensions::{host, Activation, ExtensionService, Severity};

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
