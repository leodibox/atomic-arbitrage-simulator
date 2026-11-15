use reqwest::Client;
use serde_json::json;

pub async fn send_telegram(token: &str, chat_id: &str, text: &str) {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let _ = Client::new()
        .post(&url)
        .json(&json!({ "chat_id": chat_id, "text": text }))
        .send()
        .await;
}
