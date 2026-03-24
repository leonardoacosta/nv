mod registry;
mod server;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::error;

use crate::server::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let server = McpServer::new();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        // EOF — client closed the connection
        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "failed to parse JSON-RPC request");
                let parse_error = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    }
                });
                let mut response = serde_json::to_string(&parse_error)?;
                response.push('\n');
                stdout.write_all(response.as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }
        };

        let response = server.handle_request(request);
        let mut response_str = serde_json::to_string(&response)?;
        response_str.push('\n');
        stdout.write_all(response_str.as_bytes()).await?;
        stdout.flush().await?;
    }

    Ok(())
}
