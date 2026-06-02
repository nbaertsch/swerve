use clap::{Args, Parser, Subcommand};

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
    /// Optionally specify a --serve-as name to control how the file appears
    /// when served through swerve sockets.
    Upload(UploadArgs),

    /// List all files currently stored on the server
    ///
    /// Shows real name, serve name, serving state, and file size for each file.
    Files,

    /// Download a file from the server via the management API
    ///
    /// Downloads are always available through the management port regardless
    /// of the file's serving state. The file is decrypted server-side before transfer.
    Download(DownloadArgs),

    /// Permanently delete a file from the server
    ///
    /// Removes both the encrypted file from storage and all associated metadata.
    /// This action cannot be undone.
    Destroy(DestroyArgs),

    /// Control file serving state and serve names
    ///
    /// Enable or disable serving for individual files, or change the filename
    /// that appears when the file is served through swerve sockets.
    #[command(subcommand)]
    Serve(ServeCommands),

    /// Manage swerve socket bindings (network interfaces for file serving)
    ///
    /// Swerve sockets are HTTP listeners that serve enabled files under their
    /// configured serve names. Create bindings on specific interface:port pairs
    /// to control where files are accessible.
    #[command(subcommand)]
    Sockets(SocketCommands),
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Set the swerve server URL and API key
    ///
    /// Example: fswerve config set --server-url http://10.0.0.5:9740 --api-key mySecretKey123
    Set(ConfigSetArgs),

    /// Display the current configuration
    Show,
}

#[derive(Args, Debug)]
pub struct ConfigSetArgs {
    /// The swerve server management URL (e.g., http://host:9740)
    #[arg(short, long, help = "Server management URL (e.g., http://10.0.0.5:9740)")]
    pub server_url: String,

    /// The API key for authenticating with the swerve server
    #[arg(short, long, help = "API key for server authentication")]
    pub api_key: String,
}

#[derive(Args, Debug)]
pub struct UploadArgs {
    /// Path to the local file to upload
    #[arg(help = "Local file path to upload (e.g., ./payload.bin)")]
    pub file: String,

    /// The filename to use when serving this file through swerve sockets.
    /// If omitted, the original filename is used as the serve name.
    #[arg(long, help = "Spoofed filename for serving (defaults to real filename)")]
    pub serve_as: Option<String>,
}

#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// The real (original) name of the file to download
    #[arg(help = "Real filename as it was uploaded (e.g., payload.bin)")]
    pub real_name: String,

    /// Output path for the downloaded file. Defaults to the real filename in the current directory.
    #[arg(short, long, help = "Output file path (defaults to ./<real_name>)")]
    pub output: Option<String>,
}

#[derive(Args, Debug)]
pub struct DestroyArgs {
    /// The real (original) name of the file to delete
    #[arg(help = "Real filename to permanently delete from the server")]
    pub real_name: String,
}

#[derive(Subcommand, Debug)]
pub enum ServeCommands {
    /// Enable serving for a file (make it accessible on swerve sockets)
    ///
    /// The file will be served under its configured serve name on all active
    /// swerve socket bindings.
    Enable(ServeTargetArgs),

    /// Disable serving for a file (stop serving it on swerve sockets)
    ///
    /// The file remains stored on the server and can still be downloaded
    /// via the management API.
    Disable(ServeTargetArgs),

    /// Change the serve name (spoofed filename) for a file
    ///
    /// This changes how the file appears when accessed through swerve sockets.
    /// The real filename and storage are not affected.
    Rename(ServeRenameArgs),
}

#[derive(Args, Debug)]
pub struct ServeTargetArgs {
    /// The real (original) name of the file
    #[arg(help = "Real filename to enable/disable serving for")]
    pub real_name: String,
}

#[derive(Args, Debug)]
pub struct ServeRenameArgs {
    /// The real (original) name of the file
    #[arg(help = "Real filename to rename the serve name for")]
    pub real_name: String,

    /// The new serve name (spoofed filename)
    #[arg(short, long, help = "New spoofed filename for serving")]
    pub name: String,
}

#[derive(Subcommand, Debug)]
pub enum SocketCommands {
    /// List all active swerve socket bindings
    ///
    /// Shows each interface:port pair where swerve is currently serving files.
    List,

    /// Bind a new swerve socket to serve files on a specific address
    ///
    /// Example: fswerve sockets bind 0.0.0.0:8080
    Bind(SocketAddrArgs),

    /// Unbind a swerve socket to stop serving files on that address
    ///
    /// The socket is gracefully shut down and the port is released.
    Unbind(SocketAddrArgs),
}

#[derive(Args, Debug)]
pub struct SocketAddrArgs {
    /// The interface:port to bind/unbind (e.g., 0.0.0.0:8080 or 192.168.1.5:443)
    #[arg(help = "Socket address as interface:port (e.g., 0.0.0.0:8080)")]
    pub addr: String,
}
