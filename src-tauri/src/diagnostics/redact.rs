use crate::config::Config;

pub(crate) fn redact(text: &str, cfg: &Config) -> String {
    let text = redact_home(text);
    let text = redact_configured(&text, cfg);
    let text = redact_ipv4(&text);
    let text = redact_ipv6(&text);
    redact_magicdns(&text)
}

fn redact_configured(text: &str, cfg: &Config) -> String {
    let mut out = text.to_string();
    for (value, placeholder) in [
        (cfg.handle.as_deref(), "<handle>"),
        (cfg.default_peer_ip.as_deref(), "<peer-ip>"),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            out = out.replace(value, placeholder);
        }
    }
    out
}

fn redact_home(text: &str) -> String {
    let Some(home) = crate::env::home_dir() else {
        return text.to_string();
    };
    let home = home.to_string_lossy();
    if home.is_empty() {
        return text.to_string();
    }
    text.replace(home.as_ref(), "~")
}

fn redact_ipv4(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            token.push(ch);
        } else {
            push_redacted_token(&mut token, &mut out);
            out.push(ch);
        }
    }
    push_redacted_token(&mut token, &mut out);
    out
}

fn push_redacted_token(token: &mut String, out: &mut String) {
    if token.is_empty() {
        return;
    }
    if token.parse::<std::net::Ipv4Addr>().is_ok() {
        out.push_str("100.x.x.x");
    } else {
        out.push_str(token);
    }
    token.clear();
}

fn redact_ipv6(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_hexdigit() || ch == ':' {
            token.push(ch);
        } else {
            push_redacted_ipv6_token(&mut token, &mut out);
            out.push(ch);
        }
    }
    push_redacted_ipv6_token(&mut token, &mut out);
    out
}

fn push_redacted_ipv6_token(token: &mut String, out: &mut String) {
    if token.is_empty() {
        return;
    }
    if token.contains(':') && token.parse::<std::net::Ipv6Addr>().is_ok() {
        out.push_str("fdxx::x");
    } else {
        out.push_str(token);
    }
    token.clear();
}

fn redact_magicdns(text: &str) -> String {
    const SUFFIX: &str = ".ts.net";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(SUFFIX) {
        let bytes = rest.as_bytes();
        let mut start = index;
        while start > 0 {
            let prev = bytes[start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'-' || prev == b'.' {
                start -= 1;
            } else {
                break;
            }
        }
        let mut end = index + SUFFIX.len();
        while end < bytes.len() {
            let next = bytes[end];
            if next.is_ascii_alphanumeric() || next == b'-' || next == b'.' {
                end += 1;
            } else {
                break;
            }
        }
        out.push_str(&rest[..start]);
        out.push_str("example-tailnet.ts.net");
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_ipv4_addresses() {
        assert_eq!(
            redact_ipv4("connected to 100.64.0.2:41641 via 192.0.2.101"),
            "connected to 100.x.x.x:41641 via 100.x.x.x"
        );
    }

    #[test]
    fn redacts_ipv6_addresses() {
        assert_eq!(
            redact_ipv6("via [fd7a:115c:a1e0::f801:3fab]:41641 at 12:34:56"),
            "via [fdxx::x]:41641 at 12:34:56"
        );
    }

    #[test]
    fn redacts_magicdns_names() {
        assert_eq!(
            redact_magicdns(
                "self mac-host.example-tailnet.ts.net. peer windows-host.example-tailnet.ts.net."
            ),
            "self example-tailnet.ts.net peer example-tailnet.ts.net"
        );
    }

    #[test]
    fn redacts_configured_handle_and_peer() {
        let cfg = Config {
            handle: Some("player-one".into()),
            default_peer_ip: Some("100.64.0.2".into()),
            ..Config::default()
        };
        assert_eq!(
            redact("hi player-one at 100.64.0.2", &cfg),
            "hi <handle> at <peer-ip>"
        );
    }

    #[test]
    fn leaves_version_like_tokens_alone() {
        assert_eq!(redact_ipv4("v1.0.0.03 GIT6bb3167"), "v1.0.0.03 GIT6bb3167");
        assert_eq!(redact_ipv4("RetroArch 1.22.2"), "RetroArch 1.22.2");
    }
}
