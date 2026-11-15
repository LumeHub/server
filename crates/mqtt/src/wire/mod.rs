//! Minimal MQTT 3.1.1 codec: exactly the packets a LumeHub client sends and
//! receives. Encoders return complete frames; [`decode`] parses one frame
//! from the front of a buffer and reports how many bytes it consumed.

/// Packets a broker sends to us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    ConnAck {
        code: u8,
    },
    Publish {
        topic: String,
        payload: Vec<u8>,
        pid: Option<u16>,
    },
    PubAck {
        pid: u16,
    },
    SubAck {
        pid: u16,
    },
    PingResp,
    /// Valid frame of a type we don't act on (PUBREC, UNSUBACK, ...).
    Other {
        packet_type: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    MalformedLength,
    Malformed(&'static str),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedLength => write!(f, "malformed remaining length"),
            Self::Malformed(what) => write!(f, "malformed packet: {what}"),
        }
    }
}

impl std::error::Error for WireError {}

pub fn connect(
    client_id: &str,
    keep_alive_secs: u16,
    clean_session: bool,
    username: Option<&str>,
    password: Option<&str>,
) -> Vec<u8> {
    let mut flags = 0u8;
    if clean_session {
        flags |= 0b0000_0010;
    }
    if username.is_some() {
        flags |= 0b1000_0000;
    }
    if password.is_some() {
        flags |= 0b0100_0000;
    }
    let mut body = Vec::new();
    put_str(&mut body, "MQTT");
    body.push(4); // protocol level 4 = MQTT 3.1.1
    body.push(flags);
    body.extend_from_slice(&keep_alive_secs.to_be_bytes());
    put_str(&mut body, client_id);
    if let Some(u) = username {
        put_str(&mut body, u);
    }
    if let Some(p) = password {
        put_str(&mut body, p);
    }
    frame(0x10, &body)
}

/// `pid` is `Some` for QoS 1, `None` for QoS 0.
pub fn publish(topic: &str, payload: &[u8], retain: bool, pid: Option<u16>) -> Vec<u8> {
    let mut header = 0x30;
    if pid.is_some() {
        header |= 0b0010;
    }
    if retain {
        header |= 0b0001;
    }
    let mut body = Vec::with_capacity(topic.len() + payload.len() + 4);
    put_str(&mut body, topic);
    if let Some(pid) = pid {
        body.extend_from_slice(&pid.to_be_bytes());
    }
    body.extend_from_slice(payload);
    frame(header, &body)
}

pub fn subscribe(pid: u16, topics: &[&str], qos: u8) -> Vec<u8> {
    let mut body = pid.to_be_bytes().to_vec();
    for t in topics {
        put_str(&mut body, t);
        body.push(qos);
    }
    frame(0x82, &body)
}

pub fn puback(pid: u16) -> Vec<u8> {
    frame(0x40, &pid.to_be_bytes())
}

pub fn pingreq() -> Vec<u8> {
    vec![0xC0, 0]
}

pub fn disconnect() -> Vec<u8> {
    vec![0xE0, 0]
}

/// Total length of the first frame in `buf`, as soon as its header is
/// complete, even if the body hasn't fully arrived. Lets readers reject
/// oversized packets before buffering them.
pub fn frame_len(buf: &[u8]) -> Result<Option<usize>, WireError> {
    Ok(frame_bounds(buf)?.map(|(_, end)| end))
}

/// `(body_start, frame_end)` of the first frame, once its header is complete.
fn frame_bounds(buf: &[u8]) -> Result<Option<(usize, usize)>, WireError> {
    if buf.is_empty() {
        return Ok(None);
    }
    Ok(remaining_length(&buf[1..])?.map(|(len, len_bytes)| (1 + len_bytes, 1 + len_bytes + len)))
}

/// Decode the first frame in `buf`. `Ok(None)` means more bytes are needed.
pub fn decode(buf: &[u8]) -> Result<Option<(Incoming, usize)>, WireError> {
    let Some((start, end)) = frame_bounds(buf)? else {
        return Ok(None);
    };
    if buf.len() < end {
        return Ok(None);
    }
    let header = buf[0];
    let body = &buf[start..end];
    let packet = match header >> 4 {
        2 => Incoming::ConnAck {
            code: *body.get(1).ok_or(WireError::Malformed("connack"))?,
        },
        3 => decode_publish(header, body)?,
        4 => Incoming::PubAck {
            pid: read_u16(body, 0, "puback")?,
        },
        9 => Incoming::SubAck {
            pid: read_u16(body, 0, "suback")?,
        },
        13 => Incoming::PingResp,
        t => Incoming::Other { packet_type: t },
    };
    Ok(Some((packet, end)))
}

fn decode_publish(header: u8, body: &[u8]) -> Result<Incoming, WireError> {
    let topic_len = read_u16(body, 0, "publish topic")? as usize;
    let topic_end = 2 + topic_len;
    let topic = body
        .get(2..topic_end)
        .ok_or(WireError::Malformed("publish topic"))?;
    let topic =
        String::from_utf8(topic.to_vec()).map_err(|_| WireError::Malformed("topic utf8"))?;
    let qos = (header >> 1) & 0b11;
    let (pid, payload_start) = if qos > 0 {
        (
            Some(read_u16(body, topic_end, "publish pid")?),
            topic_end + 2,
        )
    } else {
        (None, topic_end)
    };
    Ok(Incoming::Publish {
        topic,
        payload: body[payload_start..].to_vec(),
        pid,
    })
}

/// Variable-length "remaining length": 7 bits per byte, at most 4 bytes.
/// Returns `(value, bytes_used)`, or `None` if the field isn't complete yet.
fn remaining_length(buf: &[u8]) -> Result<Option<(usize, usize)>, WireError> {
    let mut value = 0usize;
    for (i, &byte) in buf.iter().enumerate().take(4) {
        value |= ((byte & 0x7F) as usize) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(Some((value, i + 1)));
        }
    }
    if buf.len() >= 4 {
        return Err(WireError::MalformedLength);
    }
    Ok(None)
}

fn frame(header: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 5);
    out.push(header);
    let mut len = body.len();
    loop {
        let mut byte = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if len == 0 {
            break;
        }
    }
    out.extend_from_slice(body);
    out
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn read_u16(buf: &[u8], at: usize, what: &'static str) -> Result<u16, WireError> {
    buf.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(WireError::Malformed(what))
}
