use pot_gtk::services::translate::bing::BingTranslate;
use pot_gtk::services::translate::google::GoogleTranslate;
use pot_gtk::services::translate::lingva::LingvaTranslate;
use pot_gtk::services::translate::openai::OpenAITranslate;
use pot_gtk::services::translate::TranslateService;
use pot_gtk::services::types::TranslateRequest;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

struct MockServer {
    base_url: String,
    _server: Arc<tiny_http::Server>,
}

impl MockServer {
    fn start<F>(responder: F) -> Self
    where
        F: Fn(tiny_http::Request) + Send + Sync + 'static,
    {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = match server.server_addr() {
            tiny_http::ListenAddr::IP(addr) => addr.port(),
            _ => panic!("unexpected listen addr"),
        };
        let base_url = format!("http://127.0.0.1:{}", port);
        let server = Arc::new(server);
        let responder = Arc::new(responder);

        let srv = server.clone();
        let responder_clone = responder.clone();
        thread::spawn(move || {
            for req in srv.incoming_requests() {
                responder_clone(req);
            }
        });
        thread::sleep(Duration::from_millis(50));

        MockServer {
            base_url,
            _server: server,
        }
    }
}

// ── Google Translate ─────────────────────────────────────────────────────────

fn google_ok(req: tiny_http::Request) {
    let body = r#"[[["你好","hello",null,null,10]],"en"]"#;
    let resp = tiny_http::Response::from_string(body).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
    );
    let _ = req.respond(resp);
}

#[tokio::test]
async fn google_translate_text() {
    let mock = MockServer::start(google_ok);
    let svc = GoogleTranslate;
    let req = TranslateRequest {
        text: "hello".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!({ "custom_url": mock.base_url }),
    };
    let result = svc.translate(req).await.unwrap();
    match &result {
        pot_gtk::services::types::TranslateResult::Text(t) => assert_eq!(t, "你好"),
        pot_gtk::services::types::TranslateResult::Dictionary(d) => {
            panic!(
                "expected Text variant, got Dictionary with {} explanations",
                d.explanations.len()
            );
        }
    }
}

#[tokio::test]
async fn google_translate_requests_text_segments() {
    let requested_url = Arc::new(Mutex::new(String::new()));
    let request_state = requested_url.clone();

    let mock = MockServer::start(move |req| {
        let url = req.url().to_string();
        *request_state.lock().unwrap_or_else(|e| e.into_inner()) = url.clone();

        let body = if url.contains("dt=t") {
            r#"[[["你好","hello",null,null,10]],null,"en",null,null,null,null,[]]"#
        } else {
            r#"[null,null,"",null,null,null,null,[]]"#
        };

        let resp = tiny_http::Response::from_string(body).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
        let _ = req.respond(resp);
    });
    let svc = GoogleTranslate;
    let req = TranslateRequest {
        text: "hello".into(),
        from: "auto".into(),
        to: "zh_cn".into(),
        config: json!({ "custom_url": mock.base_url }),
    };

    let result = svc.translate(req).await.unwrap();
    let requested_url = requested_url
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    assert!(requested_url.contains("dt=t"));
    match result {
        pot_gtk::services::types::TranslateResult::Text(text) => assert_eq!(text, "你好"),
        _ => panic!("expected Text variant"),
    }
}

#[test]
fn google_name_and_default_config() {
    let svc = GoogleTranslate;
    assert_eq!(svc.name(), "google");
    assert!(svc.default_config().get("custom_url").is_some());
}

#[tokio::test]
async fn bing_translate_maps_auto_and_chinese_language_codes() {
    let requested_url = Arc::new(Mutex::new(String::new()));
    let request_state = requested_url.clone();

    let mock = MockServer::start(move |req| {
        let url = req.url().to_string();

        if url == "/auth" {
            let resp = tiny_http::Response::from_string("mock-token");
            let _ = req.respond(resp);
            return;
        }

        *request_state.lock().unwrap_or_else(|e| e.into_inner()) = url.clone();

        let body = if url.contains("from=auto") || url.contains("to=zh_cn") {
            r#"{"error":{"code":400035,"message":"The source or target language is not valid."}}"#
        } else {
            r#"[{"translations":[{"text":"你好","to":"zh-Hans"}]}]"#
        };

        let resp = tiny_http::Response::from_string(body).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
        let _ = req.respond(resp);
    });
    let svc = BingTranslate;
    let req = TranslateRequest {
        text: "hello".into(),
        from: "auto".into(),
        to: "zh_cn".into(),
        config: json!({
            "auth_url": format!("{}/auth", mock.base_url),
            "request_url": format!("{}/translate", mock.base_url),
        }),
    };

    let result = svc.translate(req).await.unwrap();
    let requested_url = requested_url
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    assert!(!requested_url.contains("from=auto"));
    assert!(requested_url.contains("to=zh-Hans"));
    match result {
        pot_gtk::services::types::TranslateResult::Text(text) => assert_eq!(text, "你好"),
        _ => panic!("expected Text variant"),
    }
}

