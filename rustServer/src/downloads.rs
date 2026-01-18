use actix_files::NamedFile;
use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::auth::get_user_id;

const DOWNLOADS_DIR: &str = "../downloads";

#[derive(Serialize)]
pub struct DownloadFile {
    pub id: String,
    pub file_path: String,
    pub display_name: String,
    pub description: Option<String>,
    pub is_protected: bool,
}

#[derive(Serialize)]
pub struct DownloadToken {
    pub token: String,
    pub download_url: String,
}

#[derive(Deserialize)]
pub struct GenerateTokenRequest {
    pub file_id: String,
}

pub async fn list_files(pool: web::Data<SqlitePool>, session: Session) -> HttpResponse {
    let is_authenticated = get_user_id(&session).is_some();

    let files = if is_authenticated {
        // Show all files for authenticated users
        sqlx::query_as::<_, (String, String, String, Option<String>, i32)>(
            "SELECT id, file_path, display_name, description, is_protected FROM download_files",
        )
        .fetch_all(pool.get_ref())
        .await
    } else {
        // Show only public files for unauthenticated users
        sqlx::query_as::<_, (String, String, String, Option<String>, i32)>(
            "SELECT id, file_path, display_name, description, is_protected FROM download_files WHERE is_protected = 0",
        )
        .fetch_all(pool.get_ref())
        .await
    };

    match files {
        Ok(rows) => {
            let files: Vec<DownloadFile> = rows
                .into_iter()
                .map(|(id, file_path, display_name, description, is_protected)| DownloadFile {
                    id,
                    file_path,
                    display_name,
                    description,
                    is_protected: is_protected != 0,
                })
                .collect();
            HttpResponse::Ok().json(files)
        }
        Err(e) => {
            tracing::error!("Database error listing files: {}", e);
            HttpResponse::InternalServerError().body("Error listing files")
        }
    }
}

pub async fn generate_token(
    pool: web::Data<SqlitePool>,
    session: Session,
    body: web::Json<GenerateTokenRequest>,
) -> HttpResponse {
    let user_id = match get_user_id(&session) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().body("Authentication required"),
    };

    // Verify file exists and is protected
    let file = sqlx::query_as::<_, (String, i32)>(
        "SELECT file_path, is_protected FROM download_files WHERE id = ?",
    )
    .bind(&body.file_id)
    .fetch_optional(pool.get_ref())
    .await;

    let (file_path, is_protected) = match file {
        Ok(Some(f)) => f,
        Ok(None) => return HttpResponse::NotFound().body("File not found"),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    };

    // For public files, return direct download URL
    if is_protected == 0 {
        return HttpResponse::Ok().json(DownloadToken {
            token: "".to_string(),
            download_url: format!("/downloads/public/{}", file_path),
        });
    }

    // Generate single-use token for protected files
    let token = Uuid::new_v4().to_string();
    let token_id = Uuid::new_v4().to_string();

    let result = sqlx::query(
        "INSERT INTO download_tokens (id, token, file_id, user_id) VALUES (?, ?, ?, ?)",
    )
    .bind(&token_id)
    .bind(&token)
    .bind(&body.file_id)
    .bind(&user_id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => HttpResponse::Ok().json(DownloadToken {
            token: token.clone(),
            download_url: format!("/downloads/token/{}", token),
        }),
        Err(e) => {
            tracing::error!("Failed to create download token: {}", e);
            HttpResponse::InternalServerError().body("Failed to generate token")
        }
    }
}

