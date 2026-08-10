use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error};
use bytes::{BytesMut,Buf};
use std::sync::{Arc,RwLock};
use std::collections::HashMap;

mod logger;

// type Db = Arc<RwLock<HashMap<String, String>>>;




#[tokio::main]  
async fn main() {
    // 初始化日志系统
    let _guard = logger::init_logger();
    
    info!("Starting mini-redis server");

    let listener = TcpListener::bind("127.0.0.1:6379").await.expect("TcpListener.bind is error");
    loop{
        // 阻塞并等待下一个连接
        let (stream, _addr) = listener.accept().await.expect("TcpListener.accept is error");
        tokio::spawn(async { handle_client(stream).await });
    }

}

async fn handle_client(mut stream: TcpStream)  {
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
                while let Some(pos) = buf.windows(2).position(|windows|windows == b"\r\n"){
                    info!("Received CRLF at position {}", pos);
                    let data = parse_frame(&mut buf);
                    info!("Received frame: {:?}", data);


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
    // SimpleString(String),
    BulkString(Option<String>),
    Array(Vec<RespFrame>),
    // Error(String),

}

// 读取以 \r\n 结尾的行数据，返回行数据，删除 \r\n
pub fn read_line(buf:&mut BytesMut)->Option<Vec<u8>>{
    if let Some(pos) = buf.windows(2).position(|window| window == b"\r\n") {
        let content =buf.split_to(pos);
        buf.advance(2);
        Some(content.to_vec())
    }else{
        None
    }
}


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
