use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn open_internal_browser(
    app: AppHandle,
    url: String,
    title: String,
) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;
    let host = parsed.host_str().unwrap_or("");
    if host != "localhost" && host != "127.0.0.1" {
        return Err("Internal browser only supports localhost URLs for security".into());
    }

    let label = format!("browser-{}", title.to_lowercase().replace(' ', "-"));

    if let Some(window) = app.get_webview_window(&label) {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::External(parsed),
    )
    .title(format!("ImpForge — {}", title))
    .inner_size(1200.0, 800.0)
    .min_inner_size(600.0, 400.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn close_internal_browser(
    app: AppHandle,
    title: String,
) -> Result<(), String> {
    let label = format!("browser-{}", title.to_lowercase().replace(' ', "-"));
    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
