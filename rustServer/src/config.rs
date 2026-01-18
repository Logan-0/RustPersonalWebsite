use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid environment variable: {0}")]
    InvalidEnvVar(String),
}

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct Config {
    pub node_env: Option<String>,
    pub mail_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if present
        dotenvy::dotenv().ok();

        let node_env = get_env_var("NODE_ENV")?;
        let mail_api_key = get_env_var("MAIL_API_KEY")?;

        Ok(Self {
            node_env: Some(node_env),
            mail_api_key: Some(mail_api_key),
        })
    }

    pub fn mail_api_key(&self) -> Option<&str> {
        self.mail_api_key.as_deref()
    }
}

fn get_env_var(name: &str) -> Result<String, ConfigError> {
    let value = std::env::var(name).map_err(|_| ConfigError::MissingEnvVar(name.to_string()))?;

    if value.is_empty() {
        return Err(ConfigError::InvalidEnvVar(format!(
            "{} cannot be empty",
            name
        )));
    }

    Ok(value)
}
