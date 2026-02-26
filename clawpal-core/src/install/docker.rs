use crate::install::{DockerInstallOptions, InstallError, Result, StepResult};

pub fn pull(options: &DockerInstallOptions) -> Result<StepResult> {
    run_step(
        "docker_pull",
        "docker compose pull",
        options.dry_run || command_exists("docker"),
        options.dry_run,
    )
}

pub fn configure(options: &DockerInstallOptions) -> Result<StepResult> {
    run_step(
        "docker_configure",
        "write docker env/config",
        true,
        options.dry_run,
    )
}

pub fn up(options: &DockerInstallOptions) -> Result<StepResult> {
    run_step(
        "docker_up",
        "docker compose up -d",
        options.dry_run || command_exists("docker"),
        options.dry_run,
    )
}

fn run_step(step: &str, detail: &str, ok: bool, dry_run: bool) -> Result<StepResult> {
    if dry_run {
        return Ok(StepResult {
            step: step.to_string(),
            ok: true,
            detail: format!("dry-run: {detail}"),
        });
    }
    if !ok {
        return Err(InstallError::Step(format!("{step} failed: {detail}")));
    }
    Ok(StepResult {
        step: step.to_string(),
        ok: true,
        detail: detail.to_string(),
    })
}

fn command_exists(name: &str) -> bool {
    std::process::Command::new("bash")
        .args(["-lc", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_returns_step_result_on_dry_run() {
        let options = DockerInstallOptions {
            dry_run: true,
            ..DockerInstallOptions::default()
        };
        let result = pull(&options).expect("pull");
        assert!(result.ok);
    }

    #[test]
    fn configure_returns_step_result_on_dry_run() {
        let options = DockerInstallOptions {
            dry_run: true,
            ..DockerInstallOptions::default()
        };
        let result = configure(&options).expect("configure");
        assert!(result.ok);
    }

    #[test]
    fn up_returns_step_result_on_dry_run() {
        let options = DockerInstallOptions {
            dry_run: true,
            ..DockerInstallOptions::default()
        };
        let result = up(&options).expect("up");
        assert!(result.ok);
    }
}
