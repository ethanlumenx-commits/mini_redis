Mini-Redis
基于 Rust + Tokio 从零实现的极简高并发类 Redis 内存数据库服务。
Rust 异步运行时、网络编程、并发安全、事件驱动服务架构

---
✨ 简介
- 纯 Tokio 异步 TCP 服务 — 支持多客户端高并发连接处理
- 自研 RESP 协议解析器 — 手动实现 Redis 有线通信协议
- 高性能并发状态管理 — 基于 Arc + RwLock 实现线程安全内存存储
- 原生发布订阅(Pub/Sub)能力 — 依托 tokio::sync::broadcast 实现事件推送
- 支持 Redis 管道机制 — 异步流处理，适配批量区块、交易数据处理场景
- 完全兼容官方 redis-cli 客户端

---
📦 技术栈
- 开发语言：Rust 最新稳定版
- 异步运行时：Tokio（网络、同步、IO 模块）
- 通信协议：RESP（Redis 序列化协议）
- 并发方案：Tokio 任务调度、广播通道、读写锁、原子引用计数

---
✅ 已实现功能
1. 核心 TCP 服务
- 基于 Tokio 实现异步 TcpListener 端口绑定与连接监听
- 通过 tokio::spawn 实现多客户端并发独立处理
- 全程非阻塞 IO 流处理，保障高并发性能
2. RESP 协议解析
- 手动解析 RESP 数组、批量字符串核心协议格式
- 实现字节流缓冲区高效读写与解析
- 完美适配官方 Redis 客户端请求规范
3. 基础 Redis 命令
- PING — 服务健康状态检测
- SET key value — 键值对数据写入
- GET key — 键值对数据读取
- DEL key — 指定键数据删除
4. 并发安全内存存储
- 基于 Arc<RwLock<HashMap>> 实现全局共享内存数据库
- 读写分离锁机制，兼顾并发安全与读写性能