//! Lyrics verification in the system browser, with a one-use loopback callback.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) enum ChallengeSignal {
    Token(String),
    Cancel,
    BrowserFallback,
    FrameLoaded,
}

fn parse_signal(value: &serde_json::Value) -> Option<ChallengeSignal> {
    match value["type"].as_str()? {
        "turnstile-token" => {
            let token = value["token"].as_str()?.trim();
            (!token.is_empty() && token.len() < 16384 && !token.contains(['\r', '\n']))
                .then(|| ChallengeSignal::Token(token.to_string()))
        }
        "challenge-cancel" => Some(ChallengeSignal::Cancel),
        "challenge-browser-fallback" => Some(ChallengeSignal::BrowserFallback),
        "challenge-status" if value["state"].as_str()? == "frame-loaded" => {
            Some(ChallengeSignal::FrameLoaded)
        }
        _ => None,
    }
}

pub(crate) fn parse_embedded_message(body: &str) -> Option<ChallengeSignal> {
    parse_signal(&serde_json::from_str(body).ok()?)
}

fn signal_from_message(body: &[u8]) -> Option<ChallengeSignal> {
    parse_signal(&serde_json::from_slice(body).ok()?)
}

#[cfg(test)]
fn token_from_message(body: &[u8]) -> Option<String> {
    match signal_from_message(body)? {
        ChallengeSignal::Token(token) => Some(token),
        _ => None,
    }
}

fn respond(stream: &mut TcpStream, status: &str, body: &str, content_type: &str) {
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}", body.len());
}

fn handle(stream: &mut TcpStream, path: &str, origin: &str, html: &str) -> Option<ChallengeSignal> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut data = Vec::new();
    let mut buf = [0; 4096];
    let header_end = loop {
        let count = stream.read(&mut buf).ok()?;
        if count == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..count]);
        if data.len() > 32768 {
            return None;
        }
        if let Some(index) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = std::str::from_utf8(&data[..header_end]).ok()?;
    let request = headers.lines().next()?;
    let is_get = request == format!("GET {path} HTTP/1.1");
    let is_post = request == format!("POST {path} HTTP/1.1");
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let valid_origin = headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("origin") && value.trim() == origin
        })
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
        if count == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..count]);
    }
    let signal = signal_from_message(&data[header_end..header_end + length]);
    respond(stream, "200 OK", "OK", "text/plain");
    signal.filter(|signal| !matches!(signal, ChallengeSignal::FrameLoaded))
}

#[cfg(target_os = "windows")]
pub(crate) struct EmbeddedPageServer {
    url: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _thread: std::thread::JoinHandle<()>,
}

#[cfg(target_os = "windows")]
impl EmbeddedPageServer {
    pub(crate) fn url(&self) -> &str {
        &self.url
    }
}

#[cfg(target_os = "windows")]
impl Drop for EmbeddedPageServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(target_os = "windows")]
fn serve_embedded_page(stream: &mut TcpStream, path: &str, html: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut data = Vec::with_capacity(1024);
    let mut buf = [0u8; 1024];
    let header_end = loop {
        let Ok(count) = stream.read(&mut buf) else {
            respond(stream, "400 Bad Request", "Bad Request", "text/plain");
            return;
        };
        if count == 0 {
            return;
        }
        data.extend_from_slice(&buf[..count]);
        if data.len() > 8192 {
            respond(stream, "413 Content Too Large", "Too Large", "text/plain");
            return;
        }
        if let Some(index) = data.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let Ok(headers) = std::str::from_utf8(&data[..header_end]) else {
        respond(stream, "400 Bad Request", "Bad Request", "text/plain");
        return;
    };
    let expected_request = format!("GET {path} HTTP/1.1");
    if headers.lines().next() == Some(expected_request.as_str()) {
        respond(stream, "200 OK", html, "text/html; charset=utf-8");
    } else {
        respond(stream, "404 Not Found", "Not Found", "text/plain");
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn start_embedded_page() -> Result<EmbeddedPageServer, String> {
    use rand::RngCore;
    use std::sync::atomic::{AtomicBool, Ordering};

    let mut nonce = [0u8; 24];
    rand::rng().fill_bytes(&mut nonce);
    let path = format!(
        "/{}",
        nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let origin = format!(
        "http://127.0.0.1:{}",
        listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port()
    );
    let url = format!("{origin}{path}");
    let html = include_str!("../assets/lyrics-challenge.html")
        .replace(
            "<script>",
            "<script>window.__VEY_EXTERNAL__=false;</script><script>",
        )
        .replace(
            "Перевірка доступу до слів пісень",
            &crate::i18n::translate(crate::i18n::current(), "Перевірка доступу до слів пісень"),
        );
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread = std::thread::Builder::new()
        .name("vey-lyrics-webview-page".into())
        .spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                        serve_embedded_page(&mut stream, &path, &html);
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(35));
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(EmbeddedPageServer {
        url,
        stop,
        _thread: thread,
    })
}

pub fn run_helper() -> Result<(), String> {
    Err("Перевірка відкривається у системному браузері з налаштувань Vey".into())
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
        let bootstrap = format!("<script>window.__VEY_EXTERNAL__=true;window.ipc={{postMessage:function(body){{fetch('{}',{{method:'POST',headers:{{'Content-Type':'application/json'}},body}}).then(function(){{const type=JSON.parse(body).type;if(type==='turnstile-token')document.body.textContent='Перевірку завершено. Поверніться до Vey.';else if(type==='challenge-cancel')document.body.textContent='Перевірку скасовано. Поверніться до Vey.';}});}}}};</script>", path);
        let html = include_str!("../assets/lyrics-challenge.html")
            .replace("<script>", &format!("{bootstrap}<script>"))
            .replace("Перевірка доступу до слів пісень", &crate::i18n::translate(crate::i18n::current(), "Перевірка доступу до слів пісень"));
        crate::native::open_url(&format!("{origin}{path}"))?;
        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                    if let Some(signal) = handle(&mut stream, &path, &origin, &html) {
                        return match signal {
                            ChallengeSignal::Token(token) => Ok(token),
                            ChallengeSignal::Cancel => Err("Перевірку скасовано".into()),
                            ChallengeSignal::BrowserFallback => Err("Зараз уже відкрито системний браузер".into()),
                            ChallengeSignal::FrameLoaded => unreachable!(),
                        };
                    }
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
    use super::{parse_embedded_message, token_from_message, ChallengeSignal};
    #[test]
    fn accepts_only_bounded_single_line_tokens() {
        assert_eq!(
            token_from_message(br#"{"type":"turnstile-token","token":"abc"}"#).as_deref(),
            Some("abc")
        );
        assert!(token_from_message(br#"{"type":"turnstile-token","token":"x\ny"}"#).is_none());
        assert!(token_from_message(br#"{"type":"challenge-status","token":"abc"}"#).is_none());
        assert!(matches!(
            parse_embedded_message(r#"{"type":"challenge-cancel"}"#),
            Some(ChallengeSignal::Cancel)
        ));
        assert!(matches!(
            parse_embedded_message(r#"{"type":"challenge-browser-fallback"}"#),
            Some(ChallengeSignal::BrowserFallback)
        ));
        assert!(parse_embedded_message(r#"{"type":"turnstile-token","token":"a\nb"}"#).is_none());
    }
}
