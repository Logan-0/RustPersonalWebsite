use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MailError {
    #[error("Failed to send email: {0}")]
    SendError(String),
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
}

#[derive(Serialize)]
struct ResendEmail {
    from: String,
    to: Vec<String>,
    subject: String,
    text: String,
}

pub async fn send_email(
    sender_addr: &str,
    first_name: &str,
    last_name: &str,
    message: &str,
    api_key: &str,
) -> Result<bool, MailError> {
    let client = reqwest::Client::new();

    let email = ResendEmail {
        from: "Logan Carpenter <noreply@logancarpenter.space>".to_string(),
        to: vec!["LoganTCarpenter@gmail.com".to_string()],
        subject: format!(
            "Logan0Dev - Mail from: {} {}<{}>",
            first_name, last_name, sender_addr
        ),
        text: message.to_string(),
    };

    let response = client
        .post("https://api.resend.com/emails")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&email)
        .send()
        .await?;

    if response.status().is_success() {
        Ok(true)
    } else {
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("Failed to Send Email Message: {}", error_text);
        Err(MailError::SendError(error_text))
    }
}
