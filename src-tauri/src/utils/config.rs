use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use url::Url;

const KEYRING_SERVICE: &str = "cdn-upload-tool";
const PROFILES_FILENAME: &str = "profiles.json";
const SETTINGS_FILENAME: &str = "settings.json";
const PROFILES_LOCK_FILENAME: &str = "profiles.json.lock";

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
    /// AWS Access Key ID — keyring에 저장, 로드 시 빈 값 (secret과 마찬가지로 profiles.json에 평문 보관하지 않음)
    #[serde(default, rename = "accessKeyId", skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
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
    /// "cloudcdn" | "volume" (기본 "volume") — cloudcdn이면 전체 Purge 시 Purge by Service 사용 가능
    #[serde(rename = "lguplusServiceType", skip_serializing_if = "Option::is_none")]
    pub lguplus_service_type: Option<String>,
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
    /// "cloudcdn" | "volume" (기본 "volume") — cloudcdn이면 전체 Purge 시 Purge by Service 사용 가능
    #[serde(rename = "ktServiceType", skip_serializing_if = "Option::is_none")]
    pub kt_service_type: Option<String>,
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
        /// "cloudcdn" | "volume" — 전체 Purge 시 Purge by Service 사용 가능 여부
        service_type: String,
    },
    /// KT CDN (Solbox CDN v3) — JWT 인증
    Kt {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
        /// "cloudcdn" | "volume" — 전체 Purge 시 Purge by Service 사용 가능 여부
        service_type: String,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(rename = "lastProfileId")]
    pub last_profile_id: Option<String>,
    /// CDN API 감사 로그(audit-*.log)에 응답 본문까지 포함할지 여부 — 기본은 요약만(false)
    #[serde(default, rename = "detailedAuditLog")]
    pub detailed_audit_log: bool,
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

    #[cfg(test)]
    fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            profiles: RwLock::new(vec![]),
            data_dir,
        }
    }

    fn profiles_path(&self) -> PathBuf {
        self.data_dir.join(PROFILES_FILENAME)
    }
    fn settings_path(&self) -> PathBuf {
        self.data_dir.join(SETTINGS_FILENAME)
    }
    fn profiles_lock_path(&self) -> PathBuf {
        self.data_dir.join(PROFILES_LOCK_FILENAME)
    }

    /// profiles.json에 대한 배타적 크로스 프로세스 잠금을 건 채로 `f`를 실행한다.
    /// 여러 NexusPurge 인스턴스가 동시에 프로필을 저장/삭제해도 서로 덮어써서
    /// 유실되지 않도록, f 안에서 직접 최신 파일을 다시 읽고 원자적으로 쓴다.
    async fn with_profiles_lock<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let lock_path = self.profiles_lock_path();
        tokio::task::spawn_blocking(move || {
            let lock_file =
                std::fs::File::create(&lock_path).context("프로필 잠금 파일 생성 실패")?;
            fs4::fs_std::FileExt::lock_exclusive(&lock_file).context("프로필 파일 잠금 획득 실패")?;
            f()
            // lock_file drop 시 잠금 자동 해제
        })
        .await
        .context("프로필 잠금 작업 실행 실패")?
    }

    pub async fn load_all(&self) -> Result<Vec<ProfileConfig>> {
        let path = self.profiles_path();
        if !path.exists() {
            return Ok(vec![]);
        }
        let content = tokio::fs::read_to_string(&path)
            .await
            .context("프로필 파일 읽기 실패")?;
        let mut profiles: Vec<ProfileConfig> =
            serde_json::from_str(&content).context("프로필 JSON 파싱 실패")?;

        // 과거 버전에서 profiles.json에 평문으로 남아있던 인증 관련 값(Access Key ID,
        // Akamai/Hyosung 토큰, LG U+/KT 계정명)을 keyring으로 이전하고 파일에서 제거한다.
        let mut migrated = false;
        for profile in profiles.iter_mut() {
            let id = profile.id.clone();
            migrated |= migrate_legacy_auth_field(&id, "_access_key_id", &mut profile.access_key_id);
            migrated |= migrate_legacy_auth_field(&id, "_akamai_client_token", &mut profile.akamai_client_token);
            migrated |= migrate_legacy_auth_field(&id, "_akamai_access_token", &mut profile.akamai_access_token);
            migrated |= migrate_legacy_auth_field(&id, "_hyosung_api_key", &mut profile.hyosung_api_key);
            migrated |= migrate_legacy_auth_field(&id, "_lguplus_username", &mut profile.lguplus_username);
            migrated |= migrate_legacy_auth_field(&id, "_kt_username", &mut profile.kt_username);
        }
        if migrated {
            let data_dir = self.data_dir.clone();
            let profiles_path = path.clone();
            let profiles_to_write = profiles.clone();
            self.with_profiles_lock(move || {
                atomic_write_json(&data_dir, &profiles_path, &profiles_to_write)
            })
            .await
            .context("마이그레이션된 프로필 파일 저장 실패")?;
        }

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
        // AWS Access Key ID도 keyring — profiles.json에는 인증 관련 값을 남기지 않는다
        if let Some(value) = profile.access_key_id.take() {
            if !value.is_empty() {
                let key = format!("{}_access_key_id", &profile.id);
                Entry::new(KEYRING_SERVICE, &key)
                    .context("Access Key ID keyring entry 생성 실패")?
                    .set_password(&value)
                    .context("Access Key ID keyring 저장 실패")?;
            }
        }
        for (suffix, field) in [
            ("_akamai_client_token", &mut profile.akamai_client_token),
            ("_akamai_access_token", &mut profile.akamai_access_token),
            ("_hyosung_api_key", &mut profile.hyosung_api_key),
            ("_lguplus_username", &mut profile.lguplus_username),
            ("_kt_username", &mut profile.kt_username),
        ] {
            if let Some(value) = field.take() {
                if !value.is_empty() {
                    let key = format!("{}{}", &profile.id, suffix);
                    Entry::new(KEYRING_SERVICE, &key)
                        .context("Keyring entry 생성 실패")?
                        .set_password(&value)
                        .context("Keyring 저장 실패")?;
                }
            }
        }
        // Akamai client secret도 keyring (별도 키)
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

        let profiles_path = self.profiles_path();
        let data_dir = self.data_dir.clone();
        let updated = self
            .with_profiles_lock(move || {
                let mut list = read_profiles_from_disk(&profiles_path)?;
                match list.iter().position(|p| p.id == profile.id) {
                    Some(pos) => list[pos] = profile,
                    None => list.push(profile),
                }
                atomic_write_json(&data_dir, &profiles_path, &list)?;
                Ok(list)
            })
            .await?;
        *self.profiles.write().await = updated;
        Ok(())
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
        for suffix in [
            "_access_key_id",
            "_akamai_client_token",
            "_akamai_access_token",
            "_hyosung_api_key",
            "_lguplus_username",
            "_kt_username",
        ] {
            let key = format!("{}{}", id, suffix);
            if let Ok(entry) = Entry::new(KEYRING_SERVICE, &key) {
                let _ = entry.delete_password();
            }
        }
        let profiles_path = self.profiles_path();
        let data_dir = self.data_dir.clone();
        let id_owned = id.to_owned();
        let updated = self
            .with_profiles_lock(move || {
                let mut list = read_profiles_from_disk(&profiles_path)?;
                list.retain(|p| p.id != id_owned);
                atomic_write_json(&data_dir, &profiles_path, &list)?;
                Ok(list)
            })
            .await?;
        *self.profiles.write().await = updated;
        Ok(())
    }

    pub async fn get_credentials(&self, profile_id: &str) -> Result<AwsCredentials> {
        {
            let locked = self.profiles.read().await;
            locked
                .iter()
                .find(|p| p.id == profile_id)
                .context("프로필을 찾을 수 없음")?;
        }
        let secret = Entry::new(KEYRING_SERVICE, profile_id)
            .context("Keyring entry 생성 실패")?
            .get_password()
            .context("Keyring에서 자격증명 로드 실패")?;
        let access_key_id = Entry::new(KEYRING_SERVICE, &format!("{}_access_key_id", profile_id))
            .context("Access Key ID keyring entry 생성 실패")?
            .get_password()
            .context("Access Key ID를 keyring에서 불러오지 못했습니다")?;
        Ok(AwsCredentials {
            access_key_id,
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
                let (host, cdn_domain, cp_code) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("프로필을 찾을 수 없음")?;
                    (
                        profile.akamai_host.clone().unwrap_or_default(),
                        provider_domain(profile, "akamai").unwrap_or_default(),
                        profile.akamai_cp_code.clone(),
                    )
                }; // RwLockReadGuard 해제 후 keyring 호출
                let akamai_key = format!("{}_akamai", profile_id);
                let client_secret = Entry::new(KEYRING_SERVICE, &akamai_key)
                    .context("Akamai Keyring entry 생성 실패")?
                    .get_password()
                    .context("Akamai Keyring에서 자격증명 로드 실패")?;
                let client_token = Entry::new(KEYRING_SERVICE, &format!("{}_akamai_client_token", profile_id))
                    .ok()
                    .and_then(|e| e.get_password().ok())
                    .unwrap_or_default();
                let access_token = Entry::new(KEYRING_SERVICE, &format!("{}_akamai_access_token", profile_id))
                    .ok()
                    .and_then(|e| e.get_password().ok())
                    .unwrap_or_default();
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
                let (service_name, volume_name, endpoint, cdn_domain, service_type) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.lguplus_service_name.clone().unwrap_or_default(),
                        profile.lguplus_volume_name.clone().unwrap_or_default(),
                        profile.lguplus_endpoint.clone()
                            .unwrap_or_else(|| "https://api.lgucdn.com".to_owned()),
                        provider_domain(profile, "lguplus").unwrap_or_default(),
                        profile.lguplus_service_type.clone()
                            .filter(|v| !v.trim().is_empty())
                            .unwrap_or_else(|| "volume".to_owned()),
                    )
                };
                let username = Entry::new(KEYRING_SERVICE, &format!("{}_lguplus_username", profile_id))
                    .ok()
                    .and_then(|e| e.get_password().ok())
                    .unwrap_or_default();
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
                    service_type,
                })
            }
            "hyosung" => {
                let (endpoint, cdn_domain) = {
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
                        ep,
                        provider_domain(profile, "hyosung").unwrap_or_default(),
                    )
                };
                let api_key = Entry::new(KEYRING_SERVICE, &format!("{}_hyosung_api_key", profile_id))
                    .ok()
                    .and_then(|e| e.get_password().ok())
                    .unwrap_or_default();
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
                let (service_name, volume_name, endpoint, cdn_domain, service_type) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.kt_service_name.clone().unwrap_or_default(),
                        profile.kt_volume_name.clone().unwrap_or_default(),
                        profile.kt_endpoint.clone()
                            .unwrap_or_else(|| "https://api.ktcdn.co.kr".to_owned()),
                        provider_domain(profile, "kt").unwrap_or_default(),
                        profile.kt_service_type.clone()
                            .filter(|v| !v.trim().is_empty())
                            .unwrap_or_else(|| "volume".to_owned()),
                    )
                };
                let username = Entry::new(KEYRING_SERVICE, &format!("{}_kt_username", profile_id))
                    .ok()
                    .and_then(|e| e.get_password().ok())
                    .unwrap_or_default();
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
                    service_type,
                })
            }
            other => Err(anyhow::anyhow!("지원하지 않는 CDN 공급자: {}", other)),
        }
    }

    /// settings.json을 읽는다. 파일이 없거나 파싱에 실패하면 기본값을 반환한다.
    async fn read_settings(&self) -> AppSettings {
        let path = self.settings_path();
        if !path.exists() {
            return AppSettings::default();
        }
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AppSettings::default(),
        }
    }

    async fn write_settings(&self, settings: &AppSettings) -> Result<()> {
        tokio::fs::write(
            self.settings_path(),
            serde_json::to_string_pretty(settings).context("설정 직렬화 실패")?,
        )
        .await
        .context("설정 파일 쓰기 실패")
    }

    /// H-7: 마지막 연결 프로필 ID 저장 — 다른 설정 필드를 덮어쓰지 않도록 read-modify-write
    pub async fn save_last_profile_id(&self, id: &str) -> Result<()> {
        let mut settings = self.read_settings().await;
        settings.last_profile_id = Some(id.to_owned());
        self.write_settings(&settings).await
    }

    /// H-7: 마지막 연결 프로필 ID 조회
    pub async fn get_last_profile_id(&self) -> Result<Option<String>> {
        Ok(self.read_settings().await.last_profile_id)
    }

    /// 앱 전역 설정 조회 (감사 로그 상세 레벨 등)
    pub async fn get_app_settings(&self) -> Result<AppSettings> {
        Ok(self.read_settings().await)
    }

    /// CDN API 감사 로그 상세 레벨(응답 본문 포함 여부) 저장
    pub async fn save_detailed_audit_log(&self, enabled: bool) -> Result<()> {
        let mut settings = self.read_settings().await;
        settings.detailed_audit_log = enabled;
        self.write_settings(&settings).await
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
    if profile
        .access_key_id
        .as_deref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
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
    profile.access_key_id = profile
        .access_key_id
        .take()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
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

/// 과거 버전이 profiles.json에 평문으로 남겨둔 인증 관련 필드 하나를 keyring으로 옮기고
/// 구조체에서 제거한다. 옮길 값이 없으면 아무 것도 하지 않고 false를 반환한다.
fn migrate_legacy_auth_field(profile_id: &str, suffix: &str, field: &mut Option<String>) -> bool {
    let Some(value) = field.take() else { return false };
    if value.is_empty() {
        return false;
    }
    let key = format!("{}{}", profile_id, suffix);
    if let Ok(entry) = Entry::new(KEYRING_SERVICE, &key) {
        let _ = entry.set_password(&value);
    }
    true
}

/// profiles.json을 디스크에서 동기적으로 읽는다. 파일이 없으면 빈 목록을 반환한다.
/// `ProfileStore::with_profiles_lock`으로 잠근 구간 안에서, 다른 인스턴스가 그 사이에
/// 저장한 최신 내용을 반영하기 위해 캐시(`self.profiles`) 대신 이 함수로 다시 읽는다.
fn read_profiles_from_disk(path: &Path) -> Result<Vec<ProfileConfig>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(path).context("프로필 파일 읽기 실패")?;
    serde_json::from_str(&content).context("프로필 JSON 파싱 실패")
}

