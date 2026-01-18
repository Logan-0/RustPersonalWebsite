use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::Config;
use crate::mail;

#[derive(Deserialize)]
pub struct EmailRequest {
    pub sender: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct EmailResponse {
    pub data: bool,
}

pub async fn send_email(
    config: web::Data<Arc<Config>>,
    body: web::Json<EmailRequest>,
) -> HttpResponse {
    let api_key = match config.mail_api_key() {
        Some(key) => key,
        None => {
            tracing::error!("Mail API key not configured");
            return HttpResponse::ServiceUnavailable().finish();
        }
    };

    match mail::send_email(
        &body.sender,
        &body.first_name,
        &body.last_name,
        &body.message,
        api_key,
    )
    .await
    {
        Ok(true) => HttpResponse::Ok().json(EmailResponse { data: true }),
        Ok(false) => HttpResponse::InternalServerError().finish(),
        Err(e) => {
            tracing::error!("Error Sending E-Mail: {:?}", e);
            HttpResponse::ServiceUnavailable().finish()
        }
    }
}

pub async fn spa_fallback() -> Result<actix_files::NamedFile> {
    Ok(actix_files::NamedFile::open(
        "../client/leptosUI/dist/index.html",
    )?)
}
