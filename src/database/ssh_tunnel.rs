use anyhow::{anyhow, Context, Result};
use std::net::{SocketAddr, TcpListener};
use tokio::process::{Child, Command};
use tokio::time::{sleep, Duration};

use super::SSHConfig;

pub struct SshTunnelProcess {
    pub local_port: u16,
    child: Child,
}

impl SshTunnelProcess {
    pub async fn start(
        ssh: &SSHConfig,
        target_host: &str,
        target_port: u16,
    ) -> Result<SshTunnelProcess> {
        let local_port = allocate_free_local_port()?;

        let mut args: Vec<String> = vec![
            "-N".into(),
            "-L".into(),
            format!("{}:{}:{}", local_port, target_host, target_port),
            "-p".into(),
            ssh.port.to_string(),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
        ];

        if let Some(key_path) = &ssh.private_key_path {
            if !key_path.is_empty() {
                args.push("-i".into());
                args.push(key_path.clone());
            }
        }

        // Build user@host target
        let user_at_host = if !ssh.username.is_empty() {
            format!("{}@{}", ssh.username, ssh.host)
        } else {
            ssh.host.clone()
        };
        args.push(user_at_host);

        let mut cmd = Command::new("ssh");
        cmd.args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().context("failed to spawn ssh process")?;

        // Give ssh some time to bind and establish forwarding; also detect early exit
        for _ in 0..10u8 {
            if let Some(status) = child.try_wait()? {
                return Err(anyhow!("ssh process exited early with status: {}", status));
            }
            sleep(Duration::from_millis(100)).await;
        }

        Ok(SshTunnelProcess { local_port, child })
    }

    pub async fn stop(&mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }
}

fn allocate_free_local_port() -> Result<u16> {
    let addr: SocketAddr = "127.0.0.1:0"
        .parse()
        .context("failed to parse local socket address")?;
    let listener = TcpListener::bind(addr).context("failed to bind local ephemeral port")?;
    let local_port = listener.local_addr()?.port();
    drop(listener);
    Ok(local_port)
}
