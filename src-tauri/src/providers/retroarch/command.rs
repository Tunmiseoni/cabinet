use crate::constants;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

pub(crate) const GET_STATUS: &str = "GET_STATUS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RamRead {
    pub address: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstanceStatus {
    Contentless,
    Loaded {
        playing: bool,
        system: String,
        content: String,
        crc32: Option<u32>,
    },
}

pub(crate) fn netplay_game_watch() -> &'static str {
    "NETPLAY_GAME_WATCH"
}

pub(crate) fn read_core_ram(address: u32, bytes: u16) -> String {
    format!("READ_CORE_RAM {address:x} {bytes}")
}

fn frame(command: &str) -> Vec<u8> {
    let mut bytes = command.trim_end().as_bytes().to_vec();
    bytes.push(b'\n');
    bytes
}

fn local_socket() -> crate::error::Result<UdpSocket> {
    Ok(UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?)
}

fn target(port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
}

pub(crate) fn send(port: u16, command: &str) -> crate::error::Result<()> {
    let socket = local_socket()?;
    socket.send_to(&frame(command), target(port))?;
    Ok(())
}

pub(crate) fn request(port: u16, command: &str, timeout: Duration) -> crate::error::Result<String> {
    let socket = local_socket()?;
    socket.set_read_timeout(Some(timeout))?;
    socket.send_to(&frame(command), target(port))?;
    let mut buffer = [0u8; 2048];
    let len = socket.recv(&mut buffer).map_err(|err| match err.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
            format!("no reply from the command socket on port {port}")
        }
        _ => format!("command socket error on port {port}: {err}"),
    })?;
    Ok(String::from_utf8_lossy(&buffer[..len]).into_owned())
}

pub(crate) fn toggle_game_watch(port: u16) -> crate::error::Result<()> {
    send(port, netplay_game_watch())
}

pub(crate) fn read_core_ram_bytes(
    port: u16,
    address: u32,
    bytes: u16,
) -> crate::error::Result<RamRead> {
    let reply = request(
        port,
        &read_core_ram(address, bytes),
        constants::RETROARCH_COMMAND_TIMEOUT,
    )?;
    parse_read_core_ram(&reply)
}

pub(crate) fn instance_status(port: u16) -> crate::error::Result<InstanceStatus> {
    let reply = request(port, GET_STATUS, constants::RETROARCH_COMMAND_TIMEOUT)?;
    parse_get_status(&reply)
}

pub(crate) fn parse_read_core_ram(reply: &str) -> crate::error::Result<RamRead> {
    let line = reply.lines().next().unwrap_or("").trim();
    let mut tokens = line.split_whitespace();
    match tokens.next() {
        Some("READ_CORE_RAM") => {}
        _ => return Err(format!("unexpected command reply: {line}").into()),
    }
    let address_token = tokens
        .next()
        .ok_or("READ_CORE_RAM reply is missing an address")?;
    let address = u32::from_str_radix(address_token, 16)
        .map_err(|_| format!("READ_CORE_RAM reply has an invalid address: {address_token}"))?;

    let rest: Vec<&str> = tokens.collect();
    if rest.first() == Some(&"-1") {
        let message = rest[1..].join(" ");
        let message = if message.is_empty() {
            "read failed".to_string()
        } else {
            message
        };
        return Err(format!("READ_CORE_RAM {address_token} {message}").into());
    }

    let mut bytes = Vec::with_capacity(rest.len());
    for token in rest {
        if token.is_empty() || token.len() > 2 {
            return Err(format!("READ_CORE_RAM reply has an invalid byte: {token}").into());
        }
        let byte = u8::from_str_radix(token, 16)
            .map_err(|_| format!("READ_CORE_RAM reply has an invalid byte: {token}"))?;
        bytes.push(byte);
    }
    Ok(RamRead { address, bytes })
}

