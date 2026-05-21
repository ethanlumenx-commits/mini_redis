use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error};

mod logger;

#[tokio::main]  
async fn main()->anyhow::Result<()> {
    // 初始化日志系统
    let _guard = logger::init_logger();
    
    info!("Starting mini-redis server");

    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    info!("Listening on {}", listener.local_addr()?);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        info!("Accepted connection from {}", addr);

        tokio::task::spawn(async move {
            info!("Handling connection from {}", addr);
            loop {
                let mut buf = vec![0; 1024];
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        info!("Connection closed");
                    }
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buf[..n]);
                        info!("Received {} bytes: {}", n, data);
                        
                        
                        if data.starts_with("PING") {
                            info!("Received PING request");
                            let response = b"+PONG\r\n";

                            if let Err(e) = socket.write_all(response).await {
                                error!("Failed to write to socket: {}", e);
                                break;
                            }
                        } else {
                            let response = b"+OK\r\n";
                            if let Err(e) = socket.write_all(response).await {
                                error!("Failed to write to socket: {}", e);
                                break;
                            }
                        }

                    }
                    Err(e) => {
                        info!("Failed to read from socket: {}", e);
                        break;
                    }
                }
            }
        });
    
    }
}