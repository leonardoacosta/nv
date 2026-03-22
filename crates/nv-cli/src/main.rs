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
    /// Show message statistics and usage dashboard
    Stats,
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
        Commands::Stats => {
            fetch_stats().await;
        }
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

/// Stats response from the daemon.
#[derive(Debug, Deserialize)]
struct StatsResponse {
    total_messages: i64,
    messages_today: i64,
    avg_response_time_ms: Option<f64>,
    total_tokens_in: i64,
    total_tokens_out: i64,
    daily_counts: Vec<(String, i64)>,
    #[serde(default)]
    tool_usage: Option<ToolUsageStats>,
    #[serde(default)]
    claude_usage: Option<ClaudeUsageStats>,
    #[serde(default)]
    budget: Option<BudgetStatusEntry>,
    #[serde(default)]
    account: Option<AccountInfoEntry>,
}

/// Account info from the daemon /stats response.
#[derive(Debug, Deserialize)]
struct AccountInfoEntry {
    plan: String,
    username: String,
    auth_method: String,
}

/// Budget status from the daemon /stats response.
#[derive(Debug, Deserialize)]
struct BudgetStatusEntry {
    rolling_7d_cost: f64,
    weekly_budget: f64,
    pct_used: f64,
}

/// Tool usage stats section from the daemon /stats response.
#[derive(Debug, Deserialize)]
struct ToolUsageStats {
    total_invocations: i64,
    invocations_today: i64,
    per_tool: Vec<ToolBreakdownEntry>,
    slowest: Vec<SlowestEntry>,
}

#[derive(Debug, Deserialize)]
struct ToolBreakdownEntry {
    name: String,
    count: i64,
    success_count: i64,
    avg_duration_ms: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SlowestEntry {
    tool_name: String,
    duration_ms: i64,
    timestamp: String,
}

/// Claude API usage stats from the daemon /stats response.
#[derive(Debug, Deserialize)]
struct ClaudeUsageStats {
    today_cost: f64,
    today_calls: i64,
    today_tokens_in: i64,
    today_tokens_out: i64,
    week_cost: f64,
    month_cost: f64,
    #[serde(default)]
    daily_breakdown: Vec<DailyUsageEntry>,
}

#[derive(Debug, Deserialize)]
struct DailyUsageEntry {
    date: String,
    cost: f64,
    calls: i64,
    #[allow(dead_code)]
    tokens_in: i64,
    #[allow(dead_code)]
    tokens_out: i64,
}

/// Fetch and display message statistics from the daemon.
async fn fetch_stats() {
    let port = get_health_port();
    let url = format!("http://127.0.0.1:{port}/stats");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to create HTTP client");

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to read response: {e}");
                    std::process::exit(1);
                }
            };

            match serde_json::from_str::<StatsResponse>(&body) {
                Ok(stats) => display_stats(&stats),
                Err(e) => {
                    eprintln!("Failed to parse stats response: {e}");
                    std::process::exit(1);
                }
            }
        }
        Ok(resp) => {
            eprintln!("Daemon returned HTTP {}", resp.status());
            std::process::exit(1);
        }
        Err(e) => {
            if e.is_connect() {
                eprintln!("Cannot connect to NV daemon. Is it running?");
            } else {
                eprintln!("Failed to connect to daemon: {e}");
            }
            std::process::exit(1);
        }
    }
}

