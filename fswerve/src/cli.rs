use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(
    name = "fswerve",
    about = "CLI client for the swerve encrypted file staging server",
    long_about = "fswerve is the command-line interface for managing files on a remote swerve server.\n\n\
                   Upload files, control which files are served, manage swerve socket bindings,\n\
                   and download files — all through the authenticated management API.\n\n\
                   Start by configuring your server connection:\n  fswerve config set --server-url http://host:9740 --api-key YOUR_KEY",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output format as JSON instead of human-readable text
    #[arg(long, global = true, help = "Output as JSON (useful for scripting)")]
    pub json: bool,

    /// Suppress all non-error output
    #[arg(long, global = true, help = "Suppress non-error output")]
    pub quiet: bool,

    /// Show verbose output including request details
    #[arg(long, global = true, help = "Show verbose output")]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long, global = true, env = "NO_COLOR", help = "Disable colored output")]
    pub no_color: bool,

    /// Override server URL from config
    #[arg(long, global = true, env = "FSWERVE_SERVER_URL", help = "Override server URL")]
    pub server_url: Option<String>,

    /// Override API key from config
    #[arg(long, global = true, env = "FSWERVE_API_KEY", help = "Override API key")]
    pub api_key: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configure the server connection (URL and API key)
    ///
    /// Settings are stored in ~/.fswerve/config.toml
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Upload a local file to the swerve server
    ///
    /// The file is encrypted server-side and stored with a hashed filename.
    /// Re-uploading a file with the same name overwrites the existing one.
    ///
    /// Example: fswerve upload ./payload.bin --serve-name update.exe
    Upload(UploadArgs),

    /// List all files currently stored on the server
    ///
    /// Shows real name, serve name, serving state, and file size for each file.
    ///
    /// Example: fswerve files
    /// Example: fswerve files --json
    Files,

    /// Download a file from the server via the management API
    ///
    /// Downloads are always available through the management port regardless
    /// of the file's serving state. The file is decrypted server-side before transfer.
    ///
    /// Example: fswerve download payload.bin -o ./local_copy.bin
    Download(DownloadArgs),

    /// Permanently delete a file from the server
    ///
    /// Removes both the encrypted file from storage and all associated metadata.
    /// This action cannot be undone. Prompts for confirmation unless --yes is provided.
    ///
    /// Example: fswerve destroy payload.bin --yes
    #[command(alias = "delete", alias = "rm")]
    Destroy(DestroyArgs),

    /// Control file serving state and serve names
    ///
    /// Enable or disable serving for individual files, or change the filename
    /// that appears when the file is served through swerve sockets.
    ///
    /// Example: fswerve serve enable payload.bin
    #[command(subcommand)]
    Serve(ServeCommands),

    /// Manage swerve socket bindings (network interfaces for file serving)
    ///
    /// Swerve sockets are HTTP listeners that serve enabled files under their
    /// configured serve names. Create bindings on specific interface:port pairs
    /// to control where files are accessible.
    ///
    /// Example: fswerve sockets list
    #[command(subcommand)]
    Sockets(SocketCommands),

    /// Check connectivity to the swerve server
    ///
    /// Verifies that the configured server URL is reachable and the API key is valid.
    ///
    /// Example: fswerve status
    Status,

    /// Generate shell completions
    ///
    /// Example: fswerve completions bash > ~/.bash_completion.d/fswerve
    Completions(CompletionsArgs),
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Set the swerve server URL and API key
    ///
    /// Example: fswerve config set --server-url http://10.0.0.5:9740 --api-key mySecretKey123
    Set(ConfigSetArgs),

    /// Display the current configuration
    ///
    /// Example: fswerve config show
    Show,
}

#[derive(Args, Debug)]
pub struct ConfigSetArgs {
    /// The swerve server management URL
    #[arg(short, long, value_name = "URL", help = "Server management URL (e.g., http://10.0.0.5:9740)")]
    pub server_url: String,

    /// The API key for authenticating with the swerve server
    #[arg(short, long, value_name = "KEY", help = "API key for server authentication")]
    pub api_key: String,
}

