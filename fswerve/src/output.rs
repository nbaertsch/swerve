use crate::config::Config;
use colored::Colorize;
use swerve_core::{
    api::StatusResponse,
    types::{SwerveFile, SwerveSocket},
};

pub fn print_success(msg: &str) {
    println!("{} {}", "[✓]".green().bold(), msg);
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}", "[✗]".red().bold(), msg);
}

pub fn print_status(status: &StatusResponse) {
    if status.ok {
        print_success(&status.message);
    } else {
        print_error(&status.message);
    }
}

pub fn print_config(config: &Config) {
    println!("{}", "fswerve configuration".bold().underline());
    println!("  {} {}", "Server URL:".cyan(), config.server_url);
    println!("  {} {}", "API Key:   ".cyan(), mask_key(&config.api_key));
}

pub fn print_file_list(files: &[SwerveFile]) {
    if files.is_empty() {
        println!("{}", "No files on server.".dimmed());
        return;
    }

    println!(
        "{:<30} {:<30} {:>10} {}",
        "REAL NAME".bold(),
        "SERVE NAME".bold(),
        "SIZE".bold(),
        "SERVING".bold()
    );
    println!("{}", "─".repeat(80));

    for f in files {
        let serving = if f.serving {
            "● ON".green().to_string()
        } else {
            "○ OFF".dimmed().to_string()
        };
        println!(
            "{:<30} {:<30} {:>10} {}",
            f.real_name,
            f.serve_name,
            human_size(f.size),
            serving
        );
    }
}

pub fn print_socket_list(sockets: &[SwerveSocket]) {
    if sockets.is_empty() {
        println!("{}", "No active swerve sockets.".dimmed());
        return;
    }

    println!("{}", "ACTIVE SWERVE SOCKETS".bold().underline());
    for s in sockets {
        let status = if s.active {
            "● LISTENING".green().to_string()
        } else {
            "○ INACTIVE".dimmed().to_string()
        };
        println!("  {} {}", status, s.addr);
    }
}

fn mask_key(key: &str) -> String {
    if key.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}****", &key[..4])
    }
}

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
