use crate::db::Db;
use crate::frame::RespFrame;

/// 解析出的 Redis 命令
#[derive(Debug)]
pub enum Command {
    Ping(Option<String>),
    Echo(String),
    Get { key: String },
    Set { key: String, value: Vec<u8> },
    Del { key: String },
    Exists { key: String },
}

impl Command {
    /// 从 RespFrame 数组中解析命令。
    /// 返回 Ok(cmd) 成功，Err(msg) 表示协议/参数错误。
    pub fn parse(frame: RespFrame) -> Result<Self, String> {
        let arr = match frame {
            RespFrame::Array(Some(a)) => a,
            _ => return Err("expected array frame".into()),
        };
        let mut iter = arr.into_iter();

        // 第一个元素：命令名（BulkString）
        let cmd_name = match iter.next() {
            Some(RespFrame::BulkString(Some(b))) => {
                String::from_utf8(b).map_err(|_| "invalid command utf8")?.to_uppercase()
            }
            _ => return Err("invalid command frame".into()),
        };

        match cmd_name.as_str() {
            "PING" => {
                let arg = match iter.next() {
                    Some(RespFrame::BulkString(Some(b))) => {
                        Some(String::from_utf8(b).map_err(|_| "invalid ping arg utf8")?)
                    }
                    Some(_) => return Err("invalid ping argument".into()),
                    None => None,
                };
                Ok(Command::Ping(arg))
            }
            "ECHO" => {
                let msg = next_bulk_string(&mut iter, "ECHO")?;
                Ok(Command::Echo(msg))
            }
            "GET" => {
                let key = next_bulk_string(&mut iter, "GET key")?;
                Ok(Command::Get { key })
            }
            "SET" => {
                let key = next_bulk_string(&mut iter, "SET key")?;
                let value = next_bulk_bytes(&mut iter, "SET value")?;
                Ok(Command::Set { key, value })
            }
            "DEL" => {
                let key = next_bulk_string(&mut iter, "DEL key")?;
                Ok(Command::Del { key })
            }
            "EXISTS" => {
                let key = next_bulk_string(&mut iter, "EXISTS key")?;
                Ok(Command::Exists { key })
            }
            other => Err(format!("unknown command '{}'", other)),
        }
    }

    /// 执行命令并返回 RESP 响应帧
    pub async fn execute(self, db: &Db) -> RespFrame {
        match self {
            Command::Ping(msg) => match msg {
                Some(s) => RespFrame::BulkString(Some(s.into_bytes())),
                None => RespFrame::SimpleString("PONG".into()),
            },
            Command::Echo(msg) => RespFrame::BulkString(Some(msg.into_bytes())),
            Command::Get { key } => match db.get(&key).await {
                Some(v) => RespFrame::BulkString(Some(v)),
                None => RespFrame::BulkString(None), // Redis 规范：key 不存在返回 null bulk string
            },
            Command::Set { key, value } => {
                db.set(key, value).await;
                RespFrame::SimpleString("OK".into())
            }
            Command::Del { key } => {
                // 规范：DEL 返回被删除 key 的数量（整数）
                let removed = if db.del(&key).await { 1 } else { 0 };
                RespFrame::Integer(removed)
            }
            Command::Exists { key } => {
                let exists = if db.exists(&key).await { 1 } else { 0 };
                RespFrame::Integer(exists)
            }
        }
    }
}

/// 辅助：从迭代器中取下一个 BulkString 转成 String
fn next_bulk_string(
    iter: &mut impl Iterator<Item = RespFrame>,
    name: &str,
) -> Result<String, String> {
    let bytes = next_bulk_bytes(iter, name)?;
    String::from_utf8(bytes).map_err(|_| format!("{}: invalid utf8", name))
}

/// 辅助：从迭代器中取下一个 BulkString 返回原始字节
fn next_bulk_bytes(
    iter: &mut impl Iterator<Item = RespFrame>,
    name: &str,
) -> Result<Vec<u8>, String> {
    match iter.next() {
        Some(RespFrame::BulkString(Some(b))) => Ok(b),
        Some(RespFrame::BulkString(None)) => Err(format!("{}: expected non-null value", name)),
        Some(_) => Err(format!("{}: expected bulk string", name)),
        None => Err(format!("{}: missing argument", name)),
    }
}
