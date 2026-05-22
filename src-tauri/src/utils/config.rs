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
    #[serde(rename = "cdnDistributionId")]
    pub cdn_distribution_id: Option<String>,
    #[serde(rename = "cdnDomain")]
    pub cdn_domain: Option<String>,
    #[serde(rename = "purgeOnNewUpload", default)]
    pub purge_on_new_upload: bool,
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
    #[serde(rename = "lguplusApiKey", skip_serializing_if = "Option::is_none")]
    pub lguplus_api_key: Option<String>,
    #[serde(rename = "lguplusApiSecret", skip_serializing_if = "Option::is_none")]
    pub lguplus_api_secret: Option<String>,
    #[serde(rename = "lguplusEndpoint", skip_serializing_if = "Option::is_none")]
    pub lguplus_endpoint: Option<String>,
    #[serde(rename = "hyosungApiKey", skip_serializing_if = "Option::is_none")]
    pub hyosung_api_key: Option<String>,
    #[serde(rename = "hyosungApiSecret", skip_serializing_if = "Option::is_none")]
    pub hyosung_api_secret: Option<String>,
    #[serde(rename = "hyosungEndpoint", skip_serializing_if = "Option::is_none")]
    pub hyosung_endpoint: Option<String>,
    #[serde(rename = "ktApiKey", skip_serializing_if = "Option::is_none")]
    pub kt_api_key: Option<String>,
    #[serde(rename = "ktApiSecret", skip_serializing_if = "Option::is_none")]
    pub kt_api_secret: Option<String>,
    #[serde(rename = "ktEndpoint", skip_serializing_if = "Option::is_none")]
    pub kt_endpoint: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
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
    },
    Lguplus {
        api_key: String,
        api_secret: String,
        endpoint: String,
        cdn_domain: String,
    },
    Hyosung {
        api_key: String,
        api_secret: String,
        endpoint: String,
        cdn_domain: String,
    },
    Kt {
        api_key: String,
        api_secret: String,
        endpoint: String,
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
        validate_profile(&profile)?;

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
        if let Some(secret) = profile.lguplus_api_secret.take() {
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
        if let Some(secret) = profile.kt_api_secret.take() {
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
                let (client_token, access_token, host, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("?꾨줈?뚯씪??李얠쓣 ???놁쓬")?;
                    (
                        profile.akamai_client_token.clone().unwrap_or_default(),
                        profile.akamai_access_token.clone().unwrap_or_default(),
                        profile.akamai_host.clone().unwrap_or_default(),
                        profile.cdn_domain.clone().unwrap_or_default(),
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
                })
            }
            "lguplus" => {
                let (api_key, endpoint, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.lguplus_api_key.clone().unwrap_or_default(),
                        profile.lguplus_endpoint.clone().unwrap_or_default(),
                        profile.cdn_domain.clone().unwrap_or_default(),
                    )
                };
                if api_key.trim().is_empty()
                    || endpoint.trim().is_empty()
                    || cdn_domain.trim().is_empty()
                {
                    return Err(anyhow::anyhow!("LG U+ CDN credentials are incomplete"));
                }
                let lguplus_key = format!("{}_lguplus", profile_id);
                let api_secret = Entry::new(KEYRING_SERVICE, &lguplus_key)
                    .context("LG U+ Keyring entry creation failed")?
                    .get_password()
                    .context("LG U+ Keyring load failed")?;
                Ok(CdnCredentials::Lguplus {
                    api_key,
                    api_secret,
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
                    (
                        profile.hyosung_api_key.clone().unwrap_or_default(),
                        profile.hyosung_endpoint.clone().unwrap_or_default(),
                        profile.cdn_domain.clone().unwrap_or_default(),
                    )
                };
                if api_key.trim().is_empty()
                    || endpoint.trim().is_empty()
                    || cdn_domain.trim().is_empty()
                {
                    return Err(anyhow::anyhow!("Hyosung CDN credentials are incomplete"));
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
                let (api_key, endpoint, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.kt_api_key.clone().unwrap_or_default(),
                        profile.kt_endpoint.clone().unwrap_or_default(),
                        profile.cdn_domain.clone().unwrap_or_default(),
                    )
                };
                if api_key.trim().is_empty()
                    || endpoint.trim().is_empty()
                    || cdn_domain.trim().is_empty()
                {
                    return Err(anyhow::anyhow!("KT CDN credentials are incomplete"));
                }
                let kt_key = format!("{}_kt", profile_id);
                let api_secret = Entry::new(KEYRING_SERVICE, &kt_key)
                    .context("KT Keyring entry creation failed")?
                    .get_password()
                    .context("KT Keyring load failed")?;
                Ok(CdnCredentials::Kt {
                    api_key,
                    api_secret,
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
            if profile
                .cdn_domain
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!("CloudFront CDN domain is required"));
            }
            Ok(())
        }
        Some("akamai") => {
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
                ("Akamai CDN domain", profile.cdn_domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some("lguplus") => {
            for (label, value) in [
                ("LG U+ CDN API Key", profile.lguplus_api_key.as_deref()),
                ("LG U+ CDN Endpoint", profile.lguplus_endpoint.as_deref()),
                ("LG U+ CDN Domain", profile.cdn_domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some("hyosung") => {
            for (label, value) in [
                ("Hyosung CDN API Key", profile.hyosung_api_key.as_deref()),
                ("Hyosung CDN Endpoint", profile.hyosung_endpoint.as_deref()),
                ("Hyosung CDN Domain", profile.cdn_domain.as_deref()),
            ] {
                if value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                    return Err(anyhow::anyhow!("{} is required", label));
                }
            }
            Ok(())
        }
        Some("kt") => {
            for (label, value) in [
                ("KT CDN API Key", profile.kt_api_key.as_deref()),
                ("KT CDN Endpoint", profile.kt_endpoint.as_deref()),
                ("KT CDN Domain", profile.cdn_domain.as_deref()),
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
