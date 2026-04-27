use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;

const KEYRING_SERVICE: &str = "cdn-upload-tool";
const PROFILES_FILENAME: &str = "profiles.json";
const SETTINGS_FILENAME: &str = "settings.json";

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
    // H-6: Akamai EdgeGrid 자격증명 필드
    #[serde(rename = "akamaiClientToken", skip_serializing_if = "Option::is_none")]
    pub akamai_client_token: Option<String>,
    /// Akamai client secret — keyring에 저장, JSON에는 포함하지 않음
    #[serde(rename = "akamaiClientSecret", skip_serializing_if = "Option::is_none")]
    pub akamai_client_secret: Option<String>,
    #[serde(rename = "akamaiAccessToken", skip_serializing_if = "Option::is_none")]
    pub akamai_access_token: Option<String>,
    /// Akamai EdgeGrid API 호스트 (e.g. akab-xxxx.luna.akamaiapis.net)
    #[serde(rename = "akamaiHost", skip_serializing_if = "Option::is_none")]
    pub akamai_host: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

// ─── Credentials ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

/// H-6: CDN 공급자별 자격증명 — Clone 가능하여 async 태스크 간 공유 가능
#[derive(Clone)]
pub enum CdnCredentials {
    CloudFront(AwsCredentials),
    Akamai {
        client_token:  String,
        client_secret: String,
        access_token:  String,
        host:          String, // EdgeGrid API 호스트
        cdn_domain:    String, // CDN 도메인 (Purge URL 구성용)
    },
}

// ─── App Settings ─────────────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppSettings {
    #[serde(rename = "lastProfileId")]
    last_profile_id: Option<String>,
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
        Ok(Self { profiles: RwLock::new(vec![]), data_dir })
    }

    fn profiles_path(&self) -> PathBuf { self.data_dir.join(PROFILES_FILENAME) }
    fn settings_path(&self) -> PathBuf { self.data_dir.join(SETTINGS_FILENAME) }

    pub async fn load_all(&self) -> Result<Vec<ProfileConfig>> {
        let path = self.profiles_path();
        if !path.exists() { return Ok(vec![]); }
        let content = tokio::fs::read_to_string(&path)
            .await
            .context("프로파일 파일 읽기 실패")?;
        let profiles: Vec<ProfileConfig> =
            serde_json::from_str(&content).context("프로파일 JSON 파싱 실패")?;
        *self.profiles.write().await = profiles.clone();
        Ok(profiles)
    }

    pub async fn save(&self, mut profile: ProfileConfig) -> Result<()> {
        // S3 secret → keyring
        if let Some(secret) = profile.secret_access_key.take() {
            if !secret.is_empty() {
                Entry::new(KEYRING_SERVICE, &profile.id)
                    .context("Keyring entry 생성 실패")?
                    .set_password(&secret)
                    .context("Keyring 저장 실패")?;
            }
        }
        // Akamai client secret → keyring (별도 키)
        if let Some(secret) = profile.akamai_client_secret.take() {
            if !secret.is_empty() {
                let key = format!("{}_akamai", &profile.id);
                Entry::new(KEYRING_SERVICE, &key)
                    .context("Akamai Keyring entry 생성 실패")?
                    .set_password(&secret)
                    .context("Akamai Keyring 저장 실패")?;
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
        let akamai_key = format!("{}_akamai", id);
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, &akamai_key) {
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

    /// H-6: CDN 공급자별 자격증명 조회
    pub async fn get_cdn_credentials(
        &self,
        profile_id: &str,
        provider: &str,
    ) -> Result<CdnCredentials> {
        match provider {
            "cloudfront" => {
                let creds = self.get_credentials(profile_id).await?;
                Ok(CdnCredentials::CloudFront(creds))
            }
            "akamai" => {
                let (client_token, access_token, host, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("프로파일을 찾을 수 없음")?;
                    (
                        profile.akamai_client_token.clone().unwrap_or_default(),
                        profile.akamai_access_token.clone().unwrap_or_default(),
                        profile.akamai_host.clone().unwrap_or_default(),
                        profile.cdn_domain.clone().unwrap_or_default(),
                    )
                }; // RwLockReadGuard 해제 후 keyring 호출
                let akamai_key = format!("{}_akamai", profile_id);
                let client_secret = Entry::new(KEYRING_SERVICE, &akamai_key)
                    .context("Akamai Keyring entry 생성 실패")?
                    .get_password()
                    .context("Akamai Keyring에서 자격증명 로드 실패")?;
                Ok(CdnCredentials::Akamai {
                    client_token,
                    client_secret,
                    access_token,
                    host,
                    cdn_domain,
                })
            }
            other => Err(anyhow::anyhow!("알 수 없는 CDN 공급자: {}", other)),
        }
    }

    /// H-7: 마지막 연결 프로파일 ID 저장
    pub async fn save_last_profile_id(&self, id: &str) -> Result<()> {
        let settings = AppSettings { last_profile_id: Some(id.to_owned()) };
        tokio::fs::write(
            self.settings_path(),
            serde_json::to_string_pretty(&settings).context("설정 직렬화 실패")?,
        )
        .await
        .context("설정 파일 저장 실패")
    }

    /// H-7: 마지막 연결 프로파일 ID 조회
    pub async fn get_last_profile_id(&self) -> Result<Option<String>> {
        let path = self.settings_path();
        if !path.exists() { return Ok(None); }
        let content = tokio::fs::read_to_string(&path)
            .await
            .context("설정 파일 읽기 실패")?;
        let settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();
        Ok(settings.last_profile_id)
    }
}
