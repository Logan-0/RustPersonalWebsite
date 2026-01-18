use actix_session::Session;
use actix_web::{web, HttpResponse};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
}

pub async fn login(
    pool: web::Data<SqlitePool>,
    session: Session,
    body: web::Json<LoginRequest>,
) -> HttpResponse {
    let user = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, username, password_hash FROM users WHERE username = ?",
    )
    .bind(&body.username)
    .fetch_optional(pool.get_ref())
    .await;

    let user = match user {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::Unauthorized().json(AuthResponse {
                success: false,
                message: "Invalid credentials".to_string(),
            });
        }
        Err(e) => {
            tracing::error!("Database error during login: {}", e);
            return HttpResponse::InternalServerError().json(AuthResponse {
                success: false,
                message: "Internal error".to_string(),
            });
        }
    };

    let (user_id, username, password_hash) = user;

    // Verify password
    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => {
            return HttpResponse::InternalServerError().json(AuthResponse {
                success: false,
                message: "Internal error".to_string(),
            });
        }
    };

    if Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return HttpResponse::Unauthorized().json(AuthResponse {
            success: false,
            message: "Invalid credentials".to_string(),
        });
    }

    // Set session
    if let Err(e) = session.insert("user_id", &user_id) {
        tracing::error!("Failed to set session: {}", e);
        return HttpResponse::InternalServerError().json(AuthResponse {
            success: false,
            message: "Session error".to_string(),
        });
    }

    if let Err(e) = session.insert("username", &username) {
        tracing::error!("Failed to set session: {}", e);
    }

    HttpResponse::Ok().json(AuthResponse {
        success: true,
        message: "Logged in successfully".to_string(),
    })
}

pub async fn logout(session: Session) -> HttpResponse {
    session.purge();
    HttpResponse::Ok().json(AuthResponse {
        success: true,
        message: "Logged out successfully".to_string(),
    })
}

pub async fn me(session: Session) -> HttpResponse {
    let user_id = session.get::<String>("user_id").ok().flatten();
    let username = session.get::<String>("username").ok().flatten();

    match (user_id, username) {
        (Some(id), Some(name)) => HttpResponse::Ok().json(UserInfo {
            id,
            username: name,
        }),
        _ => HttpResponse::Unauthorized().json(AuthResponse {
            success: false,
            message: "Not authenticated".to_string(),
        }),
    }
}

pub fn get_user_id(session: &Session) -> Option<String> {
    session.get::<String>("user_id").ok().flatten()
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let id = Uuid::new_v4().to_string();
    let password_hash = hash_password(password).map_err(|e| e.to_string())?;

    sqlx::query("INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(username)
        .bind(&password_hash)
        .execute(pool)
        .await?;

    Ok(id)
}
