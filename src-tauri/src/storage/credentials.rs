use std::fs;
use std::path::PathBuf;
use super::settings::get_app_data_dir;

pub struct CredentialsProvider;

#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case)]
struct CREDENTIALW {
    Flags: u32,
    Type: u32,
    TargetName: *mut u16,
    Comment: *mut u16,
    LastWritten: [u32; 2],
    CredentialBlobSize: u32,
    CredentialBlob: *mut u8,
    Persist: u32,
    AttributeCount: u32,
    Attributes: *mut std::ffi::c_void,
    TargetAlias: *mut u16,
    UserName: *mut u16,
}

#[cfg(windows)]
#[link(name = "advapi32")]
extern "system" {
    fn CredReadW(
        TargetName: *const u16,
        Type: u32,
        Flags: u32,
        Credential: *mut *mut CREDENTIALW,
    ) -> i32;

    fn CredWriteW(
        Credential: *const CREDENTIALW,
        Flags: u32,
    ) -> i32;

    fn CredDeleteW(TargetName: *const u16, Type: u32, Flags: u32) -> i32;

    fn CredFree(Buffer: *mut std::ffi::c_void);
}

impl CredentialsProvider {
    /// Sanitizes an API key by stripping all control characters, null bytes (`\0`),
    /// Unicode byte order marks (BOM), zero-width spaces, and whitespace.
    /// Only printable ASCII graphic characters (0x21..=0x7E) are retained.
    pub fn sanitize_key(raw: &str) -> String {
        raw.chars()
            .filter(|c| c.is_ascii_graphic())
            .collect::<String>()
    }

