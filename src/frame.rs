use bytes::{Buf, BytesMut};
use std::fmt;

/// RESP 协议中的一帧数据
#[derive(Debug, Clone, PartialEq)]
pub enum RespFrame {
    /// 简单字符串：+OK\r\n
    SimpleString(String),
    /// 错误信息：-ERR message\r\n
    Error(String),
    /// 整数：:1000\r\n
    Integer(i64),
    /// 批量字符串：$6\r\nfoobar\r\n  或  $-1\r\n (null)
    BulkString(Option<Vec<u8>>),
    /// 数组：*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n  或  *-1\r\n (null)
    Array(Option<Vec<RespFrame>>),
}

impl fmt::Display for RespFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RespFrame::SimpleString(s) => write!(f, "SimpleString({:?})", s),
            RespFrame::Error(s) => write!(f, "Error({:?})", s),
            RespFrame::Integer(i) => write!(f, "Integer({})", i),
            RespFrame::BulkString(Some(b)) => {
                write!(f, "BulkString({:?})", String::from_utf8_lossy(b))
            }
            RespFrame::BulkString(None) => write!(f, "BulkString(null)"),
            RespFrame::Array(Some(a)) => write!(f, "Array({:?})", a),
            RespFrame::Array(None) => write!(f, "Array(null)"),
        }
    }
}

impl RespFrame {
    /// 将 RespFrame 序列化为 RESP 协议字节，写入 BytesMut
    pub fn serialize(&self, dst: &mut BytesMut) {
        match self {
            RespFrame::SimpleString(s) => {
                dst.extend_from_slice(b"+");
                dst.extend_from_slice(s.as_bytes());
                dst.extend_from_slice(b"\r\n");
            }
            RespFrame::Error(s) => {
                dst.extend_from_slice(b"-");
                dst.extend_from_slice(s.as_bytes());
                dst.extend_from_slice(b"\r\n");
            }
            RespFrame::Integer(i) => {
                dst.extend_from_slice(b":");
                dst.extend_from_slice(i.to_string().as_bytes());
                dst.extend_from_slice(b"\r\n");
            }
            RespFrame::BulkString(Some(data)) => {
                dst.extend_from_slice(b"$");
                dst.extend_from_slice(data.len().to_string().as_bytes());
                dst.extend_from_slice(b"\r\n");
                dst.extend_from_slice(data);
                dst.extend_from_slice(b"\r\n");
            }
            RespFrame::BulkString(None) => {
                dst.extend_from_slice(b"$-1\r\n");
            }
            RespFrame::Array(Some(items)) => {
                dst.extend_from_slice(b"*");
                dst.extend_from_slice(items.len().to_string().as_bytes());
                dst.extend_from_slice(b"\r\n");
                for item in items {
                    item.serialize(dst);
                }
            }
            RespFrame::Array(None) => {
                dst.extend_from_slice(b"*-1\r\n");
            }
        }
    }

    /// 辅助：如果是 BulkString(Some)，返回字节切片
    pub fn as_bulk(&self) -> Option<&[u8]> {
        match self {
            RespFrame::BulkString(Some(b)) => Some(b),
            _ => None,
        }
    }
}

/// 在不可变切片上"尝试"解析一帧。
/// 成功：Some((frame, consumed_bytes))
/// 数据不足：None
/// 协议错误：当前保守地当作数据不足（返回 None），更严谨可拆分成 Result
fn parse_bytes(src: &[u8]) -> Option<(RespFrame, usize)> {
    if src.is_empty() {
        return None;
    }
    let first = src[0];
    let mut consumed = 1usize;
    let rest = &src[1..];

    let frame = match first {
        b'+' => {
            let (line, n) = read_line_from(rest)?;
            consumed += n;
            let s = String::from_utf8_lossy(line).into_owned();
            RespFrame::SimpleString(s)
        }
        b'-' => {
            let (line, n) = read_line_from(rest)?;
            consumed += n;
            let s = String::from_utf8_lossy(line).into_owned();
            RespFrame::Error(s)
        }
        b':' => {
            let (line, n) = read_line_from(rest)?;
            consumed += n;
            let n_str = std::str::from_utf8(line).ok()?;
            let v = n_str.parse::<i64>().ok()?;
            RespFrame::Integer(v)
        }
        b'$' => {
            let (line, n) = read_line_from(rest)?;
            consumed += n;
            let len_str = std::str::from_utf8(line).ok()?;
            let len = len_str.parse::<isize>().ok()?;
            if len == -1 {
                RespFrame::BulkString(None)
            } else if len < 0 {
                return None;
            } else {
                let len = len as usize;
                let payload = &src[consumed..];
                if payload.len() < len + 2 {
                    return None;
                }
                let data = payload[..len].to_vec();
                consumed += len + 2;
                RespFrame::BulkString(Some(data))
            }
        }
        b'*' => {
            let (line, n) = read_line_from(rest)?;
            consumed += n;
            let len_str = std::str::from_utf8(line).ok()?;
            let len = len_str.parse::<isize>().ok()?;
            if len == -1 {
                RespFrame::Array(None)
            } else if len < 0 {
                return None;
            } else {
                let len = len as usize;
                let mut children = Vec::with_capacity(len);
                let mut inner_src = &src[consumed..];
                for _ in 0..len {
                    let (child, n) = parse_bytes(inner_src)?;
                    children.push(child);
                    inner_src = &inner_src[n..];
                    consumed += n;
                }
                RespFrame::Array(Some(children))
            }
        }
        // 未知前缀：协议错误，返回 None（调用方会保留字节等待更多数据，但实际永远解析不出）
        _ => return None,
    };

    Some((frame, consumed))
}

/// 从切片中读取到下一个 \r\n，返回 (line_slice_without_crlf, total_bytes_including_crlf)
fn read_line_from(src: &[u8]) -> Option<(&[u8], usize)> {
    let pos = src.windows(2).position(|w| w == b"\r\n")?;
    Some((&src[..pos], pos + 2))
}

/// 从 BytesMut 中解析出一帧完整的 RESP 数据。
/// 只有确认能解析出完整帧时才会从 buf 中移除对应字节。
/// 数据不足时返回 None，buf 保持不变。
pub fn parse_frame(buf: &mut BytesMut) -> Option<RespFrame> {
    let (frame, n) = parse_bytes(buf.chunk())?;
    buf.advance(n);
    Some(frame)
}