pub async fn download_by_token(
    pool: web::Data<SqlitePool>,
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let token = path.into_inner();

    // Find and validate token
    let token_data = sqlx::query_as::<_, (String, String, i32)>(
        r#"
        SELECT dt.id, df.file_path, dt.used 
        FROM download_tokens dt
        JOIN download_files df ON dt.file_id = df.id
        WHERE dt.token = ?
        "#,
    )
    .bind(&token)
    .fetch_optional(pool.get_ref())
    .await;

    let (token_id, file_path, used) = match token_data {
        Ok(Some(data)) => data,
        Ok(None) => return Ok(HttpResponse::NotFound().body("Invalid or expired token")),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Ok(HttpResponse::InternalServerError().body("Database error"));
        }
    };

    // Check if token already used
    if used != 0 {
        return Ok(HttpResponse::Gone().body("Token has already been used"));
    }

    // Mark token as used
    let _ = sqlx::query("UPDATE download_tokens SET used = 1 WHERE id = ?")
        .bind(&token_id)
        .execute(pool.get_ref())
        .await;

    // Serve the file
    serve_file(&req, &file_path).await
}

pub async fn download_public(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse> {
    let requested_path = path.into_inner();
    serve_file(&req, &requested_path).await
}

async fn serve_file(req: &HttpRequest, requested_path: &str) -> Result<HttpResponse> {
    // Security: Validate and sanitize the path
    let safe_path = match sanitize_path(requested_path) {
        Some(p) => p,
        None => {
            tracing::warn!("Invalid download path requested: {}", requested_path);
            return Ok(HttpResponse::BadRequest().body("Invalid file path"));
        }
    };

    let file_path = Path::new(DOWNLOADS_DIR).join(&safe_path);

    // Security: Ensure the resolved path is still within downloads directory
    let canonical_downloads = match std::fs::canonicalize(DOWNLOADS_DIR) {
        Ok(p) => p,
        Err(_) => {
            tracing::error!("Downloads directory not found");
            return Ok(HttpResponse::InternalServerError().body("Downloads directory not configured"));
        }
    };

    let canonical_file = match std::fs::canonicalize(&file_path) {
        Ok(p) => p,
        Err(_) => {
            tracing::debug!("File not found: {:?}", file_path);
            return Ok(HttpResponse::NotFound().body("File not found"));
        }
    };

    if !canonical_file.starts_with(&canonical_downloads) {
        tracing::warn!("Path traversal attempt detected: {}", requested_path);
        return Ok(HttpResponse::Forbidden().body("Access denied"));
    }

    // Serve the file with proper headers for download
    match NamedFile::open(&canonical_file) {
        Ok(file) => {
            let file = file
                .use_last_modified(true)
                .set_content_disposition(actix_web::http::header::ContentDisposition {
                    disposition: actix_web::http::header::DispositionType::Attachment,
                    parameters: vec![actix_web::http::header::DispositionParam::Filename(
                        safe_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("download")
                            .to_string(),
                    )],
                });
            Ok(file.into_response(req))
        }
        Err(e) => {
            tracing::error!("Error opening file {:?}: {}", canonical_file, e);
            Ok(HttpResponse::InternalServerError().body("Error reading file"))
        }
    }
}

fn sanitize_path(path: &str) -> Option<PathBuf> {
    let path = path.trim_start_matches('/');
    
    // Reject empty paths
    if path.is_empty() {
        return None;
    }

    // Reject paths with null bytes
    if path.contains('\0') {
        return None;
    }

    let path_buf = PathBuf::from(path);

    // Reject paths with parent directory references
    for component in path_buf.components() {
        match component {
            std::path::Component::Normal(_) => continue,
            std::path::Component::ParentDir => return None,
            std::path::Component::CurDir => continue,
            _ => return None,
        }
    }

    Some(path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_path_valid() {
        assert!(sanitize_path("file.zip").is_some());
        assert!(sanitize_path("projects/myapp.tar.gz").is_some());
        assert!(sanitize_path("nested/deep/file.pdf").is_some());
    }

    #[test]
    fn test_sanitize_path_invalid() {
        assert!(sanitize_path("../etc/passwd").is_none());
        assert!(sanitize_path("foo/../bar").is_none());
        assert!(sanitize_path("/absolute/path").is_some()); // Leading slash is stripped
        assert!(sanitize_path("").is_none());
    }
}