fn display_stats(stats: &StatsResponse) {
    let avg_time = stats
        .avg_response_time_ms
        .map(|ms| format!("{:.1}s", ms / 1000.0))
        .unwrap_or_else(|| "n/a".into());

    println!("Nova Stats");
    println!("\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}");

    if let Some(acct) = &stats.account {
        println!(
            "Account:           {} / {} / {}",
            acct.plan, acct.username, acct.auth_method
        );
    }
    println!(
        "Total messages:    {}",
        format_number(stats.total_messages)
    );
    println!(
        "Today:             {}",
        format_number(stats.messages_today)
    );
    println!("Avg response time: {avg_time}");
    println!(
        "Tokens (in/out):   {} / {}",
        format_number(stats.total_tokens_in),
        format_number(stats.total_tokens_out)
    );

    if !stats.daily_counts.is_empty() {
        println!();
        println!("Last 7 days:");

        let max_count = stats
            .daily_counts
            .iter()
            .map(|(_, c)| *c)
            .max()
            .unwrap_or(1)
            .max(1);

        for (date, count) in &stats.daily_counts {
            // Extract month-day from YYYY-MM-DD
            let display_date = if date.len() >= 10 {
                &date[5..10]
            } else {
                date
            };
            let bar_len = (*count as f64 / max_count as f64 * 20.0) as usize;
            let bar: String = "\u{2588}".repeat(bar_len);
            println!("  {display_date}: {bar:<20} {count}");
        }
    }

    // Tool usage section
    if let Some(tu) = &stats.tool_usage {
        println!();
        println!("Tool Usage");
        println!("\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}");
        println!(
            "Total calls:       {}",
            format_number(tu.total_invocations)
        );
        println!(
            "Today:             {}",
            format_number(tu.invocations_today)
        );

        if !tu.per_tool.is_empty() {
            println!();
            println!(
                "  {:<24} {:>6} {:>9} {:>8}",
                "Tool", "Calls", "Success%", "Avg ms"
            );
            println!("  {}", "\u{2500}".repeat(51));
            for entry in &tu.per_tool {
                let success_pct = if entry.count > 0 {
                    (entry.success_count as f64 / entry.count as f64 * 100.0) as i64
                } else {
                    0
                };
                let avg_ms = entry
                    .avg_duration_ms
                    .map(|ms| format!("{:.0}", ms))
                    .unwrap_or_else(|| "n/a".into());
                println!(
                    "  {:<24} {:>6} {:>8}% {:>8}",
                    entry.name, entry.count, success_pct, avg_ms
                );
            }
        }

        if !tu.slowest.is_empty() {
            println!();
            println!("  Top 5 slowest:");
            for entry in &tu.slowest {
                let ts = if entry.timestamp.len() >= 16 {
                    &entry.timestamp[..16]
                } else {
                    &entry.timestamp
                };
                println!(
                    "    {}ms  {:<20} {}",
                    entry.duration_ms, entry.tool_name, ts
                );
            }
        }
    }

    // Claude Usage section
    if let Some(cu) = &stats.claude_usage {
        println!();
        println!("Claude Usage");
        println!("\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}");

        let today_tokens_total = cu.today_tokens_in + cu.today_tokens_out;
        let tokens_display = format_number(today_tokens_total);
        println!(
            "Today:             ${:.2} / {} calls / {} tokens",
            cu.today_cost, cu.today_calls, tokens_display
        );
        println!("This week:         ${:.2}", cu.week_cost);
        println!("This month:        ${:.2}", cu.month_cost);

        if let Some(budget) = &stats.budget {
            println!(
                "Budget:            ${:.2} / ${:.2} ({:.0}%)",
                budget.rolling_7d_cost, budget.weekly_budget, budget.pct_used
            );
        }

        if !cu.daily_breakdown.is_empty() {
            println!();
            println!("  {:<12} {:>10} {:>8}", "Date", "Cost", "Calls");
            println!("  {}", "\u{2500}".repeat(32));
            for day in &cu.daily_breakdown {
                let display_date = if day.date.len() >= 10 {
                    &day.date[5..10]
                } else {
                    &day.date
                };
                println!(
                    "  {:<12} {:>9} {:>8}",
                    display_date,
                    format!("${:.2}", day.cost),
                    day.calls
                );
            }
        }
    }
}

fn format_number(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        // Add commas
        let s = n.to_string();
        let mut result = String::new();
        for (i, ch) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(ch);
        }
        result.chars().rev().collect()
    } else {
        n.to_string()
    }
}
