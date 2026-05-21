use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use std::fs::OpenOptions;
use tracing_appender::non_blocking::WorkerGuard;

pub fn init_logger() -> WorkerGuard {
    // 从环境变量 RUST_LOG 读取日志级别，如果未设置则默认为 info
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    // 创建日志目录（如果不存在）
    std::fs::create_dir_all("logs").expect("Failed to create logs directory");
    
    // 以追加模式打开日志文件（如果不存在则创建）
    let file = OpenOptions::new()
        .create(true)    // 文件不存在时创建
        .append(true)    // 追加模式
        .open("logs/mini_redis.log")
        .expect("Failed to open log file");
    
    // 创建非阻塞的文件写入器
    let (non_blocking, guard) = tracing_appender::non_blocking(file);
    
    // 配置文件输出层
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);  // 文件中不使用 ANSI 颜色代码
    
    // 组合控制台和文件输出
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer())  // 控制台输出
        .with(file_layer)    // 文件输出
        .init();
    
    guard
}