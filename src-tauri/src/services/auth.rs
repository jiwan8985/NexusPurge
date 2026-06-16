use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub user: AuthUser,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(default, rename = "refreshToken", skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

pub trait AuthAdapter {
    fn login(&self) -> Result<AuthSession>;
    fn logout(&self) -> Result<()>;
    fn refresh_token(&self, session: AuthSession) -> Result<AuthSession>;
    fn current_session(&self) -> Result<Option<AuthSession>>;
}

pub struct ExternalAuthAdapter;

impl AuthAdapter for ExternalAuthAdapter {
    fn login(&self) -> Result<AuthSession> {
        Err(anyhow!(
            "External authentication module is not configured yet. AI LB or external auth integration is required."
        ))
    }

    fn logout(&self) -> Result<()> {
        Err(anyhow!(
            "External authentication module is not configured yet. AI LB or external auth integration is required."
        ))
    }

    fn refresh_token(&self, _session: AuthSession) -> Result<AuthSession> {
        Err(anyhow!(
            "External authentication module is not configured yet. AI LB or external auth integration is required."
        ))
    }

    fn current_session(&self) -> Result<Option<AuthSession>> {
        Ok(None)
    }
}
