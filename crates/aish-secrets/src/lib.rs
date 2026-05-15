//! aish-secrets — 跨平台 keyring 凭证存储。
//!
//! service 固定 "aish"，username 用 HostId 的 UUID 字符串。
//! 调用方在 macOS Keychain Access.app / Windows 凭据管理器 搜 "aish" 能列出全部条目。

use aish_types::HostId;

#[cfg(not(test))]
use keyring::Entry;

/// 固定 service name；user 在 OS keyring 里看到的应用名。
#[allow(dead_code)]
const SERVICE: &str = "aish";

/// 跨平台 keyring 凭证存储。
///
/// 所有方法都是同步的（keyring crate 3.x 的 sync API）— 操作通常 < 10ms。
/// 调用方应当在异步上下文里用 `tokio::task::spawn_blocking` 或在 cx.spawn 内部调用。
pub struct SecretStore;

impl SecretStore {
    /// 把 password 存到 keyring，绑定到 host_id。
    /// 已存在则覆盖。
    pub fn set(host_id: HostId, password: &str) -> Result<(), SecretError> {
        #[cfg(test)]
        {
            tests::mock_set(host_id.to_string(), password.to_string());
            Ok(())
        }
        #[cfg(not(test))]
        {
            let entry = Entry::new(SERVICE, &host_id.to_string())?;
            entry.set_password(password)?;
            Ok(())
        }
    }

    /// 取出 host_id 对应的 password。
    /// 不存在返回 `SecretError::NoEntry`。
    pub fn get(host_id: HostId) -> Result<String, SecretError> {
        #[cfg(test)]
        {
            tests::mock_get(&host_id.to_string()).ok_or(SecretError::NoEntry)
        }
        #[cfg(not(test))]
        {
            let entry = Entry::new(SERVICE, &host_id.to_string())?;
            match entry.get_password() {
                Ok(p) => Ok(p),
                Err(keyring::Error::NoEntry) => Err(SecretError::NoEntry),
                Err(e) => Err(SecretError::Keyring(e)),
            }
        }
    }

    /// 删除 host_id 对应的 entry。
    /// 不存在视为成功（调用方期望 idempotent 删除）。
    pub fn delete(host_id: HostId) -> Result<(), SecretError> {
        #[cfg(test)]
        {
            tests::mock_delete(&host_id.to_string());
            Ok(())
        }
        #[cfg(not(test))]
        {
            let entry = Entry::new(SERVICE, &host_id.to_string())?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(SecretError::Keyring(e)),
            }
        }
    }

    // ==========================================================================
    // SSH key passphrase 系列（加密私钥解密用）。
    //
    // username key 用 `{host_id}-passphrase`，与 password (host_id) 在 keyring
    // 内不同 entry，互不冲突。一个 host 理论上可以同时存 password (回退用) +
    // passphrase (KeyFile 解密用)，但实际 SshAuth 是 enum 互斥，单 host 只
    // 用一个。
    // ==========================================================================

    fn passphrase_username(host_id: HostId) -> String {
        format!("{}-passphrase", host_id)
    }

    pub fn set_passphrase(host_id: HostId, passphrase: &str) -> Result<(), SecretError> {
        let user = Self::passphrase_username(host_id);
        #[cfg(test)]
        {
            tests::mock_set(user, passphrase.to_string());
            Ok(())
        }
        #[cfg(not(test))]
        {
            let entry = Entry::new(SERVICE, &user)?;
            entry.set_password(passphrase)?;
            Ok(())
        }
    }

    pub fn get_passphrase(host_id: HostId) -> Result<String, SecretError> {
        let user = Self::passphrase_username(host_id);
        #[cfg(test)]
        {
            tests::mock_get(&user).ok_or(SecretError::NoEntry)
        }
        #[cfg(not(test))]
        {
            let entry = Entry::new(SERVICE, &user)?;
            match entry.get_password() {
                Ok(p) => Ok(p),
                Err(keyring::Error::NoEntry) => Err(SecretError::NoEntry),
                Err(e) => Err(SecretError::Keyring(e)),
            }
        }
    }

    pub fn delete_passphrase(host_id: HostId) -> Result<(), SecretError> {
        let user = Self::passphrase_username(host_id);
        #[cfg(test)]
        {
            tests::mock_delete(&user);
            Ok(())
        }
        #[cfg(not(test))]
        {
            let entry = Entry::new(SERVICE, &user)?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(SecretError::Keyring(e)),
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring 操作失败: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("条目不存在")]
    NoEntry,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    thread_local! {
        static MOCK_STORE: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    }

    pub(crate) fn mock_set(key: String, value: String) {
        MOCK_STORE.with(|store| {
            store.lock().unwrap().insert(key, value);
        });
    }

    pub(crate) fn mock_get(key: &str) -> Option<String> {
        MOCK_STORE.with(|store| store.lock().unwrap().get(key).cloned())
    }

    pub(crate) fn mock_delete(key: &str) {
        MOCK_STORE.with(|store| {
            store.lock().unwrap().remove(key);
        });
    }

    pub(crate) fn setup() {
        MOCK_STORE.with(|store| {
            store.lock().unwrap().clear();
        });
    }

    use super::*;

    #[test]
    fn set_then_get_returns_value() {
        setup();
        let host = HostId::new();
        SecretStore::set(host, "p@ss123").unwrap();
        let got = SecretStore::get(host).unwrap();
        assert_eq!(got, "p@ss123");
    }

    #[test]
    fn get_nonexistent_returns_no_entry() {
        setup();
        let host = HostId::new();
        let r = SecretStore::get(host);
        assert!(matches!(r, Err(SecretError::NoEntry)));
    }

    #[test]
    fn delete_then_get_returns_no_entry() {
        setup();
        let host = HostId::new();
        SecretStore::set(host, "x").unwrap();
        SecretStore::delete(host).unwrap();
        let r = SecretStore::get(host);
        assert!(matches!(r, Err(SecretError::NoEntry)));
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        setup();
        let host = HostId::new();
        SecretStore::delete(host).unwrap();
    }

    #[test]
    fn set_overwrites_existing() {
        setup();
        let host = HostId::new();
        SecretStore::set(host, "old").unwrap();
        SecretStore::set(host, "new").unwrap();
        assert_eq!(SecretStore::get(host).unwrap(), "new");
    }

    // SSH key passphrase 系列

    #[test]
    fn passphrase_set_then_get() {
        setup();
        let host = HostId::new();
        SecretStore::set_passphrase(host, "phr@s3").unwrap();
        let got = SecretStore::get_passphrase(host).unwrap();
        assert_eq!(got, "phr@s3");
    }

    #[test]
    fn passphrase_independent_from_password() {
        // 同 host 的 password 与 passphrase 在 keyring 不同 entry，互不干扰
        setup();
        let host = HostId::new();
        SecretStore::set(host, "pw").unwrap();
        SecretStore::set_passphrase(host, "phrase").unwrap();
        assert_eq!(SecretStore::get(host).unwrap(), "pw");
        assert_eq!(SecretStore::get_passphrase(host).unwrap(), "phrase");
    }

    #[test]
    fn passphrase_delete_independent() {
        setup();
        let host = HostId::new();
        SecretStore::set(host, "pw").unwrap();
        SecretStore::set_passphrase(host, "phrase").unwrap();
        SecretStore::delete_passphrase(host).unwrap();
        // password 仍在
        assert_eq!(SecretStore::get(host).unwrap(), "pw");
        // passphrase 已删
        assert!(matches!(
            SecretStore::get_passphrase(host),
            Err(SecretError::NoEntry)
        ));
    }
}
