use clap::{Parser, Subcommand};
use clawpal_core::instance::InstanceRegistry;
use serde_json::json;

#[derive(Parser, Debug)]
#[command(name = "clawpal")]
#[command(about = "ClawPal CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Instance {
        #[command(subcommand)]
        command: InstanceCommands,
    },
    Install {
        #[command(subcommand)]
        command: InstallCommands,
    },
    Connect {
        #[command(subcommand)]
        command: ConnectCommands,
    },
    Health {
        #[command(subcommand)]
        command: HealthCommands,
    },
    Ssh {
        #[command(subcommand)]
        command: SshCommands,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
}

#[derive(Subcommand, Debug)]
enum InstanceCommands {
    List,
    Remove { id: String },
}

#[derive(Subcommand, Debug)]
enum InstallCommands {
    Docker,
    Local,
}

#[derive(Subcommand, Debug)]
enum ConnectCommands {
    Docker,
    Ssh,
}

#[derive(Subcommand, Debug)]
enum HealthCommands {
    Check { id: Option<String> },
}

#[derive(Subcommand, Debug)]
enum SshCommands {
    Connect { host_id: String },
    Disconnect { host_id: String },
    List,
}

#[derive(Subcommand, Debug)]
enum ProfileCommands {
    List,
    Add,
    Remove { id: String },
    Test { id: String },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Instance { command } => run_instance_command(command),
        command => Ok(json!({
            "status": "not yet implemented",
            "command": format!("{command:?}"),
        })),
    };

    match result {
        Ok(value) => println!("{value}"),
        Err(message) => {
            println!("{}", json!({ "error": message }));
            std::process::exit(1);
        }
    }
}

fn run_instance_command(command: InstanceCommands) -> Result<serde_json::Value, String> {
    match command {
        InstanceCommands::List => {
            let registry = InstanceRegistry::load().map_err(|e| e.to_string())?;
            Ok(json!(registry.list()))
        }
        InstanceCommands::Remove { id } => {
            let mut registry = InstanceRegistry::load().map_err(|e| e.to_string())?;
            let removed = registry.remove(&id).is_some();
            registry.save().map_err(|e| e.to_string())?;
            Ok(json!({ "removed": removed, "id": id }))
        }
    }
}