#[derive(Args, Debug)]
pub struct UploadArgs {
    /// Path to the local file to upload
    #[arg(value_name = "FILE", help = "Local file path to upload (e.g., ./payload.bin)")]
    pub file: String,

    /// The filename to use when serving this file through swerve sockets.
    /// If omitted, the original filename is used as the serve name.
    #[arg(long, value_name = "NAME", help = "Spoofed filename for serving (defaults to real filename)")]
    pub serve_name: Option<String>,
}

#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// The real (original) name of the file to download
    #[arg(value_name = "FILE", help = "Real filename as it was uploaded (e.g., payload.bin)")]
    pub real_name: String,

    /// Output path for the downloaded file
    #[arg(short, long, value_name = "PATH", help = "Output file path (defaults to ./<real_name>)")]
    pub output: Option<String>,
}

#[derive(Args, Debug)]
pub struct DestroyArgs {
    /// The real (original) name of the file to delete
    #[arg(value_name = "FILE", help = "Real filename to permanently delete from the server")]
    pub real_name: String,

    /// Skip confirmation prompt
    #[arg(short, long, help = "Skip confirmation prompt")]
    pub yes: bool,
}

#[derive(Subcommand, Debug)]
pub enum ServeCommands {
    /// Enable serving for a file (make it accessible on swerve sockets)
    ///
    /// The file will be served under its configured serve name on all active
    /// swerve socket bindings.
    ///
    /// Example: fswerve serve enable payload.bin
    Enable(ServeTargetArgs),

    /// Disable serving for a file (stop serving it on swerve sockets)
    ///
    /// The file remains stored on the server and can still be downloaded
    /// via the management API.
    ///
    /// Example: fswerve serve disable payload.bin
    Disable(ServeTargetArgs),

    /// Change the serve name (spoofed filename) for a file
    ///
    /// This changes how the file appears when accessed through swerve sockets.
    /// The real filename and storage are not affected.
    ///
    /// Example: fswerve serve set-name payload.bin --serve-name installer.exe
    #[command(alias = "rename")]
    SetName(ServeRenameArgs),
}

#[derive(Args, Debug)]
pub struct ServeTargetArgs {
    /// The real (original) name of the file
    #[arg(value_name = "FILE", help = "Real filename to enable/disable serving for")]
    pub real_name: String,
}

#[derive(Args, Debug)]
pub struct ServeRenameArgs {
    /// The real (original) name of the file
    #[arg(value_name = "FILE", help = "Real filename to set the serve name for")]
    pub real_name: String,

    /// The new serve name (spoofed filename)
    #[arg(long, value_name = "NAME", help = "New spoofed filename for serving")]
    pub serve_name: String,
}

#[derive(Subcommand, Debug)]
pub enum SocketCommands {
    /// List all active swerve socket bindings
    ///
    /// Shows each interface:port pair where swerve is currently serving files.
    ///
    /// Example: fswerve sockets list
    List,

    /// Bind a new swerve socket to serve files on a specific address
    ///
    /// Example: fswerve sockets bind 0.0.0.0:8080
    Bind(SocketAddrArgs),

    /// Unbind a swerve socket to stop serving files on that address
    ///
    /// The socket is gracefully shut down and the port is released.
    ///
    /// Example: fswerve sockets unbind 0.0.0.0:8080
    Unbind(SocketAddrArgs),
}

