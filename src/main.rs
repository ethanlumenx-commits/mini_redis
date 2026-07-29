use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod logger;




#[tokio::main]  
async fn main() {
    // 初始化日志系统
    let _guard = logger::init_logger();
    
    info!("Starting mini-redis server");

}