    /// Decodes a raw byte buffer that may be UTF-8 or UTF-16LE (e.g. from PowerShell Set-Content).
    fn decode_raw_key_bytes(raw_bytes: &[u8]) -> String {
        let decoded = if raw_bytes.contains(&0) && raw_bytes.len() >= 2 && raw_bytes.len() % 2 == 0 {
            let u16_vec: Vec<u16> = raw_bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16_vec)
        } else {
            String::from_utf8_lossy(raw_bytes).to_string()
        };
        Self::sanitize_key(&decoded)
    }

    fn get_key_file() -> PathBuf {
        get_app_data_dir().join(".groq_key")
    }

    fn get_local_program_key_file() -> Option<PathBuf> {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            Some(PathBuf::from(local).join("Programs").join("Rift").join(".groq_key"))
        } else {
            None
        }
    }

    pub fn get_api_key() -> Option<String> {
        // 1. Environment variable
        if let Ok(key) = std::env::var("GROQ_API_KEY") {
            let sanitized = Self::sanitize_key(&key);
            if !sanitized.is_empty() {
                return Some(sanitized);
            }
        }

        // Prefer Windows Credential Manager; plaintext files below are legacy migration sources.
        #[cfg(windows)]
        for target in &["Rift", "groq_api_key@Rift"] {
            if let Some(key) = Self::read_wincred(target) {
                let sanitized = Self::sanitize_key(&key);
                if !sanitized.is_empty() {
                    let _ = Self::remove_legacy_groq_files();
                    return Some(sanitized);
                }
            }
        }

        // 2. Legacy key file in %APPDATA%\Rift\.groq_key
        let path = Self::get_key_file();
        if path.exists() {
            if let Ok(raw_bytes) = fs::read(&path) {
                let sanitized = Self::decode_raw_key_bytes(&raw_bytes);
                if !sanitized.is_empty() {
                    #[cfg(windows)]
                    if Self::write_wincred("Rift", "groq_api_key", &sanitized).is_ok() {
                        let _ = Self::remove_legacy_groq_files();
                    }
                    return Some(sanitized);
                }
            }
        }

        // 3. Fallback key file in %LOCALAPPDATA%\Programs\Rift\.groq_key
        if let Some(local_path) = Self::get_local_program_key_file() {
            if local_path.exists() {
                if let Ok(raw_bytes) = fs::read(&local_path) {
                    let sanitized = Self::decode_raw_key_bytes(&raw_bytes);
                    if !sanitized.is_empty() {
                        #[cfg(windows)]
                        if Self::write_wincred("Rift", "groq_api_key", &sanitized).is_ok() {
                            let _ = Self::remove_legacy_groq_files();
                        }
                        return Some(sanitized);
                    }
                }
            }
        }

        None
    }

    pub fn set_api_key(api_key: &str) -> Result<(), String> {
        let clean = Self::sanitize_key(api_key);
        if clean.is_empty() || clean.len() > 4096 {
            return Err("Groq API key must contain valid ASCII characters (non-empty)".to_string());
        }
        #[cfg(windows)]
        {
            Self::write_wincred("Rift", "groq_api_key", &clean)?;
            Self::remove_legacy_groq_files()?;
        }
        #[cfg(not(windows))]
        fs::write(Self::get_key_file(), clean)
            .map_err(|error| format!("Could not save Groq API key: {error}"))?;

        Ok(())
    }

    pub fn delete_api_key() -> Result<(), String> {
        #[cfg(windows)]
        for target in &["Rift", "groq_api_key@Rift"] {
            Self::delete_wincred(target)?;
        }
        Self::remove_legacy_groq_files()?;
        Ok(())
    }

    fn remove_legacy_groq_files() -> Result<(), String> {
        Self::remove_legacy_file(&Self::get_key_file())?;
        if let Some(path) = Self::get_local_program_key_file() {
            Self::remove_legacy_file(&path)?;
        }
        Ok(())
    }

    fn get_openrouter_key_file() -> PathBuf {
        get_app_data_dir().join(".openrouter_key")
    }

    fn get_local_program_openrouter_key_file() -> Option<PathBuf> {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            Some(PathBuf::from(local).join("Programs").join("Rift").join(".openrouter_key"))
        } else {
            None
        }
    }

    pub fn get_openrouter_api_key() -> Option<String> {
        // 1. Environment variable
        if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            let sanitized = Self::sanitize_key(&key);
            if !sanitized.is_empty() {
                return Some(sanitized);
            }
        }

        // Prefer Windows Credential Manager; plaintext files below are legacy migration sources.
        #[cfg(windows)]
        for target in &["Rift/OpenRouter", "openrouter_api_key@Rift"] {
            if let Some(key) = Self::read_wincred(target) {
                let sanitized = Self::sanitize_key(&key);
                if !sanitized.is_empty() {
                    let _ = Self::remove_legacy_openrouter_files();
                    return Some(sanitized);
                }
            }
        }

        // 2. Legacy key file in %APPDATA%\Rift\.openrouter_key
        let path = Self::get_openrouter_key_file();
        if path.exists() {
            if let Ok(raw_bytes) = fs::read(&path) {
                let sanitized = Self::decode_raw_key_bytes(&raw_bytes);
                if !sanitized.is_empty() {
                    #[cfg(windows)]
                    if Self::write_wincred("Rift/OpenRouter", "openrouter_api_key", &sanitized).is_ok() {
                        let _ = Self::remove_legacy_openrouter_files();
                    }
                    return Some(sanitized);
                }
            }
        }

        // 3. Fallback key file in %LOCALAPPDATA%\Programs\Rift\.openrouter_key
        if let Some(local_path) = Self::get_local_program_openrouter_key_file() {
            if local_path.exists() {
                if let Ok(raw_bytes) = fs::read(&local_path) {
                    let sanitized = Self::decode_raw_key_bytes(&raw_bytes);
                    if !sanitized.is_empty() {
                        #[cfg(windows)]
                        if Self::write_wincred("Rift/OpenRouter", "openrouter_api_key", &sanitized).is_ok() {
                            let _ = Self::remove_legacy_openrouter_files();
                        }
                        return Some(sanitized);
                    }
                }
            }
        }

        None
    }

    pub fn set_openrouter_api_key(api_key: &str) -> Result<(), String> {
        let clean = Self::sanitize_key(api_key);
        if clean.is_empty() || clean.len() > 4096 {
            return Err("OpenRouter API key must contain valid ASCII characters (non-empty)".to_string());
        }
        #[cfg(windows)]
        {
            Self::write_wincred("Rift/OpenRouter", "openrouter_api_key", &clean)?;
            Self::remove_legacy_openrouter_files()?;
        }
        #[cfg(not(windows))]
        fs::write(Self::get_openrouter_key_file(), clean)
            .map_err(|error| format!("Could not save OpenRouter API key: {error}"))?;

        Ok(())
    }

    pub fn delete_openrouter_api_key() -> Result<(), String> {
        #[cfg(windows)]
        for target in &["Rift/OpenRouter", "openrouter_api_key@Rift"] {
            Self::delete_wincred(target)?;
        }
        Self::remove_legacy_openrouter_files()?;
        Ok(())
    }

    fn remove_legacy_openrouter_files() -> Result<(), String> {
        Self::remove_legacy_file(&Self::get_openrouter_key_file())?;
        if let Some(path) = Self::get_local_program_openrouter_key_file() {
            Self::remove_legacy_file(&path)?;
        }
        Ok(())
    }

    fn remove_legacy_file(path: &PathBuf) -> Result<(), String> {
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("Could not remove legacy credential file: {error}"))?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn read_wincred(target_name: &str) -> Option<String> {
        let target_wide: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut p_cred: *mut CREDENTIALW = std::ptr::null_mut();

        // CRED_TYPE_GENERIC = 1
        let res = unsafe { CredReadW(target_wide.as_ptr(), 1, 0, &mut p_cred) };
        if res != 0 && !p_cred.is_null() {
            let cred = unsafe { &*p_cred };
            let blob_len = cred.CredentialBlobSize as usize;
            let blob_slice = unsafe { std::slice::from_raw_parts(cred.CredentialBlob, blob_len) };

            // Detect if the credential was stored as UTF-16LE (e.g. contains interleaved null bytes)
            let decoded = if blob_slice.contains(&0) && blob_len >= 2 && blob_len % 2 == 0 {
                let u16_slice = unsafe {
                    std::slice::from_raw_parts(cred.CredentialBlob as *const u16, blob_len / 2)
                };
                String::from_utf16_lossy(u16_slice)
            } else if let Ok(s) = std::str::from_utf8(blob_slice) {
                s.to_string()
            } else if blob_len >= 2 && blob_len % 2 == 0 {
                let u16_slice = unsafe {
                    std::slice::from_raw_parts(cred.CredentialBlob as *const u16, blob_len / 2)
                };
                String::from_utf16_lossy(u16_slice)
            } else {
                String::new()
            };

            unsafe { CredFree(p_cred as *mut _) };

            let sanitized = Self::sanitize_key(&decoded);
            if !sanitized.is_empty() {
                // Self-healing: if the stored blob was corrupted with null bytes (e.g. 112 bytes vs 56 bytes),
                // rewrite the clean UTF-8 credential to Windows Credential Manager immediately so it's permanently cured.
                if blob_len != sanitized.len() {
                    let user = if target_name.contains("OpenRouter") {
                        "openrouter_api_key"
                    } else {
                        "groq_api_key"
                    };
                    let _ = Self::write_wincred(target_name, user, &sanitized);
                }
                return Some(sanitized);
            }
        }

        None
    }

    #[cfg(not(windows))]
    fn read_wincred(_target_name: &str) -> Option<String> {
        None
    }

    #[cfg(windows)]
    fn write_wincred(target_name: &str, user_name: &str, secret: &str) -> Result<(), String> {
        let mut target_wide: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut user_wide: Vec<u16> = user_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut secret_bytes = secret.as_bytes().to_vec();

        let cred = CREDENTIALW {
            Flags: 0,
            Type: 1, // CRED_TYPE_GENERIC
            TargetName: target_wide.as_mut_ptr(),
            Comment: std::ptr::null_mut(),
            LastWritten: [0, 0],
            CredentialBlobSize: secret_bytes.len() as u32,
            CredentialBlob: secret_bytes.as_mut_ptr(),
            Persist: 2, // CRED_PERSIST_LOCAL_MACHINE
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: user_wide.as_mut_ptr(),
        };

        if unsafe { CredWriteW(&cred, 0) } == 0 {
            return Err(format!("Windows Credential Manager write failed: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    #[cfg(windows)]
    fn delete_wincred(target_name: &str) -> Result<(), String> {
        let target_wide: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();
        if unsafe { CredDeleteW(target_wide.as_ptr(), 1, 0) } == 0 {
            let error = std::io::Error::last_os_error();
            // ERROR_NOT_FOUND is the expected result when a credential was never saved.
            if error.raw_os_error() != Some(1168) {
                return Err(format!("Windows Credential Manager delete failed: {error}"));
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn write_wincred(_target_name: &str, _user_name: &str, _secret: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_key_strips_null_bytes_and_whitespace() {
        let dirty = "g\0s\0k\0_1\02\03\0abc\r\n\t ";
        let clean = CredentialsProvider::sanitize_key(dirty);
        assert_eq!(clean, "gsk_123abc");
    }

    #[test]
    fn test_sanitize_key_strips_unicode_and_bom() {
        let dirty = "\u{feff}gsk_secret_key_123\u{200b} ";
        let clean = CredentialsProvider::sanitize_key(dirty);
        assert_eq!(clean, "gsk_secret_key_123");
    }

    #[test]
    fn test_decode_raw_key_bytes_handles_utf16le() {
        // UTF-16LE encoded "gsk_test"
        let utf16_bytes: Vec<u8> = "gsk_test"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        assert_eq!(utf16_bytes.len(), 16);
        let decoded = CredentialsProvider::decode_raw_key_bytes(&utf16_bytes);
        assert_eq!(decoded, "gsk_test");
    }

    #[test]
    fn test_decode_raw_key_bytes_handles_utf8() {
        let utf8_bytes = b"gsk_clean_key_456\n";
        let decoded = CredentialsProvider::decode_raw_key_bytes(utf8_bytes);
        assert_eq!(decoded, "gsk_clean_key_456");
    }
}
