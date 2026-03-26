//! CloudPC SSH helper for running PowerShell scripts on the remote Windows machine.
//!
//! All Teams and Outlook tools that require delegated authentication execute PowerShell
//! scripts on `cloudpc` via SSH instead of calling Graph API directly. The scripts
//! manage their own device-code + token refresh flow.

const CLOUDPC_HOST: &str = "cloudpc";
const CLOUDPC_USER_PATH: &str = r"C:\Users\leo.346-CPC-QJXVZ";

/// Run a PowerShell script on the CloudPC via SSH.
///
/// The script is dot-sourced and invoked with the provided `args` string.
/// Returns stdout as a `String` with SSH/PowerShell noise lines stripped.
///
/// # Errors
///
/// Returns an error if:
/// - The SSH connection fails (host unreachable, auth failure).
/// - The PowerShell script exits with a non-zero status.
pub async fn ssh_cloudpc_script(script: &str, args: &str) -> anyhow::Result<String> {
    let cmd = format!(
        r#"powershell -ExecutionPolicy Bypass -Command "& {{ . {CLOUDPC_USER_PATH}\{script} {args} }}""#
    );

    let output = tokio::process::Command::new("ssh")
        .args(["-o", "ConnectTimeout=10", CLOUDPC_HOST, &cmd])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to invoke SSH: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Provide a friendlier message for connection-refused / timeout
        if stderr.contains("Connection refused")
            || stderr.contains("timed out")
            || stderr.contains("No route to host")
        {
            anyhow::bail!("CloudPC unreachable — cannot connect to '{CLOUDPC_HOST}' via SSH");
        }
        anyhow::bail!("CloudPC script failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let filtered = stdout
        .lines()
        .filter(|l| {
            !l.contains("WARNING:")
                && !l.contains("vulnerable")
                && !l.contains("upgraded")
                && !l.contains("security fix")
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(filtered)
}
