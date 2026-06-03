use crate::config::Config;
use colored::control::set_override;
use colored::Colorize;
use swerve_core::{
    api::StatusResponse,
    types::{SwerveFile, SwerveSocket},
};

pub struct OutputConfig {
    pub json: bool,
    pub quiet: bool,
}

pub fn init_color(no_color: bool) {
    if no_color {
        set_override(false);
    }
}

pub fn print_success(msg: &str, out: &OutputConfig) {
    if out.quiet {
        return;
    }
    if out.json {
        println!("{}", serde_json::json!({"ok": true, "message": msg}));
    } else {
        println!("{} {}", "[✓]".green().bold(), msg);
    }
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}", "[✗]".red().bold(), msg);
}

pub fn print_error_json(msg: &str, out: &OutputConfig) {
    if out.json {
        eprintln!("{}", serde_json::json!({"ok": false, "error": msg}));
    } else {
        print_error(msg);
    }
}

pub fn print_status(status: &StatusResponse, out: &OutputConfig) {
    if status.ok && out.quiet {
        return;
    }
    if out.json {
        if status.ok {
            println!("{}", serde_json::json!({"ok": true, "message": status.message}));
        } else {
            eprintln!("{}", serde_json::json!({"ok": false, "error": status.message}));
        }
        return;
    }
    if status.ok {
        println!("{} {}", "[✓]".green().bold(), &status.message);
    } else {
        print_error(&status.message);
    }
}

pub fn print_config(config: &Config, out: &OutputConfig) {
    if out.quiet {
        return;
    }
    if out.json {
        println!("{}", serde_json::json!({
            "ok": true,
            "data": {
                "server_url": &config.server_url,
                "api_key": mask_key(&config.api_key),
            }
        }));
        return;
    }
    println!("{}", "fswerve configuration".bold().underline());
    println!("  {} {}", "Server URL:".cyan(), config.server_url);
    println!("  {} {}", "API Key:   ".cyan(), mask_key(&config.api_key));
}

pub fn print_file_list(files: &[SwerveFile], out: &OutputConfig) {
    if out.quiet {
        return;
    }
    if out.json {
        println!("{}", serde_json::json!({"ok": true, "data": files}));
        return;
    }
    if files.is_empty() {
        println!("{}", "No files on server.".dimmed());
        return;
    }

    // Dynamic column widths
    let rn_width = files.iter().map(|f| f.real_name.len()).max().unwrap_or(9).clamp(9, 40);
    let sn_width = files.iter().map(|f| f.serve_name.len()).max().unwrap_or(10).clamp(10, 40);

    println!(
        "{:<rn_width$}  {:<sn_width$}  {:>10}  {}",
        "REAL NAME".bold(),
        "SERVE NAME".bold(),
        "SIZE".bold(),
        "SERVING".bold(),
    );
    println!("{}", "─".repeat(rn_width + sn_width + 22));

    for f in files {
        let serving = if f.serving {
            "● ON".green().to_string()
        } else {
            "○ OFF".dimmed().to_string()
        };
        let rn = truncate(&f.real_name, rn_width);
        let sn = truncate(&f.serve_name, sn_width);
        println!(
            "{:<rn_width$}  {:<sn_width$}  {:>10}  {}",
            rn, sn, human_size(f.size), serving,
        );
    }
}

pub fn print_socket_list(sockets: &[SwerveSocket], out: &OutputConfig) {
    if out.quiet {
        return;
    }
    if out.json {
        println!("{}", serde_json::json!({"ok": true, "data": sockets}));
        return;
    }
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

fn truncate(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else if max > 3 {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max).collect()
    }
}

fn mask_key(key: &str) -> String {
    let char_count = key.chars().count();
    if char_count <= 4 {
        "****".to_string()
    } else {
        let prefix: String = key.chars().take(4).collect();
        format!("{}****", prefix)
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- truncate ---

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        assert_eq!(truncate("longfilename.bin", 10), "longfil...");
    }

    #[test]
    fn truncate_max_3_no_ellipsis() {
        assert_eq!(truncate("abcdef", 3), "abc");
    }

    #[test]
    fn truncate_max_4_with_ellipsis() {
        assert_eq!(truncate("abcdef", 4), "a...");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn truncate_unicode_no_panic() {
        let s = "🔥🔥🔥🔥🔥🔥";
        let result = truncate(s, 5);
        assert_eq!(result, "🔥🔥...");
    }

    // --- mask_key ---

    #[test]
    fn mask_key_short_fully_masked() {
        assert_eq!(mask_key("abc"), "****");
        assert_eq!(mask_key("abcd"), "****");
    }

    #[test]
    fn mask_key_shows_first_four() {
        assert_eq!(mask_key("abcde"), "abcd****");
        assert_eq!(mask_key("my-secret-key-12345"), "my-s****");
    }

    #[test]
    fn mask_key_empty() {
        assert_eq!(mask_key(""), "****");
    }

    #[test]
    fn mask_key_unicode_no_panic() {
        let key = "🔑🔑🔑🔑🔑extra";
        let result = mask_key(key);
        assert_eq!(result, "🔑🔑🔑🔑****");
    }

    // --- human_size ---

    #[test]
    fn human_size_bytes() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1023), "1023 B");
    }

    #[test]
    fn human_size_kilobytes() {
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
    }

    #[test]
    fn human_size_megabytes() {
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn human_size_gigabytes() {
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(human_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    // --- OutputConfig ---

    #[test]
    fn output_config_fields() {
        let out = OutputConfig {
            json: true,
            quiet: false,
        };
        assert!(out.json);
        assert!(!out.quiet);
    }
}