pub(crate) fn parse_get_status(reply: &str) -> crate::error::Result<InstanceStatus> {
    let line = reply.lines().next().unwrap_or("").trim();
    let rest = line
        .strip_prefix(GET_STATUS)
        .ok_or_else(|| format!("unexpected command reply: {line}"))?
        .trim();
    if rest.is_empty() || rest == "CONTENTLESS" {
        return Ok(InstanceStatus::Contentless);
    }

    let mut parts = rest.splitn(2, ' ');
    let state = parts.next().unwrap_or("");
    let detail = parts.next().unwrap_or("");
    let playing = match state {
        "PLAYING" => true,
        "PAUSED" => false,
        other => return Err(format!("unexpected GET_STATUS state: {other}").into()),
    };

    let (head, crc_field) = detail.rsplit_once(',').unwrap_or((detail, ""));
    let crc32 = crc_field
        .strip_prefix("crc32=")
        .and_then(|hex| u32::from_str_radix(hex, 16).ok());
    let (system, content) = head.split_once(',').unwrap_or((head, ""));

    Ok(InstanceStatus::Loaded {
        playing,
        system: system.to_string(),
        content: content.to_string(),
        crc32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_commands_with_a_trailing_newline() {
        assert_eq!(frame("GET_STATUS"), b"GET_STATUS\n");
        assert_eq!(frame("GET_STATUS\n"), b"GET_STATUS\n");
    }

    #[test]
    fn read_core_ram_frames_a_hex_address_and_a_decimal_length() {
        assert_eq!(read_core_ram(0x068D08, 1), "READ_CORE_RAM 68d08 1");
    }

    #[test]
    fn parses_a_single_byte_read() {
        let read = parse_read_core_ram("READ_CORE_RAM 68d08 A0\n").unwrap();
        assert_eq!(read.address, 0x68d08);
        assert_eq!(read.bytes, vec![0xA0]);
    }

    #[test]
    fn parses_a_multi_byte_read() {
        let read = parse_read_core_ram("READ_CORE_RAM 010d28 00 01 FF").unwrap();
        assert_eq!(read.address, 0x010d28);
        assert_eq!(read.bytes, vec![0x00, 0x01, 0xFF]);
    }

    #[test]
    fn rejects_a_failed_read_with_a_message() {
        let err = parse_read_core_ram("READ_CORE_RAM 0 -1 no memory map defined\n").unwrap_err();
        assert!(err.to_string().contains("no memory map defined"));
    }

    #[test]
    fn rejects_a_failed_read_without_a_message() {
        assert!(parse_read_core_ram("READ_CORE_RAM 68d08 -1\n").is_err());
    }

    #[test]
    fn rejects_an_unexpected_reply() {
        assert!(parse_read_core_ram("VERSION 1.22.2\n").is_err());
        assert!(parse_read_core_ram("READ_CORE_RAM\n").is_err());
    }

    #[test]
    fn parses_a_contentless_status() {
        assert_eq!(
            parse_get_status("GET_STATUS CONTENTLESS").unwrap(),
            InstanceStatus::Contentless
        );
        assert!(parse_get_status("").is_err());
    }

    #[test]
    fn parses_a_loaded_status() {
        let status =
            parse_get_status("GET_STATUS PLAYING fbneo,sfiii3nr1.zip,crc32=46119843\n").unwrap();
        assert_eq!(
            status,
            InstanceStatus::Loaded {
                playing: true,
                system: "fbneo".to_string(),
                content: "sfiii3nr1.zip".to_string(),
                crc32: Some(0x46119843),
            }
        );
    }

    #[test]
    fn parses_a_paused_status() {
        let status = parse_get_status("GET_STATUS PAUSED fbneo,sfiii3nr1.zip,crc32=0").unwrap();
        assert!(matches!(
            status,
            InstanceStatus::Loaded { playing: false, .. }
        ));
    }

    #[test]
    fn request_receives_a_reply_from_a_local_socket() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let responder = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let (len, from) = server.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..len], b"GET_STATUS\n");
            server.send_to(b"GET_STATUS CONTENTLESS", from).unwrap();
        });

        let reply = request(port, GET_STATUS, Duration::from_secs(1)).unwrap();
        assert_eq!(reply.trim(), "GET_STATUS CONTENTLESS");
        responder.join().unwrap();
    }

    #[test]
    fn send_delivers_a_command_without_waiting_for_a_reply() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let responder = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let (len, _) = server.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..len], b"NETPLAY_GAME_WATCH\n");
        });

        toggle_game_watch(port).unwrap();
        responder.join().unwrap();
    }

    #[test]
    fn request_times_out_when_nothing_replies() {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let err = request(port, GET_STATUS, Duration::from_millis(50)).unwrap_err();
        assert!(err.to_string().contains("no reply"));
    }
}
