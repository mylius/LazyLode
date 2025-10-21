use crate::database::SSHConfig;
use anyhow::Result;
use ssh2::Session;
use std::net::TcpStream;
use std::sync::Arc;
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
        listener: TcpListener,
    ) -> Result<()> {
        loop {
            let (mut local_stream, _) = listener.accept().await?;
            
            // Spawn a task to handle each connection
            let ssh_config = ssh_config.clone();
            let remote_host = remote_host.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(ssh_config, remote_host, remote_port, local_stream).await {
                    let _ = crate::logging::error(&format!("SSH tunnel connection error: {}", e));
                }
            });
        }
    }
    
    async fn handle_connection(
        ssh_config: SSHConfig,
        remote_host: String,
        remote_port: u16,
        mut local_stream: tokio::net::TcpStream,
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
        
        // Forward data between local and remote
        let (mut local_read, mut local_write) = local_stream.split();
        let (mut remote_read, mut remote_write) = channel.split();
        
        // Forward data in both directions
        let local_to_remote = tokio::io::copy(&mut local_read, &mut remote_write);
        let remote_to_local = tokio::io::copy(&mut remote_read, &mut local_write);
        
        tokio::select! {
            _ = local_to_remote => {},
            _ = remote_to_local => {},
        }
        
        Ok(())
    }
    
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
    
    pub async fn stop(self) -> Result<()> {
        // The handle will be dropped and the task will be cancelled
        Ok(())
    }
}