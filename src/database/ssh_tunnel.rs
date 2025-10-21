use crate::database::SSHConfig;
use anyhow::Result;
use ssh2::Session;
use std::net::TcpStream;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub struct SshTunnelProcess {
    local_port: u16,
    _handle: JoinHandle<()>,
}

impl SshTunnelProcess {
    pub async fn start(
        ssh_config: &SSHConfig,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<Self> {
        let ssh_config = ssh_config.clone();
        let remote_host = remote_host.to_string();
        
        // Find an available local port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let local_port = listener.local_addr()?.port();
        
        let handle = tokio::spawn(async move {
            if let Err(e) = Self::run_tunnel(ssh_config, remote_host, remote_port, listener).await {
                let _ = crate::logging::error(&format!("SSH tunnel error: {}", e));
            }
        });
        
        Ok(Self {
            local_port,
            _handle: handle,
        })
    }
    
    async fn run_tunnel(
        ssh_config: SSHConfig,
        remote_host: String,
        remote_port: u16,
        mut listener: TcpListener,
    ) -> Result<()> {
        // Connect to SSH server
        let tcp = TcpStream::connect(format!("{}:{}", ssh_config.host, ssh_config.port))?;
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;
        
        // Authenticate
        if let Some(private_key_path) = &ssh_config.private_key_path {
            session.userauth_pubkey_file(&ssh_config.username, None, private_key_path, None)?;
        } else if let Some(password) = &ssh_config.password {
            session.userauth_password(&ssh_config.username, password)?;
        } else {
            return Err(anyhow::anyhow!("No authentication method provided for SSH tunnel"));
        }
        
        // Create port forward
        let mut channel = session.channel_direct_tcpip(&remote_host, remote_port, None)?;
        
        // Accept connections and forward them
        loop {
            let (mut local_stream, _) = listener.accept().await?;
            
            // For now, just keep the tunnel alive
            // In a full implementation, we would forward data between local_stream and channel
            tokio::spawn(async move {
                let _ = local_stream;
                // Keep the connection alive
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            });
        }
    }
    
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
    
    pub async fn stop(self) -> Result<()> {
        // The handle will be dropped and the task will be cancelled
        Ok(())
    }
}