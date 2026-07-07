use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;
use url::Url;

const KEYRING_SERVICE: &str = "cdn-upload-tool";
const PROFILES_FILENAME: &str = "profiles.json";
const SETTINGS_FILENAME: &str = "settings.json";

// ??? Profile Config ???????????????????????????????????????????????????????????

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ProfileScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<ProfilePermissions>,
    pub region: String,
    pub bucket: String,
    #[serde(rename = "basePrefix", skip_serializing_if = "Option::is_none")]
    pub base_prefix: Option<String>,
    #[serde(rename = "accessKeyId")]
    pub access_key_id: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "secretAccessKey")]
    pub secret_access_key: Option<String>,
    pub endpoint: Option<String>,
    #[serde(rename = "cdnProvider")]
    pub cdn_provider: Option<String>,
    #[serde(default, rename = "cdnProviders", skip_serializing_if = "Vec::is_empty")]
    pub cdn_providers: Vec<CdnProviderConfig>,
    #[serde(rename = "cdnDistributionId")]
    pub cdn_distribution_id: Option<String>,
    #[serde(rename = "cdnDomain")]
    pub cdn_domain: Option<String>,
    #[serde(rename = "cdnBasePath", skip_serializing_if = "Option::is_none")]
    pub cdn_base_path: Option<String>,
    #[serde(rename = "purgeOnNewUpload", default)]
    pub purge_on_new_upload: bool,
    #[serde(default, rename = "purgePolicy", skip_serializing_if = "Option::is_none")]
    pub purge_policy: Option<PurgePolicy>,
    #[serde(default, rename = "uploadPolicy", skip_serializing_if = "Option::is_none")]
    pub upload_policy: Option<UploadPolicy>,
    #[serde(default, rename = "metadataPolicy", skip_serializing_if = "Option::is_none")]
    pub metadata_policy: Option<UploadMetadataPolicy>,
    #[serde(default, rename = "logShipping", skip_serializing_if = "Option::is_none")]
    pub log_shipping: Option<LogShippingConfig>,
    #[serde(default, rename = "authBinding", skip_serializing_if = "Option::is_none")]
    pub auth_binding: Option<ExternalAuthBinding>,
    #[serde(rename = "defaultCacheControl")]
    pub default_cache_control: Option<String>,
    #[serde(rename = "contentTypeOverride")]
    pub content_type_override: Option<String>,
    #[serde(rename = "multipartEtagFallback", default)]
    pub multipart_etag_fallback: bool,
    // H-6: Akamai EdgeGrid ?먭꺽利앸챸 ?꾨뱶
    #[serde(rename = "akamaiClientToken", skip_serializing_if = "Option::is_none")]
    pub akamai_client_token: Option<String>,
    /// Akamai client secret ??keyring????? JSON?먮뒗 ?ы븿?섏? ?딆쓬
    #[serde(rename = "akamaiClientSecret", skip_serializing_if = "Option::is_none")]
    pub akamai_client_secret: Option<String>,
    #[serde(rename = "akamaiAccessToken", skip_serializing_if = "Option::is_none")]
    pub akamai_access_token: Option<String>,
    /// Akamai EdgeGrid API ?몄뒪??(e.g. akab-xxxx.luna.akamaiapis.net)
    #[serde(rename = "akamaiHost", skip_serializing_if = "Option::is_none")]
    pub akamai_host: Option<String>,
    /// Akamai Purge 대상 CP Code — 폴더/전체(와일드카드) Purge에 사용
    #[serde(rename = "akamaiCpCode", skip_serializing_if = "Option::is_none")]
    pub akamai_cp_code: Option<String>,
    // LG U+ CDN — username/password 기반 JWT 인증
    #[serde(rename = "lguplusUsername", skip_serializing_if = "Option::is_none")]
    pub lguplus_username: Option<String>,
    /// keyring에 저장 (JSON 직렬화 제외)
    #[serde(rename = "lguplusPassword", skip_serializing_if = "Option::is_none")]
    pub lguplus_password: Option<String>,
    #[serde(rename = "lguplusServiceName", skip_serializing_if = "Option::is_none")]
    pub lguplus_service_name: Option<String>,
    #[serde(rename = "lguplusVolumeName", skip_serializing_if = "Option::is_none")]
    pub lguplus_volume_name: Option<String>,
    #[serde(rename = "lguplusEndpoint", skip_serializing_if = "Option::is_none")]
    pub lguplus_endpoint: Option<String>,
    // KT CDN — username/password 기반 JWT 인증
    #[serde(rename = "ktUsername", skip_serializing_if = "Option::is_none")]
    pub kt_username: Option<String>,
    /// keyring에 저장 (JSON 직렬화 제외)
    #[serde(rename = "ktPassword", skip_serializing_if = "Option::is_none")]
    pub kt_password: Option<String>,
    #[serde(rename = "ktServiceName", skip_serializing_if = "Option::is_none")]
    pub kt_service_name: Option<String>,
    #[serde(rename = "ktVolumeName", skip_serializing_if = "Option::is_none")]
    pub kt_volume_name: Option<String>,
    #[serde(rename = "ktEndpoint", skip_serializing_if = "Option::is_none")]
    pub kt_endpoint: Option<String>,
    // Hyosung (미지원, 하위 호환)
    #[serde(rename = "hyosungApiKey", skip_serializing_if = "Option::is_none")]
    pub hyosung_api_key: Option<String>,
    #[serde(rename = "hyosungApiSecret", skip_serializing_if = "Option::is_none")]
    pub hyosung_api_secret: Option<String>,
    #[serde(rename = "hyosungEndpoint", skip_serializing_if = "Option::is_none")]
    pub hyosung_endpoint: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileScope {
    Project,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfilePermissionRole {
    Admin,
    Operator,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePermissions {
    pub role: ProfilePermissionRole,
    pub can_import: bool,
    pub can_remove: bool,
    pub can_create: bool,
    pub can_edit: bool,
    pub can_purge: bool,
    pub can_manage_secrets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdnProviderConfig {
    pub provider: String,
    #[serde(default, rename = "displayName", skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "distributionId", skip_serializing_if = "Option::is_none")]
    pub distribution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// 멀티 CDN 프로필: cdn_providers 항목의 provider별 도메인 우선, 없으면 공용 cdn_domain
pub fn provider_domain(profile: &ProfileConfig, provider: &str) -> Option<String> {
    profile
        .cdn_providers
        .iter()
        .find(|c| c.provider == provider)
        .and_then(|c| c.domain.clone())
        .filter(|d| !d.trim().is_empty())
        .or_else(|| profile.cdn_domain.clone())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PurgeMode {
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PurgeSelectionMode {
    All,
    Individual,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverwritePolicy {
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeBatchPolicy {
    #[serde(rename = "batchSize")]
    pub batch_size: usize,
    #[serde(rename = "warningThreshold")]
    pub warning_threshold: usize,
    #[serde(rename = "notRecommendedThreshold")]
    pub not_recommended_threshold: usize,
}

impl Default for PurgeBatchPolicy {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            warning_threshold: 5000,
            not_recommended_threshold: 10000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgePolicy {
    pub mode: PurgeMode,
    #[serde(rename = "requireApprovalBeforeAutomaticPurge")]
    pub require_approval_before_automatic_purge: bool,
    #[serde(rename = "requireLargePurgeWarning")]
    pub require_large_purge_warning: bool,
    #[serde(rename = "selectionMode")]
    pub selection_mode: PurgeSelectionMode,
    #[serde(rename = "overwritePolicy")]
    pub overwrite_policy: OverwritePolicy,
    #[serde(default)]
    pub batch: PurgeBatchPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPolicy {
    #[serde(rename = "overwritePolicy")]
    pub overwrite_policy: OverwritePolicy,
    #[serde(rename = "batchSize")]
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadMetadataPolicy {
    #[serde(rename = "autoApply")]
    pub auto_apply: bool,
    #[serde(default, rename = "contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, rename = "cacheControl", skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<String>,
    #[serde(default, rename = "customHeaders")]
    pub custom_headers: std::collections::HashMap<String, String>,
    #[serde(default, rename = "userMetadata")]
    pub user_metadata: std::collections::HashMap<String, String>,
    #[serde(rename = "allowManualRetryOnFailure")]
    pub allow_manual_retry_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub enabled: bool,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: usize,
    #[serde(rename = "backoffMs")]
    pub backoff_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogShippingConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default, rename = "includeOperations")]
    pub include_operations: Vec<String>,
    pub format: String,
    pub retry: RetryPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAuthBinding {
    pub provider: String,
    #[serde(default, rename = "keyRef", skip_serializing_if = "Option::is_none")]
    pub key_ref: Option<String>,
    #[serde(default, rename = "sessionRef", skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<String>,
    #[serde(default, rename = "requiredRoles")]
    pub required_roles: Vec<String>,
}

// ??? Credentials ??????????????????????????????????????????????????????????????

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

/// H-6: CDN 怨듦툒?먮퀎 ?먭꺽利앸챸 ??Clone 媛?ν븯??async ?쒖뒪??媛?怨듭쑀 媛??
#[derive(Clone)]
pub enum CdnCredentials {
    CloudFront(AwsCredentials),
    Akamai {
        client_token: String,
        client_secret: String,
        access_token: String,
        host: String,
        cdn_domain: String,
        /// 폴더/전체(와일드카드) Purge용 CP Code (선택)
        cp_code: Option<String>,
    },
    /// LG U+ CDN (Solbox CDN v3) — JWT 인증
    Lguplus {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
    },
    /// KT CDN (Solbox CDN v3) — JWT 인증
    Kt {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
    },
    /// 효성 ITX CDN — 헤더 인증 (X-ITX-Security-Principal / Secret)
    Hyosung {
        api_key:    String,
        api_secret: String,
        endpoint:   String,
        cdn_domain: String,
    },
}

// ??? App Settings ?????????????????????????????????????????????????????????????

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppSettings {
    #[serde(rename = "lastProfileId")]
    last_profile_id: Option<String>,
}

// ??? Profile Store ????????????????????????????????????????????????????????????

pub struct ProfileStore {
    profiles: RwLock<Vec<ProfileConfig>>,
    data_dir: PathBuf,
}

impl ProfileStore {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .context("data_local_dir 議고쉶 ?ㅽ뙣")?
            .join(KEYRING_SERVICE);
        std::fs::create_dir_all(&data_dir).context("?곗씠???붾젆?좊━ ?앹꽦 ?ㅽ뙣")?;
        Ok(Self {
            profiles: RwLock::new(vec![]),
            data_dir,
        })
    }

    fn profiles_path(&self) -> PathBuf {
        self.data_dir.join(PROFILES_FILENAME)
    }
    fn settings_path(&self) -> PathBuf {
        self.data_dir.join(SETTINGS_FILENAME)
    }

    pub async fn load_all(&self) -> Result<Vec<ProfileConfig>> {
        let path = self.profiles_path();
        if !path.exists() {
            return Ok(vec![]);
        }
        let content = tokio::fs::read_to_string(&path)
            .await
            .context("?꾨줈?뚯씪 ?뚯씪 ?쎄린 ?ㅽ뙣")?;
        let profiles: Vec<ProfileConfig> =
            serde_json::from_str(&content).context("?꾨줈?뚯씪 JSON ?뚯떛 ?ㅽ뙣")?;
        *self.profiles.write().await = profiles.clone();
        Ok(profiles)
    }

    pub async fn save(&self, mut profile: ProfileConfig) -> Result<()> {
        normalize_profile_inputs(&mut profile);
        validate_profile(&profile)?;
        let has_secret_input = profile
            .secret_access_key
            .as_deref()
            .map(|value| !value.is_empty())
            .unwrap_or(false);
        let has_saved_secret = Entry::new(KEYRING_SERVICE, &profile.id)
            .ok()
            .and_then(|entry| entry.get_password().ok())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if !has_secret_input && !has_saved_secret {
            return Err(anyhow::anyhow!("Secret Access Key is required"));
        }

        // S3 secret ??keyring
        if let Some(secret) = profile.secret_access_key.take() {
            if !secret.is_empty() {
                Entry::new(KEYRING_SERVICE, &profile.id)
                    .context("Keyring entry ?앹꽦 ?ㅽ뙣")?
                    .set_password(&secret)
                    .context("Keyring ????ㅽ뙣")?;
            }
        }
        // Akamai client secret ??keyring (蹂꾨룄 ??
        if let Some(secret) = profile.akamai_client_secret.take() {
            if !secret.is_empty() {
                let key = format!("{}_akamai", &profile.id);
                Entry::new(KEYRING_SERVICE, &key)
                    .context("Akamai Keyring entry ?앹꽦 ?ㅽ뙣")?
                    .set_password(&secret)
                    .context("Akamai Keyring ????ㅽ뙣")?;
            }
        }
        if let Some(secret) = profile.lguplus_password.take() {
            if !secret.is_empty() {
                let key = format!("{}_lguplus", &profile.id);
                Entry::new(KEYRING_SERVICE, &key)
                    .context("LG U+ Keyring entry creation failed")?
                    .set_password(&secret)
                    .context("LG U+ Keyring save failed")?;
            }
        }
        if let Some(secret) = profile.hyosung_api_secret.take() {
            if !secret.is_empty() {
                let key = format!("{}_hyosung", &profile.id);
                Entry::new(KEYRING_SERVICE, &key)
                    .context("Hyosung Keyring entry creation failed")?
                    .set_password(&secret)
                    .context("Hyosung Keyring save failed")?;
            }
        }
        if let Some(secret) = profile.kt_password.take() {
            if !secret.is_empty() {
                let key = format!("{}_kt", &profile.id);
                Entry::new(KEYRING_SERVICE, &key)
                    .context("KT Keyring entry creation failed")?
                    .set_password(&secret)
                    .context("KT Keyring save failed")?;
            }
        }

        let mut locked = self.profiles.write().await;
        match locked.iter().position(|p| p.id == profile.id) {
            Some(pos) => locked[pos] = profile,
            None => locked.push(profile),
        }
        tokio::fs::write(
            self.profiles_path(),
            serde_json::to_string_pretty(&*locked).context("JSON 吏곷젹???ㅽ뙣")?,
        )
        .await
        .context("?꾨줈?뚯씪 ?뚯씪 ????ㅽ뙣")
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, id) {
            let _ = entry.delete_password();
        }
        let akamai_key = format!("{}_akamai", id);
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, &akamai_key) {
            let _ = entry.delete_password();
        }
        let lguplus_key = format!("{}_lguplus", id);
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, &lguplus_key) {
            let _ = entry.delete_password();
        }
        let hyosung_key = format!("{}_hyosung", id);
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, &hyosung_key) {
            let _ = entry.delete_password();
        }
        let kt_key = format!("{}_kt", id);
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, &kt_key) {
            let _ = entry.delete_password();
        }
        let mut locked = self.profiles.write().await;
        locked.retain(|p| p.id != id);
        tokio::fs::write(
            self.profiles_path(),
            serde_json::to_string_pretty(&*locked).context("JSON 吏곷젹???ㅽ뙣")?,
        )
        .await
        .context("?꾨줈?뚯씪 ?뚯씪 ????ㅽ뙣")
    }

    pub async fn get_credentials(&self, profile_id: &str) -> Result<AwsCredentials> {
        let locked = self.profiles.read().await;
        let profile = locked
            .iter()
            .find(|p| p.id == profile_id)
            .context("?꾨줈?뚯씪??李얠쓣 ???놁쓬")?;
        let secret = Entry::new(KEYRING_SERVICE, profile_id)
            .context("Keyring entry ?앹꽦 ?ㅽ뙣")?
            .get_password()
            .context("Keyring?먯꽌 ?먭꺽利앸챸 濡쒕뱶 ?ㅽ뙣")?;
        Ok(AwsCredentials {
            access_key_id: profile.access_key_id.clone(),
            secret_access_key: secret,
        })
    }

    pub async fn get_profile(&self, profile_id: &str) -> Result<ProfileConfig> {
        let locked = self.profiles.read().await;
        locked
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
            .context("?꾨줈?뚯씪??李얠쓣 ???놁쓬")
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
            .context("?꾨줈?뚯씪??李얠쓣 ???놁쓬")?;
        Ok((
            creds,
            profile.region.clone(),
            profile.bucket.clone(),
            profile.endpoint.clone(),
        ))
    }

    /// H-6: CDN 怨듦툒?먮퀎 ?먭꺽利앸챸 議고쉶
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
                let (client_token, access_token, host, cdn_domain, cp_code) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("?꾨줈?뚯씪??李얠쓣 ???놁쓬")?;
                    (
                        profile.akamai_client_token.clone().unwrap_or_default(),
                        profile.akamai_access_token.clone().unwrap_or_default(),
                        profile.akamai_host.clone().unwrap_or_default(),
                        provider_domain(profile, "akamai").unwrap_or_default(),
                        profile.akamai_cp_code.clone(),
                    )
                }; // RwLockReadGuard ?댁젣 ??keyring ?몄텧
                let akamai_key = format!("{}_akamai", profile_id);
                let client_secret = Entry::new(KEYRING_SERVICE, &akamai_key)
                    .context("Akamai Keyring entry ?앹꽦 ?ㅽ뙣")?
                    .get_password()
                    .context("Akamai Keyring?먯꽌 ?먭꺽利앸챸 濡쒕뱶 ?ㅽ뙣")?;
                Ok(CdnCredentials::Akamai {
                    client_token,
                    client_secret,
                    access_token,
                    host,
                    cdn_domain,
                    cp_code,
                })
            }
            "lguplus" => {
                let (username, service_name, volume_name, endpoint, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.lguplus_username.clone().unwrap_or_default(),
                        profile.lguplus_service_name.clone().unwrap_or_default(),
                        profile.lguplus_volume_name.clone().unwrap_or_default(),
                        profile.lguplus_endpoint.clone()
                            .unwrap_or_else(|| "https://api.lgucdn.com".to_owned()),
                        provider_domain(profile, "lguplus").unwrap_or_default(),
                    )
                };
                let mut missing = Vec::new();
                if username.trim().is_empty() { missing.push("Username"); }
                if service_name.trim().is_empty() { missing.push("Service Name"); }
                if cdn_domain.trim().is_empty() { missing.push("Edge Domain"); }
                if !missing.is_empty() {
                    return Err(anyhow::anyhow!(
                        "LG U+ CDN 설정 누락: {} — 프로필에 입력하고 저장한 뒤 다시 시도하세요",
                        missing.join(", ")
                    ));
                }
                let lguplus_key = format!("{}_lguplus", profile_id);
                let password = Entry::new(KEYRING_SERVICE, &lguplus_key)
                    .context("LG U+ Keyring entry creation failed")?
                    .get_password()
                    .context("LG U+ Password가 저장되어 있지 않습니다 — 프로필에서 Password를 입력하고 저장하세요")?;
                Ok(CdnCredentials::Lguplus {
                    username,
                    password,
                    service_name,
                    volume_name,
                    endpoint,
                    cdn_domain,
                })
            }
            "hyosung" => {
                let (api_key, endpoint, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    let ep = profile.hyosung_endpoint.as_deref()
                        .map(|e| e.trim())
                        .filter(|e| !e.is_empty())
                        .unwrap_or("https://api.xtrmcdn.co.kr:28091")
                        .to_owned();
                    (
                        profile.hyosung_api_key.clone().unwrap_or_default(),
                        ep,
                        provider_domain(profile, "hyosung").unwrap_or_default(),
                    )
                };
                let mut missing = Vec::new();
                if api_key.trim().is_empty() { missing.push("API Key"); }
                if endpoint.trim().is_empty() { missing.push("API 엔드포인트"); }
                if cdn_domain.trim().is_empty() { missing.push("CDN 도메인"); }
                if !missing.is_empty() {
                    return Err(anyhow::anyhow!(
                        "효성 ITX CDN 설정 누락: {} — 프로필에 입력하고 저장한 뒤 다시 시도하세요",
                        missing.join(", ")
                    ));
                }
                let hyosung_key = format!("{}_hyosung", profile_id);
                let api_secret = Entry::new(KEYRING_SERVICE, &hyosung_key)
                    .context("Hyosung Keyring entry creation failed")?
                    .get_password()
                    .context("Hyosung Keyring load failed")?;
                Ok(CdnCredentials::Hyosung {
                    api_key,
                    api_secret,
                    endpoint,
                    cdn_domain,
                })
            }
            "kt" => {
                let (username, service_name, volume_name, endpoint, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.kt_username.clone().unwrap_or_default(),
                        profile.kt_service_name.clone().unwrap_or_default(),
                        profile.kt_volume_name.clone().unwrap_or_default(),
                        profile.kt_endpoint.clone()
                            .unwrap_or_else(|| "https://api.ktcdn.co.kr".to_owned()),
                        provider_domain(profile, "kt").unwrap_or_default(),
                    )
                };
                let mut missing = Vec::new();
                if username.trim().is_empty() { missing.push("Username"); }
                if service_name.trim().is_empty() { missing.push("Service Name"); }
                if cdn_domain.trim().is_empty() { missing.push("Edge Domain"); }
                if !missing.is_empty() {
                    return Err(anyhow::anyhow!(
                        "KT CDN 설정 누락: {} — 프로필에 입력하고 저장한 뒤 다시 시도하세요",
                        missing.join(", ")
                    ));
                }
                let kt_key = format!("{}_kt", profile_id);
                let password = Entry::new(KEYRING_SERVICE, &kt_key)
                    .context("KT Keyring entry creation failed")?
                    .get_password()
                    .context("KT Password가 저장되어 있지 않습니다 — 프로필에서 Password를 입력하고 저장하세요")?;
                Ok(CdnCredentials::Kt {
                    username,
                    password,
                    service_name,
                    volume_name,
                    endpoint,
                    cdn_domain,
                })
            }
            other => Err(anyhow::anyhow!("?????녿뒗 CDN 怨듦툒?? {}", other)),
        }
    }

    /// H-7: 留덉?留??곌껐 ?꾨줈?뚯씪 ID ???
    pub async fn save_last_profile_id(&self, id: &str) -> Result<()> {
        let settings = AppSettings {
            last_profile_id: Some(id.to_owned()),
        };
        tokio::fs::write(
            self.settings_path(),
            serde_json::to_string_pretty(&settings).context("?ㅼ젙 吏곷젹???ㅽ뙣")?,
        )
        .await
        .context("?ㅼ젙 ?뚯씪 ????ㅽ뙣")
    }

    /// H-7: 留덉?留??곌껐 ?꾨줈?뚯씪 ID 議고쉶
    pub async fn get_last_profile_id(&self) -> Result<Option<String>> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&path)
            .await
            .context("?ㅼ젙 ?뚯씪 ?쎄린 ?ㅽ뙣")?;
        let settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();
        Ok(settings.last_profile_id)
    }
}

