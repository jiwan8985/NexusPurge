use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;

const KEYRING_SERVICE: &str = "cdn-upload-tool";
const PROFILES_FILENAME: &str = "profiles.json";

// ─── Profile Config ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub id: String,
    pub name: String,
    pub region: String,
    pub bucket: String,
    #[serde(rename = "accessKeyId")]
    pub access_key_id: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "secretAccessKey")]
    pub secret_access_key: Option<String>,
    pub endpoint: Option<String>,
    #[serde(rename = "cdnProvider")]
    pub cdn_provider: Option<String>,
    #[serde(rename = "cdnDistributionId")]
    pub cdn_distribution_id: Option<String>,
    #[serde(rename = "cdnDomain")]
    pub cdn_domain: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

// ─── Credentials (aws-lc-sys 없는 순수 구조체) ───────────────────────────────

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

// ─── Profile Store ────────────────────────────────────────────────────────────

pub struct ProfileStore {
    profiles: RwLock<Vec<ProfileConfig>>,
    data_dir: PathBuf,
}

impl ProfileStore {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .context("data_local_dir 조회 실패")?
            .join(KEYRING_SERVICE);

        std::fs::create_dir_all(&data_dir).context("데이터 디렉토리 생성 실패")?;

        Ok(Self {
            profiles: RwLock::new(vec![]),
            data_dir,
        })
    }

    fn profiles_path(&self) -> PathBuf {
        self.data_dir.join(PROFILES_FILENAME)
    }

    pub async fn load_all(&self) -> Result<Vec<ProfileConfig>> {
        let path = self.profiles_path();
        if !path.exists() {
            return Ok(vec![]);
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .context("프로파일 파일 읽기 실패")?;

        let profiles: Vec<ProfileConfig> =
            serde_json::from_str(&content).context("프로파일 JSON 파싱 실패")?;

        *self.profiles.write().await = profiles.clone();
        Ok(profiles)
    }

    pub async fn save(&self, mut profile: ProfileConfig) -> Result<()> {
        if let Some(secret) = profile.secret_access_key.take() {
            if !secret.is_empty() {
                Entry::new(KEYRING_SERVICE, &profile.id)
                    .context("Keyring entry 생성 실패")?
                    .set_password(&secret)
                    .context("Keyring 저장 실패")?;
            }
        }

        let mut locked = self.profiles.write().await;
        match locked.iter().position(|p| p.id == profile.id) {
            Some(pos) => locked[pos] = profile,
            None => locked.push(profile),
        }

        tokio::fs::write(
            self.profiles_path(),
            serde_json::to_string_pretty(&*locked).context("JSON 직렬화 실패")?,
        )
        .await
        .context("프로파일 파일 저장 실패")
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, id) {
            let _ = entry.delete_password();
        }

        let mut locked = self.profiles.write().await;
        locked.retain(|p| p.id != id);

        tokio::fs::write(
            self.profiles_path(),
            serde_json::to_string_pretty(&*locked).context("JSON 직렬화 실패")?,
        )
        .await
        .context("프로파일 파일 저장 실패")
    }

    pub async fn get_credentials(&self, profile_id: &str) -> Result<AwsCredentials> {
        let locked = self.profiles.read().await;
        let profile = locked
            .iter()
            .find(|p| p.id == profile_id)
            .context("프로파일을 찾을 수 없음")?;

        let secret = Entry::new(KEYRING_SERVICE, profile_id)
            .context("Keyring entry 생성 실패")?
            .get_password()
            .context("Keyring에서 자격증명 로드 실패")?;

        Ok(AwsCredentials {
            access_key_id: profile.access_key_id.clone(),
            secret_access_key: secret,
        })
    }

    pub async fn get_connection_info(
        &self,
        profile_id: &str,
    ) -> Result<(AwsCredentials, String, String, Option<String>)> {
        let creds = self.get_credentials(profile_id).await?;
        let locked = self.profiles.read().await;
        let profile = locked
            .iter()
            .find(|p| p.id == profile_id)
            .context("프로파일을 찾을 수 없음")?;

        Ok((creds, profile.region.clone(), profile.bucket.clone(), profile.endpoint.clone()))
    }
}
