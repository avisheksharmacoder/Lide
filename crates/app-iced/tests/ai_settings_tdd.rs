//! TDD test suite for Lide IDE AI settings: Endpoint input, health probing, and connection status.

use app_iced::views::settings_dialog::render_settings_dialog;
use app_iced::{AiConnectionStatus, EditorCommand, LideApp, SettingsTab};

#[test]
fn test_default_ai_settings_initialization() {
    let app = LideApp::new_headless();

    assert_eq!(app.ai_endpoint, "http://localhost:11434");
    assert_eq!(app.ai_model_name, "qwen2.5-coder:7b");
    assert_eq!(app.ai_status, AiConnectionStatus::Disconnected);
    assert_eq!(app.ai_status_message, None);
}

#[test]
fn test_set_ai_endpoint_and_state_reset() {
    let mut app = LideApp::new_headless();

    // Simulate prior connected state
    app.ai_status = AiConnectionStatus::Connected;
    app.ai_status_message = Some("Connected (3 models)".to_string());

    // Changing the endpoint resets connection state to Disconnected
    let _ = app.dispatch(EditorCommand::SetAiEndpoint("http://192.168.1.50:11434".to_string()));
    assert_eq!(app.ai_endpoint, "http://192.168.1.50:11434");
    assert_eq!(app.ai_status, AiConnectionStatus::Disconnected);
    assert_eq!(app.ai_status_message, None);
}

#[test]
fn test_set_ai_model_updates_state() {
    let mut app = LideApp::new_headless();

    let _ = app.dispatch(EditorCommand::SetAiModel("deepseek-coder:1.3b".to_string()));
    assert_eq!(app.ai_model_name, "deepseek-coder:1.3b");

    let _ = app.dispatch(EditorCommand::SetAiModel("codellama:7b".to_string()));
    assert_eq!(app.ai_model_name, "codellama:7b");
}

#[test]
fn test_probe_ai_connection_transitions_to_checking() {
    let mut app = LideApp::new_headless();

    let _ = app.dispatch(EditorCommand::ProbeAiConnection);
    assert_eq!(app.ai_status, AiConnectionStatus::Checking);
    assert_eq!(app.ai_status_message, Some("Probing Ollama endpoint...".to_string()));
}

#[test]
fn test_ai_connection_result_success_handling() {
    let mut app = LideApp::new_headless();

    let _ = app.dispatch(EditorCommand::AiConnectionResult {
        is_connected: true,
        error: Some("Connected (4 models available)".to_string()),
    });

    assert_eq!(app.ai_status, AiConnectionStatus::Connected);
    assert_eq!(app.ai_status_message, Some("Connected (4 models available)".to_string()));
}

#[test]
fn test_ai_connection_result_failure_handling() {
    let mut app = LideApp::new_headless();

    let _ = app.dispatch(EditorCommand::AiConnectionResult {
        is_connected: false,
        error: Some("Cannot connect to Ollama: connection refused".to_string()),
    });

    assert_eq!(app.ai_status, AiConnectionStatus::Error);
    assert_eq!(
        app.ai_status_message,
        Some("Cannot connect to Ollama: connection refused".to_string())
    );
}

#[test]
fn test_parse_ollama_endpoint_various_formats() {
    // Default localhost
    let (host, port) = ai_engine::parse_ollama_endpoint("http://localhost:11434");
    assert_eq!(host, "http://localhost");
    assert_eq!(port, 11434);

    // Custom IP and port with trailing slash
    let (host, port) = ai_engine::parse_ollama_endpoint("http://192.168.1.100:8000/");
    assert_eq!(host, "http://192.168.1.100");
    assert_eq!(port, 8000);

    // HTTPS scheme
    let (host, port) = ai_engine::parse_ollama_endpoint("https://remote-llm.corp:8443");
    assert_eq!(host, "https://remote-llm.corp");
    assert_eq!(port, 8443);

    // Bare host:port without scheme (defaults to http)
    let (host, port) = ai_engine::parse_ollama_endpoint("127.0.0.1:11434");
    assert_eq!(host, "http://127.0.0.1");
    assert_eq!(port, 11434);

    // Empty string fallback
    let (host, port) = ai_engine::parse_ollama_endpoint("");
    assert_eq!(host, "http://localhost");
    assert_eq!(port, 11434);
}

#[test]
fn test_headless_render_settings_ai_tab_across_all_connection_states() {
    let mut app = LideApp::new_headless();
    app.settings_dialog_open = true;
    app.active_settings_tab = SettingsTab::Ai;

    // 1. Disconnected
    app.ai_status = AiConnectionStatus::Disconnected;
    app.ai_status_message = None;
    let _element = render_settings_dialog(&app);

    // 2. Checking
    let _ = app.dispatch(EditorCommand::ProbeAiConnection);
    assert_eq!(app.ai_status, AiConnectionStatus::Checking);
    let _element = render_settings_dialog(&app);

    // 3. Connected
    let _ = app.dispatch(EditorCommand::AiConnectionResult {
        is_connected: true,
        error: Some("Connected (3 models available)".to_string()),
    });
    assert_eq!(app.ai_status, AiConnectionStatus::Connected);
    let _element = render_settings_dialog(&app);

    // 4. Error
    let _ = app.dispatch(EditorCommand::AiConnectionResult {
        is_connected: false,
        error: Some("Failed to reach Ollama daemon".to_string()),
    });
    assert_eq!(app.ai_status, AiConnectionStatus::Error);
    let _element = render_settings_dialog(&app);
}