#[tokio::test]
async fn bing_refetches_token_when_auth_url_changes() {
    let auth_hits_one = Arc::new(AtomicUsize::new(0));
    let auth_hits_two = Arc::new(AtomicUsize::new(0));

    let mock_one = MockServer::start({
        let auth_hits = auth_hits_one.clone();
        move |req| {
            if req.url() == "/auth" {
                auth_hits.fetch_add(1, Ordering::SeqCst);
                let _ = req.respond(tiny_http::Response::from_string("token-one"));
                return;
            }

            let body = r#"[{"translations":[{"text":"你好","to":"zh-Hans"}]}]"#;
            let resp = tiny_http::Response::from_string(body).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            let _ = req.respond(resp);
        }
    });

    let mock_two = MockServer::start({
        let auth_hits = auth_hits_two.clone();
        move |req| {
            if req.url() == "/auth" {
                auth_hits.fetch_add(1, Ordering::SeqCst);
                let _ = req.respond(tiny_http::Response::from_string("token-two"));
                return;
            }

            let body = r#"[{"translations":[{"text":"世界","to":"zh-Hans"}]}]"#;
            let resp = tiny_http::Response::from_string(body).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            let _ = req.respond(resp);
        }
    });

    let svc = BingTranslate;
    let first = svc
        .translate(TranslateRequest {
            text: "hello".into(),
            from: "auto".into(),
            to: "zh_cn".into(),
            config: json!({
                "auth_url": format!("{}/auth", mock_one.base_url),
                "request_url": format!("{}/translate", mock_one.base_url),
            }),
        })
        .await
        .unwrap();
    let second = svc
        .translate(TranslateRequest {
            text: "world".into(),
            from: "auto".into(),
            to: "zh_cn".into(),
            config: json!({
                "auth_url": format!("{}/auth", mock_two.base_url),
                "request_url": format!("{}/translate", mock_two.base_url),
            }),
        })
        .await
        .unwrap();

    assert_eq!(auth_hits_one.load(Ordering::SeqCst), 1);
    assert_eq!(auth_hits_two.load(Ordering::SeqCst), 1);
    assert!(matches!(
        first,
        pot_gtk::services::types::TranslateResult::Text(_)
    ));
    assert!(matches!(
        second,
        pot_gtk::services::types::TranslateResult::Text(_)
    ));
}

// ── Lingva Translate ─────────────────────────────────────────────────────────

fn lingva_ok(req: tiny_http::Request) {
    let body = r#"{"translation":"你好世界"}"#;
    let resp = tiny_http::Response::from_string(body).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
    );
    let _ = req.respond(resp);
}

#[tokio::test]
async fn lingva_translate_text() {
    let mock = MockServer::start(lingva_ok);
    let svc = LingvaTranslate;
    let req = TranslateRequest {
        text: "hello world".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!({ "custom_url": mock.base_url }),
    };
    let result = svc.translate(req).await.unwrap();
    match result {
        pot_gtk::services::types::TranslateResult::Text(t) => assert_eq!(t, "你好世界"),
        _ => panic!("expected Text variant"),
    }
}

#[test]
fn lingva_name_and_default_config() {
    let svc = LingvaTranslate;
    assert_eq!(svc.name(), "lingva");
    assert!(svc.default_config().get("custom_url").is_some());
}

// ── OpenAI Translate ─────────────────────────────────────────────────────────

fn openai_ok(req: tiny_http::Request) {
    let body = r#"{"id":"chatcmpl-123","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"你好"},"finish_reason":"stop"}]}"#;
    let resp = tiny_http::Response::from_string(body).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
    );
    let _ = req.respond(resp);
}

fn openai_error(req: tiny_http::Request) {
    let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#;
    let resp = tiny_http::Response::from_string(body)
        .with_status_code(401)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
    let _ = req.respond(resp);
}

#[tokio::test]
async fn openai_translate_text() {
    let mock = MockServer::start(openai_ok);
    let svc = OpenAITranslate;
    let req = TranslateRequest {
        text: "hello".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!({
            "api_key": "test-key",
            "model": "gpt-3.5-turbo",
            "request_path": mock.base_url,
            "service": "openai",
            "prompt": [],
            "request_arguments": {}
        }),
    };
    let result = svc.translate(req).await.unwrap();
    match result {
        pot_gtk::services::types::TranslateResult::Text(t) => assert_eq!(t, "你好"),
        _ => panic!("expected Text variant"),
    }
}

#[tokio::test]
async fn openai_error_response() {
    let mock = MockServer::start(openai_error);
    let svc = OpenAITranslate;
    let req = TranslateRequest {
        text: "hello".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!({
            "api_key": "bad-key",
            "model": "gpt-3.5-turbo",
            "request_path": mock.base_url,
            "service": "openai",
            "prompt": [],
            "request_arguments": {}
        }),
    };
    assert!(svc.translate(req).await.is_err());
}

#[test]
fn openai_name_and_default_config() {
    let svc = OpenAITranslate;
    assert_eq!(svc.name(), "openai");
    assert_eq!(
        svc.default_config().get("model").unwrap().as_str().unwrap(),
        "gpt-3.5-turbo"
    );
}

// ── Registry integration ─────────────────────────────────────────────────────

#[tokio::test]
async fn registry_translate_missing_service() {
    let reg = pot_gtk::services::translate::TranslateRegistry::new();
    let req = TranslateRequest {
        text: "hello".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!(null),
    };
    let err = reg.translate("nonexistent", req).await.unwrap_err();
    assert!(err.message.contains("not found"));
}

#[test]
fn registry_register_and_list() {
    let mut reg = pot_gtk::services::translate::TranslateRegistry::new();
    use std::sync::Arc;
    reg.register(Arc::new(GoogleTranslate));
    reg.register(Arc::new(LingvaTranslate));
    reg.register(Arc::new(OpenAITranslate));
    assert_eq!(reg.list().len(), 3);
    assert!(reg.get("google").is_some());
    assert!(reg.get("lingva").is_some());
    assert!(reg.get("openai").is_some());
    assert!(reg.get("baidu").is_none());
}