#[derive(Args, Debug)]
pub struct SocketAddrArgs {
    /// The interface:port to bind/unbind
    #[arg(value_name = "ADDR", help = "Socket address as interface:port (e.g., 0.0.0.0:8080)")]
    pub addr: String,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_name = "SHELL")]
    pub shell: Shell,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_upload_bare() {
        let cli = Cli::try_parse_from(["fswerve", "upload", "test.bin"]).unwrap();
        match cli.command {
            Commands::Upload(args) => {
                assert_eq!(args.file, "test.bin");
                assert!(args.serve_name.is_none());
            }
            _ => panic!("Expected Upload command"),
        }
    }

    #[test]
    fn parse_upload_with_serve_name() {
        let cli =
            Cli::try_parse_from(["fswerve", "upload", "test.bin", "--serve-name", "fake.exe"])
                .unwrap();
        match cli.command {
            Commands::Upload(args) => {
                assert_eq!(args.file, "test.bin");
                assert_eq!(args.serve_name.as_deref(), Some("fake.exe"));
            }
            _ => panic!("Expected Upload command"),
        }
    }

    #[test]
    fn parse_destroy_with_yes() {
        let cli =
            Cli::try_parse_from(["fswerve", "destroy", "test.bin", "--yes"]).unwrap();
        match cli.command {
            Commands::Destroy(args) => {
                assert_eq!(args.real_name, "test.bin");
                assert!(args.yes);
            }
            _ => panic!("Expected Destroy command"),
        }
    }

    #[test]
    fn parse_destroy_without_yes_defaults_false() {
        let cli = Cli::try_parse_from(["fswerve", "destroy", "test.bin"]).unwrap();
        match cli.command {
            Commands::Destroy(args) => {
                assert!(!args.yes);
            }
            _ => panic!("Expected Destroy command"),
        }
    }

    #[test]
    fn parse_destroy_alias_delete() {
        let cli =
            Cli::try_parse_from(["fswerve", "delete", "test.bin", "--yes"]).unwrap();
        assert!(matches!(cli.command, Commands::Destroy(_)));
    }

    #[test]
    fn parse_destroy_alias_rm() {
        let cli =
            Cli::try_parse_from(["fswerve", "rm", "test.bin", "--yes"]).unwrap();
        assert!(matches!(cli.command, Commands::Destroy(_)));
    }

    #[test]
    fn parse_serve_enable() {
        let cli =
            Cli::try_parse_from(["fswerve", "serve", "enable", "test.bin"]).unwrap();
        match cli.command {
            Commands::Serve(ServeCommands::Enable(args)) => {
                assert_eq!(args.real_name, "test.bin");
            }
            _ => panic!("Expected Serve Enable"),
        }
    }

    #[test]
    fn parse_serve_disable() {
        let cli =
            Cli::try_parse_from(["fswerve", "serve", "disable", "test.bin"]).unwrap();
        match cli.command {
            Commands::Serve(ServeCommands::Disable(args)) => {
                assert_eq!(args.real_name, "test.bin");
            }
            _ => panic!("Expected Serve Disable"),
        }
    }

    #[test]
    fn parse_serve_set_name() {
        let cli = Cli::try_parse_from([
            "fswerve",
            "serve",
            "set-name",
            "test.bin",
            "--serve-name",
            "fake.exe",
        ])
        .unwrap();
        match cli.command {
            Commands::Serve(ServeCommands::SetName(args)) => {
                assert_eq!(args.real_name, "test.bin");
                assert_eq!(args.serve_name, "fake.exe");
            }
            _ => panic!("Expected Serve SetName"),
        }
    }

    #[test]
    fn parse_serve_rename_alias() {
        let cli = Cli::try_parse_from([
            "fswerve",
            "serve",
            "rename",
            "test.bin",
            "--serve-name",
            "fake.exe",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Serve(ServeCommands::SetName(_))
        ));
    }

    #[test]
    fn parse_sockets_bind() {
        let cli =
            Cli::try_parse_from(["fswerve", "sockets", "bind", "0.0.0.0:8080"]).unwrap();
        match cli.command {
            Commands::Sockets(SocketCommands::Bind(args)) => {
                assert_eq!(args.addr, "0.0.0.0:8080");
            }
            _ => panic!("Expected Sockets Bind"),
        }
    }

    #[test]
    fn parse_sockets_unbind() {
        let cli =
            Cli::try_parse_from(["fswerve", "sockets", "unbind", "0.0.0.0:8080"]).unwrap();
        match cli.command {
            Commands::Sockets(SocketCommands::Unbind(args)) => {
                assert_eq!(args.addr, "0.0.0.0:8080");
            }
            _ => panic!("Expected Sockets Unbind"),
        }
    }

    #[test]
    fn parse_sockets_list() {
        let cli = Cli::try_parse_from(["fswerve", "sockets", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Sockets(SocketCommands::List)
        ));
    }

    #[test]
    fn parse_files() {
        let cli = Cli::try_parse_from(["fswerve", "files"]).unwrap();
        assert!(matches!(cli.command, Commands::Files));
    }

    #[test]
    fn parse_status() {
        let cli = Cli::try_parse_from(["fswerve", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_download_bare() {
        let cli =
            Cli::try_parse_from(["fswerve", "download", "payload.bin"]).unwrap();
        match cli.command {
            Commands::Download(args) => {
                assert_eq!(args.real_name, "payload.bin");
                assert!(args.output.is_none());
            }
            _ => panic!("Expected Download"),
        }
    }

    #[test]
    fn parse_download_with_output() {
        let cli = Cli::try_parse_from([
            "fswerve",
            "download",
            "payload.bin",
            "-o",
            "out.bin",
        ])
        .unwrap();
        match cli.command {
            Commands::Download(args) => {
                assert_eq!(args.real_name, "payload.bin");
                assert_eq!(args.output.as_deref(), Some("out.bin"));
            }
            _ => panic!("Expected Download"),
        }
    }

    #[test]
    fn parse_config_set() {
        let cli = Cli::try_parse_from([
            "fswerve",
            "config",
            "set",
            "--server-url",
            "http://test:9740",
            "--api-key",
            "mykey",
        ])
        .unwrap();
        match cli.command {
            Commands::Config(ConfigCommands::Set(args)) => {
                assert_eq!(args.server_url, "http://test:9740");
                assert_eq!(args.api_key, "mykey");
            }
            _ => panic!("Expected Config Set"),
        }
    }

    #[test]
    fn parse_config_show() {
        let cli = Cli::try_parse_from(["fswerve", "config", "show"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Config(ConfigCommands::Show)
        ));
    }

    #[test]
    fn global_json_flag() {
        let cli = Cli::try_parse_from(["fswerve", "--json", "files"]).unwrap();
        assert!(cli.json);
        assert!(!cli.quiet);
        assert!(!cli.verbose);
    }

    #[test]
    fn global_quiet_flag() {
        let cli = Cli::try_parse_from(["fswerve", "--quiet", "status"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn global_verbose_flag() {
        let cli = Cli::try_parse_from(["fswerve", "--verbose", "files"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn global_no_color_flag() {
        let cli = Cli::try_parse_from(["fswerve", "--no-color", "files"]).unwrap();
        assert!(cli.no_color);
    }

    #[test]
    fn global_server_url_override() {
        let cli = Cli::try_parse_from([
            "fswerve",
            "--server-url",
            "http://override:1234",
            "status",
        ])
        .unwrap();
        assert_eq!(cli.server_url.as_deref(), Some("http://override:1234"));
    }

    #[test]
    fn global_api_key_override() {
        let cli =
            Cli::try_parse_from(["fswerve", "--api-key", "secret", "status"]).unwrap();
        assert_eq!(cli.api_key.as_deref(), Some("secret"));
    }

    #[test]
    fn global_flags_default_off() {
        let cli = Cli::try_parse_from(["fswerve", "files"]).unwrap();
        assert!(!cli.json);
        assert!(!cli.quiet);
        assert!(!cli.verbose);
        assert!(!cli.no_color);
        assert!(cli.server_url.is_none());
        assert!(cli.api_key.is_none());
    }

    #[test]
    fn missing_subcommand_is_error() {
        assert!(Cli::try_parse_from(["fswerve"]).is_err());
    }

    #[test]
    fn upload_missing_file_arg_is_error() {
        assert!(Cli::try_parse_from(["fswerve", "upload"]).is_err());
    }

    #[test]
    fn destroy_missing_file_arg_is_error() {
        assert!(Cli::try_parse_from(["fswerve", "destroy"]).is_err());
    }

    #[test]
    fn config_set_missing_args_is_error() {
        assert!(Cli::try_parse_from(["fswerve", "config", "set"]).is_err());
    }
}
