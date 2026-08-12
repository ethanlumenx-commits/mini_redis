use anyhow::{bail, Context, Result};
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::command::Command;
use crate::db::Db;
use crate::frame::{parse_frame, RespFrame};

/// 单个连接的缓冲区上限：超过就断开，防止内存无限增长
const MAX_BUFFER_BYTES: usize = 64 * 1024 * 1024; // 64MB

/// 启动 mini-redis 服务器，监听指定地址
pub async fn run(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {}", addr))?;
    info!("mini-redis listening on {}", addr);

    let db = crate::db::new();

    loop {
        let (stream, addr) = listener
            .accept()
            .await
            .context("failed to accept connection")?;
        info!("new client connected: {}", addr);

        // 给新连接分配一个数据库克隆
        let db_clone = db.clone();

        // 扔到后台处理
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, db_clone).await {
                error!("client {} error: {}", addr, e);
            }
            info!("client {} disconnected", addr);
        });
    }
}

/// 处理单个客户端连接
async fn handle_client(mut stream: TcpStream, db: Db) -> Result<()> {
    let mut buf = BytesMut::with_capacity(4096);

    loop {
        // 读入数据
        let n = stream
            .read_buf(&mut buf)
            .await
            .context("read from socket failed")?;

        if n == 0 {
            return Ok(()); // 客户端主动关闭
        }

        info!("read {} bytes, buf len: {}", n, buf.len());

        // 缓冲区防暴增
        if buf.len() > MAX_BUFFER_BYTES {
            warn!(
                "client buffer exceeded {} bytes, disconnecting",
                MAX_BUFFER_BYTES
            );
            let resp = RespFrame::Error("ERR buffer limit exceeded".into());
            let _ = write_response(&mut stream, &resp).await;
            bail!("buffer limit exceeded");
        }

        // 消费所有可解析的帧
        while let Some(frame) = parse_frame(&mut buf) {
            info!("parsed frame: {}", frame);
            let response = match Command::parse(frame) {
                Ok(cmd) => cmd.execute(&db).await,
                Err(msg) => RespFrame::Error(format!("ERR {}", msg)),
            };
            write_response(&mut stream, &response).await?;
        }
    }
}

/// 将响应帧序列化并写入 socket，确保刷新
async fn write_response(stream: &mut TcpStream, resp: &RespFrame) -> Result<()> {
    let mut out = BytesMut::new();
    resp.serialize(&mut out);
    stream
        .write_all(&out)
        .await
        .context("write to socket failed")?;
    stream.flush().await.context("flush socket failed")?;
    Ok(())
}
