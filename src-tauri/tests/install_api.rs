use clawpal::install::types::InstallSession;

#[test]
fn install_session_serialization_roundtrip() {
    let json = r#"{
        "id": "sess-1",
        "method": "local",
        "state": "idle",
        "current_step": null,
        "logs": [],
        "artifacts": {},
        "created_at": "2026-02-24T00:00:00Z",
        "updated_at": "2026-02-24T00:00:00Z"
    }"#;

    let parsed: InstallSession = serde_json::from_str(json).expect("session json should deserialize");
    assert_eq!(parsed.method.as_str(), "local");
    assert_eq!(parsed.state.as_str(), "idle");
}
