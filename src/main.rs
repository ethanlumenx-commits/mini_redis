use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统（WorkerGuard 必须保留到 main 结束，保证日志落盘）
    let _guard = mini_redis::logger::init_logger();

    info!("Starting mini-redis server");

    // 启动服务器：默认 127.0.0.1:6379
    mini_redis::server::run("127.0.0.1:6379").await?;

    Ok(())
}
