use std::fs;
use std::sync::Mutex;

use clawpal_core::connect::connect_docker;
use clawpal_core::install::{install_docker, DockerInstallOptions};
use clawpal_core::instance::{InstanceRegistry, InstanceType, SshHostConfig};
use clawpal_core::ssh::registry as ssh_registry;
use uuid::Uuid;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

#[test]
fn install_docker_dry_run_registers_docker_instance() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let data_dir = temp_dir("clawpal-core-workflow-data");
    let home_dir = temp_dir("clawpal-core-workflow-home");
    std::env::set_var("CLAWPAL_DATA_DIR", &data_dir);

    let result = install_docker(DockerInstallOptions {
        home: Some(home_dir.to_string_lossy().to_string()),
        label: Some("Docker Workflow".to_string()),
        dry_run: true,
    })
    .expect("install docker dry-run should succeed");

    assert!(result.ok);
    assert_eq!(result.instance_id.as_deref(), Some("docker:local"));

    let registry = InstanceRegistry::load().expect("load registry");
    let instance = registry
        .get("docker:local")
        .expect("docker instance should be saved");
    assert!(matches!(instance.instance_type, InstanceType::Docker));
    assert_eq!(instance.label, "Docker Workflow");
}

#[tokio::test]
async fn connect_docker_registers_instance() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let data_dir = temp_dir("clawpal-core-connect-data");
    let home_dir = temp_dir("clawpal-core-connect-home");
    std::env::set_var("CLAWPAL_DATA_DIR", &data_dir);

    let instance = connect_docker(home_dir.to_string_lossy().as_ref(), Some("Connect Docker"))
        .await
        .expect("connect docker should succeed");

    assert!(matches!(instance.instance_type, InstanceType::Docker));
    assert_eq!(instance.label, "Connect Docker");

    let registry = InstanceRegistry::load().expect("load registry");
    assert!(registry.get(&instance.id).is_some());
}

#[test]
fn ssh_registry_roundtrip_via_instance_registry() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let data_dir = temp_dir("clawpal-core-ssh-registry-data");
    std::env::set_var("CLAWPAL_DATA_DIR", &data_dir);

    let host = SshHostConfig {
        id: "ssh:workflow-vm1".to_string(),
        label: "Workflow VM1".to_string(),
        host: "vm1".to_string(),
        port: 22,
        username: "root".to_string(),
        auth_method: "key".to_string(),
        key_path: None,
        password: None,
    };

    ssh_registry::upsert_ssh_host(host.clone()).expect("upsert ssh host");
    let listed = ssh_registry::list_ssh_hosts().expect("list ssh hosts");
    assert!(listed.iter().any(|h| h.id == host.id));

    let removed = ssh_registry::delete_ssh_host(&host.id).expect("delete ssh host");
    assert!(removed);
}
