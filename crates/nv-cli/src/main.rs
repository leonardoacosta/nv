mod commands;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "nv", about = "NV -- Master Agent Harness CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show daemon and session status
    Status,
    /// Ask NV a question
    Ask {
        /// The question to ask
        query: String,
        /// Output raw JSON response with sources
        #[arg(long)]
        json: bool,
    },
    /// Manage NV configuration
    Config,
    /// Trigger or view digest
    Digest {
        /// Trigger immediate digest
        #[arg(long)]
        now: bool,
    },
}

/// Request body for POST /ask.
#[derive(Debug, Serialize)]
struct AskRequest {
    question: String,
}

/// Response body for POST /ask.
#[derive(Debug, Deserialize)]
struct AskResponse {
    answer: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => {
            let port = get_health_port();
            commands::status::run(port).await;
        }
        Commands::Ask { query, json } => {
            ask_question(&query, json).await;
        }
        Commands::Config => println!("not implemented yet"),
        Commands::Digest { now } => {
            if now {
                trigger_digest_now().await;
            } else {
                println!("not implemented yet: show last digest");
            }
        }
    }
}

/// Read health_port from config, falling back to 8400.
fn get_health_port() -> u16 {
    nv_core::Config::load()
        .ok()
        .and_then(|c| c.daemon.map(|d| d.health_port))
        .unwrap_or(8400)
}

/// Send HTTP POST to the daemon to trigger an immediate digest.
async fn trigger_digest_now() {
    let port = get_health_port();
    let url = format!("http://127.0.0.1:{port}/digest");

    let client = reqwest::Client::new();
    match client.post(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() || status.as_u16() == 202 {
                println!("Digest triggered.");
            } else {
                let body = resp.text().await.unwrap_or_default();
                eprintln!("Daemon returned {status}: {body}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to NV daemon at {url}: {e}");
            eprintln!("Is the daemon running?");
            std::process::exit(1);
        }
    }
}

/// Send a question to the NV daemon via POST /ask and print the answer.
async fn ask_question(query: &str, json_output: bool) {
    let port = get_health_port();
    let url = format!("http://127.0.0.1:{port}/ask");

    let client = reqwest::Client::new();
    match client
        .post(&url)
        .json(&AskRequest {
            question: query.to_string(),
        })
        .timeout(std::time::Duration::from_secs(65))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if !status.is_success() {
                if let Ok(parsed) = serde_json::from_str::<AskResponse>(&body) {
                    eprintln!("{}", parsed.answer);
                } else {
                    eprintln!("Daemon returned {status}: {body}");
                }
                std::process::exit(1);
            }

            if json_output {
                println!("{body}");
            } else {
                match serde_json::from_str::<AskResponse>(&body) {
                    Ok(parsed) => println!("{}", parsed.answer),
                    Err(_) => println!("{body}"),
                }
            }
        }
        Err(e) => {
            if e.is_connect() {
                eprintln!("Cannot connect to NV daemon at {url}. Is it running?");
            } else if e.is_timeout() {
                eprintln!("Query timed out after 65 seconds.");
            } else {
                eprintln!("HTTP request failed: {e}");
            }
            std::process::exit(1);
        }
    }
}
