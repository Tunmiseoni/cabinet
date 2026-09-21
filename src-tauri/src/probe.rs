use serde::Serialize;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortProbe {
    pub ip: String,
    pub port: u16,
    pub reachable: bool,
    pub latency_ms: Option<u128>,
    pub error: Option<String>,
}

fn resolve(ip: &str, port: u16) -> Result<SocketAddr, String> {
    if let Ok(addr) = ip.parse::<IpAddr>() {
        return Ok(SocketAddr::new(addr, port));
    }
    (ip, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| format!("cannot resolve {ip}"))
}

pub fn probe(ip: &str, port: u16, timeout: Duration) -> PortProbe {
    let addr = match resolve(ip, port) {
        Ok(addr) => addr,
        Err(error) => {
            return PortProbe {
                ip: ip.to_string(),
                port,
                reachable: false,
                latency_ms: None,
                error: Some(error),
            };
        }
    };
    let started = Instant::now();
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_) => PortProbe {
            ip: ip.to_string(),
            port,
            reachable: true,
            latency_ms: Some(started.elapsed().as_millis()),
            error: None,
        },
        Err(err) => PortProbe {
            ip: ip.to_string(),
            port,
            reachable: false,
            latency_ms: None,
            error: Some(err.to_string()),
        },
    }
}

pub fn wait_for_port(ip: &str, port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if probe(ip, port, crate::constants::PORT_PROBE_ATTEMPT_TIMEOUT).reachable {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(crate::constants::PORT_PROBE_RETRY_INTERVAL);
    }
}

pub fn port_is_free(port: u16) -> Result<(), String> {
    TcpListener::bind(("0.0.0.0", port))
        .map(|_| ())
        .map_err(|err| format!("port {port} is already in use ({err})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reaches_a_bound_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let probe = probe("127.0.0.1", port, Duration::from_millis(500));
        assert!(probe.reachable, "{probe:?}");
        assert!(probe.latency_ms.is_some());
    }

    #[test]
    fn probe_reports_a_closed_port_as_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let probe = probe("127.0.0.1", port, Duration::from_millis(500));
        assert!(!probe.reachable, "{probe:?}");
        assert!(probe.error.is_some());
    }

    #[test]
    fn probe_reports_an_unresolvable_host() {
        let probe = probe("not-an-address", 55435, Duration::from_millis(100));
        assert!(!probe.reachable);
        assert!(probe.error.is_some());
    }

    #[test]
    fn wait_for_port_times_out_on_a_closed_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(!wait_for_port(
            "127.0.0.1",
            port,
            Duration::from_millis(600)
        ));
    }

    #[test]
    fn port_is_free_reports_a_bound_port() {
        let listener = TcpListener::bind("0.0.0.0:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(port_is_free(port).is_err());
        drop(listener);
    }
}
