mod cli;
mod client;
mod config;
mod output;

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use output::OutputConfig;
use std::io;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    output::init_color(cli.no_color);

    let out = OutputConfig {
        json: cli.json,
        quiet: cli.quiet,
    };

    if let Err(e) = run(cli, &out).await {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}

fn make_client(
    server_url: Option<&str>,
    api_key: Option<&str>,
    verbose: bool,
) -> Result<client::SwerveClient, Box<dyn std::error::Error>> {
    let cfg = config::resolve_config(server_url, api_key)?;
    Ok(client::SwerveClient::new(&cfg, verbose))
}

async fn run(cli: cli::Cli, out: &OutputConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Extract overrides before moving cli.command
    let server_url = cli.server_url;
    let api_key = cli.api_key;
    let verbose = cli.verbose;

    match cli.command {
        cli::Commands::Config(cmd) => match cmd {
            cli::ConfigCommands::Set(args) => {
                config::save_config(&config::Config {
                    server_url: args.server_url,
                    api_key: args.api_key,
                })?;
                output::print_success("Configuration saved", out);
                Ok(())
            }
            cli::ConfigCommands::Show => {
                let cfg = config::resolve_config(
                    server_url.as_deref(),
                    api_key.as_deref(),
                )?;
                output::print_config(&cfg, out);
                Ok(())
            }
        },
        cli::Commands::Upload(args) => {
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            let result = client.upload_file(&args.file, args.serve_name.as_deref()).await?;
            output::print_status(&result, out);
            Ok(())
        }
        cli::Commands::Files => {
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            let files = client.list_files().await?;
            output::print_file_list(&files, out);
            Ok(())
        }
        cli::Commands::Download(args) => {
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            let output_path = args.output.as_deref().unwrap_or(&args.real_name);
            client.download_file(&args.real_name, output_path).await?;
            output::print_success(
                &format!("Downloaded '{}' to '{}'", args.real_name, output_path),
                out,
            );
            Ok(())
        }
        cli::Commands::Destroy(args) => {
            if !args.yes {
                eprint!("Permanently delete '{}' from the server? [y/N] ", args.real_name);
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Aborted.");
                    return Ok(());
                }
            }
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            let result = client.destroy_file(&args.real_name).await?;
            output::print_status(&result, out);
            Ok(())
        }
        cli::Commands::Serve(cmd) => {
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            match cmd {
                cli::ServeCommands::Enable(args) => {
                    let result = client.set_serve_state(&args.real_name, true).await?;
                    output::print_status(&result, out);
                }
                cli::ServeCommands::Disable(args) => {
                    let result = client.set_serve_state(&args.real_name, false).await?;
                    output::print_status(&result, out);
                }
                cli::ServeCommands::SetName(args) => {
                    let result = client.set_serve_name(&args.real_name, &args.serve_name).await?;
                    output::print_status(&result, out);
                }
            }
            Ok(())
        }
        cli::Commands::Sockets(cmd) => {
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            match cmd {
                cli::SocketCommands::List => {
                    let sockets = client.list_sockets().await?;
                    output::print_socket_list(&sockets, out);
                }
                cli::SocketCommands::Bind(args) => {
                    let result = client.bind_socket(&args.addr).await?;
                    output::print_status(&result, out);
                }
                cli::SocketCommands::Unbind(args) => {
                    let result = client.unbind_socket(&args.addr).await?;
                    output::print_status(&result, out);
                }
            }
            Ok(())
        }
        cli::Commands::Status => {
            let client = make_client(server_url.as_deref(), api_key.as_deref(), verbose)?;
            let result = client.health().await?;
            output::print_status(&result, out);
            Ok(())
        }
        cli::Commands::Completions(args) => {
            let mut cmd = cli::Cli::command();
            generate(args.shell, &mut cmd, "fswerve", &mut io::stdout());
            Ok(())
        }
    }
}
