//! Lyrics verification in the system browser, with a one-use loopback callback.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

fn token_from_message(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if value["type"].as_str()? != "turnstile-token" {
        return None;
    }
    let token = value["token"].as_str()?.trim();
    (!token.is_empty() && token.len() < 16384 && !token.contains(['\r', '\n']))
        .then(|| token.to_string())
}

fn respond(stream: &mut TcpStream, status: &str, body: &str, content_type: &str) {
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}", body.len());
}

fn handle(stream: &mut TcpStream, path: &str, origin: &str, html: &str) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut data = Vec::new();
    let mut buf = [0; 4096];
    let header_end = loop {
        let count = stream.read(&mut buf).ok()?;
        if count == 0 { return None; }
        data.extend_from_slice(&buf[..count]);
        if data.len() > 32768 { return None; }
        if let Some(index) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = std::str::from_utf8(&data[..header_end]).ok()?;
    let request = headers.lines().next()?;
    let is_get = request == format!("GET {path} HTTP/1.1");
    let is_post = request == format!("POST {path} HTTP/1.1");
    let length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok()).flatten()
    }).unwrap_or(0);
    let valid_origin = headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| name.eq_ignore_ascii_case("origin") && value.trim() == origin)
    });
    if is_get {
        respond(stream, "200 OK", html, "text/html; charset=utf-8");
        return None;
    }
    if !is_post || !valid_origin || length == 0 || length > 17000 {
        respond(stream, "403 Forbidden", "Forbidden", "text/plain");
        return None;
    }
    while data.len() < header_end + length {
        let count = stream.read(&mut buf).ok()?;
        if count == 0 { return None; }
        data.extend_from_slice(&buf[..count]);
    }
    let token = token_from_message(&data[header_end..header_end + length]);
    respond(stream, "200 OK", "OK", "text/plain");
    token
}

pub fn run_helper() -> Result<(), String> {
    Err("Перевірка відкривається у системному браузері з налаштувань Vey".into())
}

pub async fn acquire_silent() -> Result<String, String> {
    Err("Для оновлення доступу відкрийте перевірку в налаштуваннях".into())
}

pub async fn acquire() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        use rand::RngCore;
        let mut nonce = [0u8; 24];
        rand::rng().fill_bytes(&mut nonce);
        let path = format!("/{}", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().map_err(|e| e.to_string())?.port());
        let bootstrap = format!("<script>window.ipc={{postMessage:function(body){{fetch('{}',{{method:'POST',headers:{{'Content-Type':'application/json'}},body}}).then(function(){{if(JSON.parse(body).type==='turnstile-token')document.body.textContent='Перевірку завершено. Поверніться до Vey.';}});}}}};</script>", path);
        let html = include_str!("../assets/lyrics-challenge.html")
            .replace("<script>", &format!("{bootstrap}<script>"))
            .replace("Перевірка доступу до слів пісень", &crate::i18n::translate(crate::i18n::current(), "Перевірка доступу до слів пісень"));
        crate::native::open_url(&format!("{origin}{path}"))?;
        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                    if let Some(token) = handle(&mut stream, &path, &origin, &html) { return Ok(token); }
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Час перевірки вичерпано".into())
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::token_from_message;
    #[test]
    fn accepts_only_bounded_single_line_tokens() {
        assert_eq!(token_from_message(br#"{"type":"turnstile-token","token":"abc"}"#).as_deref(), Some("abc"));
        assert!(token_from_message(br#"{"type":"turnstile-token","token":"x\ny"}"#).is_none());
        assert!(token_from_message(br#"{"type":"challenge-status","token":"abc"}"#).is_none());
    }
}
