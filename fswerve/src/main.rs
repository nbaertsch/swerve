mod cli;
mod client;
mod config;
mod output;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();

    if let Err(e) = run(cli).await {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}

async fn run(cli: cli::Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        cli::Commands::Config(cmd) => match cmd {
            cli::ConfigCommands::Set(args) => {
                config::save_config(&config::Config {
                    server_url: args.server_url,
                    api_key: args.api_key,
                })?;
                output::print_success("Configuration saved");
                Ok(())
            }
            cli::ConfigCommands::Show => {
                let cfg = config::load_config()?;
                output::print_config(&cfg);
                Ok(())
            }
        },
        cli::Commands::Upload(args) => {
            let cfg = config::load_config()?;
            let client = client::SwerveClient::new(&cfg);
            let result = client.upload_file(&args.file, args.serve_as.as_deref()).await?;
            output::print_status(&result);
            Ok(())
        }
        cli::Commands::Files => {
            let cfg = config::load_config()?;
            let client = client::SwerveClient::new(&cfg);
            let files = client.list_files().await?;
            output::print_file_list(&files);
            Ok(())
        }
        cli::Commands::Download(args) => {
            let cfg = config::load_config()?;
            let client = client::SwerveClient::new(&cfg);
            let output_path = args.output.as_deref().unwrap_or(&args.real_name);
            client.download_file(&args.real_name, output_path).await?;
            output::print_success(&format!("Downloaded '{}' to '{}'", args.real_name, output_path));
            Ok(())
        }
        cli::Commands::Destroy(args) => {
            let cfg = config::load_config()?;
            let client = client::SwerveClient::new(&cfg);
            let result = client.destroy_file(&args.real_name).await?;
            output::print_status(&result);
            Ok(())
        }
        cli::Commands::Serve(cmd) => {
            let cfg = config::load_config()?;
            let client = client::SwerveClient::new(&cfg);
            match cmd {
                cli::ServeCommands::Enable(args) => {
                    let result = client.set_serve_state(&args.real_name, true).await?;
                    output::print_status(&result);
                }
                cli::ServeCommands::Disable(args) => {
                    let result = client.set_serve_state(&args.real_name, false).await?;
                    output::print_status(&result);
                }
                cli::ServeCommands::Rename(args) => {
                    let result = client.set_serve_name(&args.real_name, &args.name).await?;
                    output::print_status(&result);
                }
            }
            Ok(())
        }
        cli::Commands::Sockets(cmd) => {
            let cfg = config::load_config()?;
            let client = client::SwerveClient::new(&cfg);
            match cmd {
                cli::SocketCommands::List => {
                    let sockets = client.list_sockets().await?;
                    output::print_socket_list(&sockets);
                }
                cli::SocketCommands::Bind(args) => {
                    let result = client.bind_socket(&args.addr).await?;
                    output::print_status(&result);
                }
                cli::SocketCommands::Unbind(args) => {
                    let result = client.unbind_socket(&args.addr).await?;
                    output::print_status(&result);
                }
            }
            Ok(())
        }
    }
}
