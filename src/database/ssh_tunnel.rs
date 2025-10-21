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
        listener: TcpListener,
    ) -> Result<()> {
        // Connect to SSH server
        let tcp = TcpStream::connect(format!("{}:{}", ssh_config.host, ssh_config.port))?;
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;
        
        // Authenticate
        if let Some(private_key_path) = &ssh_config.private_key_path {
            session.userauth_pubkey_file(&ssh_config.username, None, std::path::Path::new(private_key_path), None)?;
        } else if let Some(password) = &ssh_config.password {
            session.userauth_password(&ssh_config.username, password)?;
        } else {
            return Err(anyhow::anyhow!("No authentication method provided for SSH tunnel"));
        }
        
        // Accept connections and forward them
        loop {
            let (local_stream, _) = listener.accept().await?;
            
            // Create a new SSH channel for each connection
            let mut channel = session.channel_direct_tcpip(&remote_host, remote_port, None)?;
            
            // Forward data between local and remote connections
            tokio::spawn(async move {
                if let Err(e) = Self::forward_data(local_stream, channel).await {
                    let _ = crate::logging::error(&format!("SSH tunnel forwarding error: {}", e));
                }
            });
        }
    }
    
    async fn forward_data(
        mut local_stream: tokio::net::TcpStream,
        channel: ssh2::Channel,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use std::io::{Read, Write};
        
        let (mut local_read, mut local_write) = local_stream.split();
        
        // Forward data from local to remote
        let local_to_remote = async {
            let mut buffer = [0; 4096];
            loop {
                match local_read.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        // Use blocking task for SSH channel operations
                        if let Err(e) = tokio::task::spawn_blocking({
                            let channel = channel.clone();
                            let data = buffer[..n].to_vec();
                            move || {
                                let mut channel = channel;
                                channel.write_all(&data)?;
                                channel.flush()?;
                                Ok::<(), std::io::Error>(())
                            }
                        }).await.unwrap() {
                            let _ = crate::logging::error(&format!("Error writing to SSH channel: {}", e));
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = crate::logging::error(&format!("Error reading from local stream: {}", e));
                        break;
                    }
                }
            }
        };
        
        // Forward data from remote to local
        let remote_to_local = async {
            let mut buffer = [0; 4096];
            loop {
                // Use blocking task for SSH channel operations
                let result = tokio::task::spawn_blocking({
                    let channel = channel.clone();
                    move || {
                        let mut channel = channel;
                        channel.read(&mut buffer)
                    }
                }).await.unwrap();
                
                match result {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Err(e) = local_write.write_all(&buffer[..n]).await {
                            let _ = crate::logging::error(&format!("Error writing to local stream: {}", e));
                            break;
                        }
                        if let Err(e) = local_write.flush().await {
                            let _ = crate::logging::error(&format!("Error flushing local stream: {}", e));
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = crate::logging::error(&format!("Error reading from SSH channel: {}", e));
                        break;
                    }
                }
            }
        };
        
        // Run both directions concurrently
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