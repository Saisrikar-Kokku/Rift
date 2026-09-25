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

    fn CredFree(Buffer: *mut std::ffi::c_void);
}

impl CredentialsProvider {
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
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // 2. Primary key file in %APPDATA%\Rift\.groq_key
        let path = Self::get_key_file();
        if path.exists() {
            if let Ok(key) = fs::read_to_string(&path) {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        // 3. Fallback key file in %LOCALAPPDATA%\Programs\Rift\.groq_key
        if let Some(local_path) = Self::get_local_program_key_file() {
            if local_path.exists() {
                if let Ok(key) = fs::read_to_string(&local_path) {
                    let trimmed = key.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        // 4. Windows Credential Manager (Target: "Rift" or "groq_api_key@Rift")
        for target in &["Rift", "groq_api_key@Rift"] {
            if let Some(key) = Self::read_wincred(target) {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    // Auto-sync to file for faster subsequent reads
                    let _ = fs::write(&path, trimmed);
                    return Some(trimmed.to_string());
                }
            }
        }

        None
    }

    pub fn set_api_key(api_key: &str) -> Result<(), std::io::Error> {
        let trimmed = api_key.trim();
        let path = Self::get_key_file();
        fs::write(&path, trimmed)?;

        if let Some(local_path) = Self::get_local_program_key_file() {
            let _ = fs::write(local_path, trimmed);
        }

        // Also write to Windows Credential Manager
        Self::write_wincred("Rift", "groq_api_key", trimmed);

        Ok(())
    }

    pub fn delete_api_key() -> Result<(), std::io::Error> {
        let path = Self::get_key_file();
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        if let Some(local_path) = Self::get_local_program_key_file() {
            if local_path.exists() {
                let _ = fs::remove_file(local_path);
            }
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
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // 2. Primary key file in %APPDATA%\Rift\.openrouter_key
        let path = Self::get_openrouter_key_file();
        if path.exists() {
            if let Ok(key) = fs::read_to_string(&path) {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        // 3. Fallback key file in %LOCALAPPDATA%\Programs\Rift\.openrouter_key
        if let Some(local_path) = Self::get_local_program_openrouter_key_file() {
            if local_path.exists() {
                if let Ok(key) = fs::read_to_string(&local_path) {
                    let trimmed = key.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        // 4. Windows Credential Manager (Target: "Rift/OpenRouter" or "openrouter_api_key@Rift")
        for target in &["Rift/OpenRouter", "openrouter_api_key@Rift"] {
            if let Some(key) = Self::read_wincred(target) {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    let _ = fs::write(&path, trimmed);
                    return Some(trimmed.to_string());
                }
            }
        }

        None
    }

    pub fn set_openrouter_api_key(api_key: &str) -> Result<(), std::io::Error> {
        let trimmed = api_key.trim();
        let path = Self::get_openrouter_key_file();
        fs::write(&path, trimmed)?;

        if let Some(local_path) = Self::get_local_program_openrouter_key_file() {
            let _ = fs::write(local_path, trimmed);
        }

        Self::write_wincred("Rift/OpenRouter", "openrouter_api_key", trimmed);

        Ok(())
    }

    pub fn delete_openrouter_api_key() -> Result<(), std::io::Error> {
        let path = Self::get_openrouter_key_file();
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        if let Some(local_path) = Self::get_local_program_openrouter_key_file() {
            if local_path.exists() {
                let _ = fs::remove_file(local_path);
            }
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

            let key_str = if let Ok(s) = std::str::from_utf8(blob_slice) {
                Some(s.to_string())
            } else if blob_len >= 2 && blob_len % 2 == 0 {
                let u16_slice = unsafe {
                    std::slice::from_raw_parts(cred.CredentialBlob as *const u16, blob_len / 2)
                };
                Some(String::from_utf16_lossy(u16_slice))
            } else {
                None
            };

            unsafe { CredFree(p_cred as *mut _) };
            return key_str;
        }

        None
    }

    #[cfg(not(windows))]
    fn read_wincred(_target_name: &str) -> Option<String> {
        None
    }

    #[cfg(windows)]
    fn write_wincred(target_name: &str, user_name: &str, secret: &str) {
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

        unsafe {
            CredWriteW(&cred, 0);
        }
    }

    #[cfg(not(windows))]
    fn write_wincred(_target_name: &str, _user_name: &str, _secret: &str) {}
}