fn validate_profile(profile: &ProfileConfig) -> Result<()> {
    if profile.name.trim().is_empty() {
        return Err(anyhow::anyhow!("Profile name is required"));
    }
    if profile.bucket.trim().is_empty() {
        return Err(anyhow::anyhow!("Bucket name is required"));
    }
    if profile.region.trim().is_empty() {
        return Err(anyhow::anyhow!("Region is required"));
    }
    if profile.access_key_id.trim().is_empty() {
        return Err(anyhow::anyhow!("Access Key ID is required"));
    }

    if let Some(endpoint) = profile
        .endpoint
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let url = Url::parse(endpoint).context("S3 custom endpoint URL is invalid")?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(anyhow::anyhow!("S3 custom endpoint must use http or https")),
        }
        if url.host_str().is_none() {
            return Err(anyhow::anyhow!("S3 custom endpoint host is required"));
        }
    }

    match profile
        .cdn_provider
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        None => Ok(()),
        Some("cloudfront") => {
            if profile
                .cdn_distribution_id
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!("CloudFront Distribution ID is required"));
            }
            if provider_domain(profile, "cloudfront")
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!("CloudFront CDN domain is required"));
            }
            Ok(())
        }
        Some("akamai") => {
            let domain = provider_domain(profile, "akamai");
            for (label, value) in [
                ("Akamai EdgeGrid host", profile.akamai_host.as_deref()),
                (
                    "Akamai Client Token",
                    profile.akamai_client_token.as_deref(),
                ),
                (
                    "Akamai Access Token",
                    profile.akamai_access_token.as_deref(),
                ),
                ("Akamai CDN domain", domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some("lguplus") => {
            let domain = provider_domain(profile, "lguplus");
            for (label, value) in [
                ("LG U+ CDN Username", profile.lguplus_username.as_deref()),
                ("LG U+ CDN Service Name", profile.lguplus_service_name.as_deref()),
                ("LG U+ CDN Edge Domain", domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some("hyosung") => {
            let domain = provider_domain(profile, "hyosung");
            for (label, value) in [
                ("Hyosung CDN API Key", profile.hyosung_api_key.as_deref()),
                ("Hyosung CDN Domain", domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some("kt") => {
            let domain = provider_domain(profile, "kt");
            for (label, value) in [
                ("KT CDN Username", profile.kt_username.as_deref()),
                ("KT CDN Service Name", profile.kt_service_name.as_deref()),
                ("KT CDN Edge Domain", domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some(other) => Err(anyhow::anyhow!("Unsupported CDN provider: {}", other)),
    }
}

fn normalize_profile_inputs(profile: &mut ProfileConfig) {
    profile.name = profile.name.trim().to_owned();
    profile.region = profile.region.trim().to_owned();
    profile.bucket = profile.bucket.trim().to_owned();
    profile.access_key_id = profile.access_key_id.trim().to_owned();
    profile.secret_access_key = profile
        .secret_access_key
        .take()
        .map(|value| value.trim().to_owned());
    profile.endpoint = profile
        .endpoint
        .take()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty());
}
