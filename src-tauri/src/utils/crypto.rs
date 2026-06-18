use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const PBKDF2_ITER: u32 = 100_000;
const SALT_LEN: usize = 16;

#[derive(Serialize, Deserialize)]
struct EncryptedBlob {
    v: u8,
    salt: String,
    nonce: String,
    data: String,
}

pub fn encrypt(plaintext: &[u8], passphrase: &str) -> Result<String> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);

    let mut key_bytes = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, PBKDF2_ITER, &mut key_bytes);

    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("암호화 실패: {}", e))?;

    let blob = EncryptedBlob {
        v: 1,
        salt: STANDARD.encode(salt),
        nonce: STANDARD.encode(nonce.as_slice()),
        data: STANDARD.encode(ciphertext),
    };
    serde_json::to_string(&blob).context("JSON 직렬화 실패")
}

pub fn decrypt(blob_str: &str, passphrase: &str) -> Result<Vec<u8>> {
    let blob: EncryptedBlob =
        serde_json::from_str(blob_str).context("잘못된 암호화 파일 형식입니다")?;

    let salt = STANDARD.decode(&blob.salt).context("salt 디코딩 실패")?;
    let nonce_bytes = STANDARD.decode(&blob.nonce).context("nonce 디코딩 실패")?;
    let ciphertext = STANDARD.decode(&blob.data).context("암호문 디코딩 실패")?;

    let mut key_bytes = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, PBKDF2_ITER, &mut key_bytes);

    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("복호화 실패: 패스프레이즈가 틀리거나 파일이 손상되었습니다"))
}
