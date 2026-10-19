//! A dedicated, isolated challenge window. No Google pages or account bridge.
#[derive(Debug, PartialEq)]
enum ChallengeEvent {
    Token(String),
    Progress(String),
}
fn navigation_allowed(url: &str) -> bool {
    use base64::Engine;
    use std::sync::LazyLock;
    static BOOTSTRAP: LazyLock<String> = LazyLock::new(|| {
        base64::engine::general_purpose::STANDARD
            .encode(include_str!("../assets/lyrics-challenge.html"))
    });
    url == "about:blank"
        || url.starts_with("https://lyrics.api.dacubeking.com/")
        || url.starts_with("https://challenges.cloudflare.com/")
        || [
            "data:text/html;charset=utf-8;base64,",
            "data:text/html;base64,",
        ]
        .iter()
        .any(|prefix| {
            url.strip_prefix(prefix)
                .is_some_and(|data| data == BOOTSTRAP.as_str())
        })
}
fn parse_message(raw: &str) -> Option<ChallengeEvent> {
    if raw.len() > 17000 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    match value["type"].as_str()? {
        "turnstile-token" => {
            let token = value["token"].as_str()?.trim();
            (!token.is_empty() && token.len() < 16384 && !token.contains(['\n', '\r']))
                .then(|| ChallengeEvent::Token(token.into()))
        }
        "turnstile-error" => {
            let code = value["error"]
                .as_str()
                .unwrap_or("unknown")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .take(64)
                .collect::<String>();
            Some(ChallengeEvent::Progress(format!("Turnstile: {code}")))
        }
        "turnstile-expired" => Some(ChallengeEvent::Progress("Turnstile: expired".into())),
        "turnstile-timeout" => Some(ChallengeEvent::Progress("Turnstile: timeout".into())),
        "challenge-status" => match value["state"].as_str()? {
            "host-ready" => Some(ChallengeEvent::Progress("challenge host ready".into())),
            "frame-loaded" => Some(ChallengeEvent::Progress("challenge frame loaded".into())),
            _ => None,
        },
        _ => None,
    }
}
#[cfg(target_os = "windows")]
pub fn run_helper() -> Result<(), String> {
    use tao::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
        window::WindowBuilder,
    };
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let silent = std::env::args().any(|a| a == "--silent-challenge");
    let builder = WindowBuilder::new()
        .with_visible(!silent)
        .with_title("PlayerVF — перевірка слів пісень")
        .with_inner_size(tao::dpi::LogicalSize::new(500., 360.));
    eprintln!("[lyrics-auth] starting browser host");
    let window = builder.build(&event_loop).map_err(|e| e.to_string())?;
    let proxy = event_loop.create_proxy();
    let html = include_str!("../assets/lyrics-challenge.html");
    let mut context =
        wry::WebContext::new(Some(std::env::temp_dir().join("playervf-lyrics-challenge")));
    let webview = wry::WebViewBuilder::new_with_web_context(&mut context)
        .with_html(html)
        .with_navigation_handler(|url| {
            let allowed = navigation_allowed(&url);
            if !allowed {
                eprintln!("[lyrics-auth] blocked unrelated browser navigation");
            }
            allowed
        })
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_ipc_handler(move |request| {
            let body = request.body();
            if parse_message(body).is_some() {
                let _ = proxy.send_event(body.clone());
            }
        })
        .build(&window)
        .map_err(|e| e.to_string())?;
    eprintln!("[lyrics-auth] browser host ready");
    let mut webview = Some(webview);
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(message) => {
                use std::io::Write;
                let done = matches!(parse_message(&message), Some(ChallengeEvent::Token(_)));
                if done {
                    drop(webview.take());
                }
                let mut out = std::io::stdout().lock();
                let _ = writeln!(out, "{message}");
                let _ = out.flush();
                if done {
                    *flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                drop(webview.take());
                *flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
#[cfg(not(target_os = "windows"))]
pub fn run_helper() -> Result<(), String> {
    Err("Вбудована перевірка ще недоступна на цій платформі".into())
}

pub async fn acquire() -> Result<String, String> {
    acquire_mode(false).await
}
pub async fn acquire_silent() -> Result<String, String> {
    acquire_mode(true).await
}
async fn acquire_mode(silent: bool) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        tokio::task::spawn_blocking(move || {
            use std::{
                io::{BufRead, BufReader, Read},
                process::{Command, Stdio},
                time::{Duration, Instant},
            };
            let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
            command.arg("--lyrics-challenge");
            if silent {
                command.arg("--silent-challenge");
            }
            let mut child = command
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| e.to_string())?;
            let stdout = child.stdout.take().ok_or("Немає відповіді перевірки")?;
            let (tx, rx) = std::sync::mpsc::channel();
            // Drain while the helper is running: Turnstile tokens can exceed
            // the anonymous pipe buffer. Waiting for exit before reading deadlocks.
            let reader = std::thread::spawn(move || {
                for line in BufReader::new(stdout.take(65536))
                    .lines()
                    .map_while(Result::ok)
                {
                    if let Some(message) = parse_message(&line) {
                        let _ = tx.send(message);
                    }
                }
            });
            let deadline = Instant::now() + Duration::from_secs(if silent { 20 } else { 120 });
            let mut last_error = "Час перевірки вичерпано".to_string();
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(ChallengeEvent::Token(token)) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = reader.join();
                        return Ok(token);
                    }
                    Ok(ChallengeEvent::Progress(message)) => {
                        eprintln!("[lyrics-auth] {message}");
                        if message.starts_with("Turnstile:") {
                            last_error = message;
                        }
                    }
                    Err(_) => {}
                }
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    let _ = reader.join();
                    for message in rx.try_iter() {
                        match message {
                            ChallengeEvent::Token(token) => return Ok(token),
                            ChallengeEvent::Progress(message) => last_error = message,
                        }
                    }
                    return Err(format!("Перевірку не завершено ({status}): {last_error}"));
                }
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(last_error);
                }
            }
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Вбудована перевірка ще недоступна на цій платформі".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn webview_bootstrap_navigation_allows_only_the_bundled_html() {
        use base64::Engine;
        let html = include_str!("../assets/lyrics-challenge.html");
        let encoded = base64::engine::general_purpose::STANDARD.encode(html);
        assert!(navigation_allowed(&format!(
            "data:text/html;charset=utf-8;base64,{encoded}"
        )));
        assert!(navigation_allowed(
            "https://lyrics.api.dacubeking.com/challenge"
        ));
        assert!(!navigation_allowed(
            "data:text/html;charset=utf-8;base64,ZXZpbA=="
        ));
        assert!(!navigation_allowed(
            "https://lyrics.api.dacubeking.com.evil.test/challenge"
        ));
        assert!(!navigation_allowed("https://example.com/"));
    }
    #[test]
    fn helper_protocol_keeps_large_tokens_and_validates_messages() {
        let token = "x".repeat(12000);
        let json = serde_json::json!({"type":"turnstile-token","token":token}).to_string();
        assert_eq!(parse_message(&json), Some(ChallengeEvent::Token(token)));
        assert_eq!(
            parse_message(r#"{"type":"turnstile-token","token":" "}"#),
            None
        );
        assert_eq!(
            parse_message(r#"{"type":"untrusted","token":"secret"}"#),
            None
        );
        assert_eq!(parse_message("invalid"), None);
        assert_eq!(
            parse_message(r#"{"type":"turnstile-error","error":"110200"}"#),
            Some(ChallengeEvent::Progress("Turnstile: 110200".into()))
        );
    }
}