/// 같은 디렉터리에 임시 파일로 쓴 뒤 rename으로 교체한다. 쓰기 도중 크래시하거나
/// 다른 프로세스가 동시에 읽어도, rename은 원자적이므로 손상된 파일이 보이지 않는다.
fn atomic_write_json<T: Serialize>(dir: &Path, path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value).context("JSON 직렬화 실패")?;
    let tmp_path = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&tmp_path, json).context("임시 파일 쓰기 실패")?;
    std::fs::rename(&tmp_path, path).context("프로필 파일 교체 실패")?;
    Ok(())
}

#[cfg(test)]
mod profile_store_lock_tests {
    use super::*;

    fn dummy_profile(id: &str) -> ProfileConfig {
        ProfileConfig {
            id: id.to_owned(),
            name: id.to_owned(),
            scope: None,
            permissions: None,
            region: "us-east-1".to_owned(),
            bucket: "test-bucket".to_owned(),
            base_prefix: None,
            access_key_id: None,
            secret_access_key: None,
            endpoint: None,
            cdn_provider: None,
            cdn_providers: vec![],
            cdn_distribution_id: None,
            cdn_domain: None,
            cdn_base_path: None,
            purge_on_new_upload: false,
            purge_policy: None,
            upload_policy: None,
            metadata_policy: None,
            log_shipping: None,
            auth_binding: None,
            default_cache_control: None,
            content_type_override: None,
            multipart_etag_fallback: false,
            akamai_client_token: None,
            akamai_client_secret: None,
            akamai_access_token: None,
            akamai_host: None,
            akamai_cp_code: None,
            lguplus_username: None,
            lguplus_password: None,
            lguplus_service_name: None,
            lguplus_volume_name: None,
            lguplus_endpoint: None,
            lguplus_service_type: None,
            kt_username: None,
            kt_password: None,
            kt_service_name: None,
            kt_volume_name: None,
            kt_endpoint: None,
            kt_service_type: None,
            hyosung_api_key: None,
            hyosung_api_secret: None,
            hyosung_endpoint: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    /// 여러 "인스턴스"(별도 ProfileStore, 같은 data_dir)가 동시에 서로 다른 프로필을
    /// upsert해도, 잠금 없이 캐시된 목록을 그대로 덮어쓰던 예전 방식과 달리 유실 없이
    /// 전부 파일에 반영되어야 한다.
    #[tokio::test]
    async fn concurrent_saves_from_multiple_instances_do_not_lose_data() {
        let data_dir = std::env::temp_dir().join(format!("nexuspurge-lock-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();

        const WRITERS: usize = 20;
        let mut handles = Vec::with_capacity(WRITERS);
        for i in 0..WRITERS {
            let dir = data_dir.clone();
            handles.push(tokio::spawn(async move {
                // 각 태스크가 별개의 ProfileStore(=별개의 NexusPurge 인스턴스)를 흉내낸다.
                let store = ProfileStore::with_data_dir(dir);
                let profile = dummy_profile(&format!("profile-{i}"));
                let profiles_path = store.profiles_path();
                let data_dir = store.data_dir.clone();
                store
                    .with_profiles_lock(move || {
                        let mut list = read_profiles_from_disk(&profiles_path)?;
                        match list.iter().position(|p| p.id == profile.id) {
                            Some(pos) => list[pos] = profile,
                            None => list.push(profile),
                        }
                        atomic_write_json(&data_dir, &profiles_path, &list)?;
                        Ok(())
                    })
                    .await
                    .unwrap();
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }

        let final_list = read_profiles_from_disk(&data_dir.join(PROFILES_FILENAME)).unwrap();
        assert_eq!(final_list.len(), WRITERS, "동시 저장 중 일부 프로필이 유실됨");

        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
