use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error};
use bytes::{BytesMut,Buf};
use std::sync::{Arc};
use tokio::sync::RwLock;

use std::collections::HashMap;

mod logger;

type Db = Arc<RwLock<HashMap<String, String>>>;




#[tokio::main]  
async fn main() {
    // 初始化日志系统
    let _guard = logger::init_logger();
    
    info!("Starting mini-redis server");

    let listener = TcpListener::bind("127.0.0.1:6379").await.expect("TcpListener.bind is error");

    let db = Arc::new(RwLock::new(HashMap::new()));
    loop{
        // 阻塞并等待下一个连接
        let (stream, _addr) = listener.accept().await.expect("TcpListener.accept is error");
        let db_clone = db.clone();
        tokio::spawn(async move{ handle_client(stream, db_clone).await });
    }

}

// 不断从缓存区读取数据，解析并处理，直到客户端关闭
async fn handle_client(mut stream: TcpStream, db: Db)  {
    // 创建一个可变的字节缓冲区
    let mut buf = BytesMut::with_capacity(1024);

    loop{
        // 从流中读取数据到缓冲区
        match stream.read_buf(&mut buf).await {
            // 字节为0表示客户端关闭
            Ok(n) if n == 0 => {
                info!("Client closed");
                break;
            }
            Ok(n) => {
                info!("Received {} bytes,buf total length {}", n, buf.len());


                // 处理缓冲区中的数据
                while let Some(data) =  parse_frame(&mut buf){
                    info!("Received frame: {:?}", data);
                    let response = handle_frame(data, &db).await;
                    // 根据返回的RespFrame手动拼接RESP字节，临时测试用
                    let send_buf = match response {
                        RespFrame::SimpleString(s) => format!("+{s}\r\n"),
                        RespFrame::Error(s) => format!("-{s}\r\n"),
                        RespFrame::BulkString(Some(value)) => format!("${}\r\n{value}\r\n", value.len()),
                        RespFrame::BulkString(None) => "$-1\r\n".to_string(),
                        RespFrame::Array(_) => "-ERR unsupported array response\r\n".to_string(),
                    };

                    // 写回客户端
                    if let Err(e) = stream.write_all(send_buf.as_bytes()).await {
                        error!("send response error: {}", e);
                        break;
                    }

                }

                

            }
            Err(e) => {
                error!("Failed to read from socket: {}", e);
                break;
            }
        }
    }
}


#[derive(Debug)]
enum RespFrame{
    SimpleString(String),
    BulkString(Option<String>),
    Array(Vec<RespFrame>),
    Error(String),

}



// 解析 RESP 框架
pub async fn handle_frame(frame: RespFrame, db: &Db) -> RespFrame{
    // match frame
    match frame {
        // 如果是respframe类型的数组
        RespFrame::Array(items) => {
            // 转为迭代器
            let mut iter = items.into_iter();
            // 获取第一个元素并匹配
            let cmd = match iter.next(){
                // 先匹配 BulkString，再匹配BulkString里的Option,转为大写
                Some(RespFrame::BulkString(Some(s))) => s.to_uppercase(),
                _ => return RespFrame::Error("Invalid frame".to_string()),
            };

            match cmd.as_str() {
                "GET" =>{
                    // 获取 key and_then 处理万一没有值的情况
                    let key_opt = iter.next().and_then(|frame| match frame {
                        RespFrame::BulkString(s) => s.clone(),
                        _ => return None,
                    });
                    // 用第一个andthen返回的Option《String》进行下一轮判断
                    let key = match key_opt {
                        Some(key) => key,
                        None => return RespFrame::Error("Invalid key".to_string()),
                    };

                    // 异步读锁
                    let db = db.read().await;
                    match db.get(&key){
                        Some(value) => return RespFrame::SimpleString(value.clone()),
                        None => return RespFrame::Error("Key not found".to_string()),
                    }

                }
                "SET" =>{
                    // 获取 key and_then 处理万一没有值的情况
                    let key = iter.next().and_then(|frame| match frame {
                        RespFrame::BulkString(s) => s.clone(),
                        _ => return None,
                    });
                    // 获取 value and_then 处理万一没有值的情况
                    let value = iter.next().and_then(|frame| match frame {
                        RespFrame::BulkString(s) => s.clone(),
                        _ => return None,
                    });

                    // insert key and value
                    if let (Some(k), Some(v)) = (key, value) {
                        let mut db = db.write().await;
                        db.insert(k, v);
                        return RespFrame::SimpleString("OK".to_string());
                    }
                    return RespFrame::Error("Invalid key or value".to_string());

                }

                "DEL" =>{
                    // 获取 key and_then 处理万一没有值的情况
                    let key = iter.next().and_then(|frame| match frame {
                        RespFrame::BulkString(s) => s.clone(),
                        _ => return None,
                    });

                    // delete key
                    if let Some(key) = key {
                        let mut db = db.write().await;
                        db.remove(&key);
                        return RespFrame::SimpleString("OK".to_string());
                    }
                    return RespFrame::Error("Invalid key".to_string());
                }
                _ => return RespFrame::Error("Invalid command".to_string()),
            }

            
        },
        _ => {},
        
    }
    RespFrame::Error("dead_code".to_string())
}

// 读取以 \r\n 结尾的行数据，返回行数据，删除 \r\n  return b'123' -> Some([49,50,51])
pub fn read_line(buf:&mut BytesMut)->  Option<Vec<u8>>{
    if let Some(pos) = buf.windows(2).position(|window| window == b"\r\n") {
        let content =buf.split_to(pos);
        buf.advance(2);
        Some(content.to_vec())
    }else{
        None
    }
}

// 解析 RESP 框架 return Some(Array([BulkString(Some("i")), BulkString(Some("have")), BulkString(Some("a"))
pub fn parse_frame(buf:&mut BytesMut) -> Option<RespFrame>{
    let first = buf.first();
    if let Some(first) = first {
        match first {
            b'*' => {
                buf.advance(1);
                // return the length of *, if None:return
                let resp = read_line(buf)?;

                // get the count of behind *
                let count = std::str::from_utf8(&resp)
                    .ok()?
                    .parse::<usize>()
                    .ok()?;
                
                // crate a vec to store the children
                let mut children = Vec::with_capacity(count);

                for _ in 0..count {
                    children.push(parse_frame(buf)?);
                }

                Some(RespFrame::Array(children))
            },

            b'$' => {
                buf.advance(1);
                // return the length of $, if None:return
                let resp = read_line(buf)?;
                // 字符数量
                let len = std::str::from_utf8(&resp)
                    .ok()?
                    .parse::<isize>()
                    .ok()?;
                
                if len == -1 {
                    return Some(RespFrame::BulkString(None));
                } 

                let len = len as usize;

                //  比较当前行字符长度，如果小于返回的 len  + \r\n 就说明返回的不完整
                if buf.len() < len + 2 {
                    return None;
                }

                // 获取数据
                let data = buf.split_to(len);

                // 跳过 \r\n
                buf.advance(2);

                let s = String::from_utf8_lossy(&data);
                Some(RespFrame::BulkString(Some(s.to_string())))

            }


            _ => None,
        }
    } else {
        None
    }
}
