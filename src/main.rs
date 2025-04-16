use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
};

use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use serde_json::json;

static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .pool_max_idle_per_host(10)
        .build()
        .unwrap()
});

#[derive(Debug)]
struct Config {
    telegram_token: String,
    telegram_chat_id: String,
    parse_mode: String,
    verbose: bool,
}

fn main() -> io::Result<()> {
    let config = Arc::new(parse_args());
    let listener = TcpListener::bind("0.0.0.0:2525")?;

    if config.verbose {
        println!("[smtp2tg] SMTP server running on 0.0.0.0:2525");
    }

    for stream in listener.incoming() {
        let config = Arc::clone(&config);
        let client = &*HTTP_CLIENT;

        std::thread::spawn(move || {
            if let Ok(mut stream) = stream {
                if config.verbose {
                    println!("[smtp2tg] Connection accepted");
                }
                if let Err(e) = handle_client(&mut stream, &config, client) {
                    if config.verbose {
                        eprintln!("[smtp2tg] Client error: {}", e);
                    }
                }
            }
        });
    }
    Ok(())
}

fn handle_client(stream: &mut TcpStream, config: &Config, client: &Client) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let writer = stream;

    let mut state = 0;
    let mut line = String::new();

    writer.write_all(b"220 smtp2tg ready\r\n")?;

    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }

        let cmd = line.trim_end();

        if config.verbose {
            println!("[smtp2tg] SMTP command: {}", cmd);
        }

        if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
            writer.write_all(b"250 smtp2tg\r\n")?;
        } else if cmd.starts_with("MAIL FROM:") {
            state = 1;
            writer.write_all(b"250 OK\r\n")?;
        } else if cmd.starts_with("RCPT TO:") {
            if state < 1 {
                writer.write_all(b"503 MAIL first\r\n")?;
            } else {
                state = 2;
                writer.write_all(b"250 OK\r\n")?;
            }
        } else if cmd.eq_ignore_ascii_case("DATA") {
            if state < 2 {
                writer.write_all(b"503 Need MAIL and RCPT\r\n")?;
            } else {
                writer.write_all(b"354 End with <CR><LF>.<CR><LF>\r\n")?;
                writer.flush()?;
                break;
            }
        } else if cmd.eq_ignore_ascii_case("QUIT") {
            writer.write_all(b"221 Bye\r\n")?;
            return Ok(());
        } else {
            writer.write_all(b"502 Command not supported\r\n")?;
        }
    }

    let mut subject = String::new();
    let mut body = String::new();
    let mut in_headers = true;

    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed == "." {
            break;
        }
        if in_headers {
            if trimmed.is_empty() {
                in_headers = false;
            } else if subject.is_empty() && trimmed.to_lowercase().starts_with("subject:") {
                subject = trimmed[8..].trim().to_string();
            }
        } else {
            body.push_str(trimmed);
            body.push('\n');
        }
    }

    if subject.is_empty() {
        subject.push_str("[No Subject]");
    }

    if config.verbose {
        println!("[smtp2tg] Subject: {}", subject);
        println!("[smtp2tg] Body preview:\n{}", body.trim());
    }

    let msg = match config.parse_mode.as_str() {
        "HTML" => format!(
            "📨 <b>{}</b>\n<blockquote expandable>{}</blockquote>",
            html_escape(&subject),
            html_escape(body.trim())
        ),
        _ => format!(
            "📨 *{}*\n{}",
            escape_markdown(&subject),
            format_expandable_quote(body.trim())
        ),
    };

    match send_telegram(&msg, config, client) {
        Ok(_) => {
            if config.verbose {
                println!("[smtp2tg] Telegram message sent");
            }
        }
        Err(e) => {
            if config.verbose {
                eprintln!("[smtp2tg] Telegram error: {}", e);
            }
        }
    }

    writer.write_all(b"250 Message accepted\r\n")?;
    Ok(())
}

fn html_escape(text: &str) -> String {
    text.chars().map(|c| match c {
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '&' => "&amp;".to_string(),
        '"' => "&quot;".to_string(),
        _ => c.to_string(),
    }).collect()
}

fn escape_markdown(text: &str) -> String {
    text.chars().flat_map(|c| {
        if "()[]{}<>`#+-=|.!*_\\".contains(c) {
            vec!['\\', c]
        } else {
            vec![c]
        }
    }).collect()
}

fn format_expandable_quote(text: &str) -> String {
    let mut lines = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let escaped = escape_markdown(line);
        if i == 0 {
            lines.push(format!("**> {}", escaped)); // bold + quote start
        } else {
            lines.push(format!("> {}", escaped));
        }
    }

    if lines.len() > 3 {
        lines.insert(3, "> ".to_string()); // trigger expandable
    }

    if let Some(last) = lines.last_mut() {
        last.push_str("||");
    }

    lines.join("\n")
}

fn send_telegram(text: &str, config: &Config, client: &Client) -> Result<(), reqwest::Error> {
    client
        .post(&format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_token))
        .json(&json!({
            "chat_id": config.telegram_chat_id,
            "text": text,
            "parse_mode": config.parse_mode,
        }))
        .send()?
        .error_for_status()?;
    Ok(())
}

fn parse_args() -> Config {
    let mut args = env::args().skip(1);
    let mut config = Config {
        telegram_token: String::new(),
        telegram_chat_id: String::new(),
        parse_mode: "MarkdownV2".to_string(),
        verbose: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--token" | "-t" => {
                config.telegram_token = args.next().unwrap_or_else(|| {
                    eprintln!("ERROR: --token requires value");
                    std::process::exit(1);
                });
            }
            "--chatid" | "-c" => {
                config.telegram_chat_id = args.next().unwrap_or_else(|| {
                    eprintln!("ERROR: --chatid requires value");
                    std::process::exit(1);
                });
            }
            "--parsemode" | "-p" => {
                if let Some(mode) = args.next() {
                    config.parse_mode = mode;
                }
            }
            "--verbose" | "-v" => {
                config.verbose = true;
            }
            "--help" | "-h" => {
                println!(
"SMTP2TG - Lightweight SMTP to Telegram forwarder

USAGE:
  smtp2tg -t TOKEN -c CHAT_ID [OPTIONS]

REQUIRED:
  -t, --token       Telegram bot token
  -c, --chatid      Telegram chat ID

OPTIONS:
  -p, --parsemode   Message format: MarkdownV2 (default) or HTML
  -v, --verbose     Enable verbose output
  -h, --help        Show this help message

EXAMPLE:
  smtp2tg --token abc123 --chatid 123456789 --parsemode HTML --verbose
");
                std::process::exit(0);
            }
            _ => {
                eprintln!("ERROR: Unknown argument '{}'", arg);
                std::process::exit(1);
            }
        }
    }

    if config.telegram_token.is_empty() || config.telegram_chat_id.is_empty() {
        eprintln!("ERROR: Required --token and --chatid");
        std::process::exit(1);
    }

    config
}
