//! Native credential storage only. No plaintext files, shell arguments, logs
//! or provider responses ever contain an API key. Session-only is the default.
use crate::AppResult;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};
use zeroize::{Zeroize, Zeroizing};

const SERVICE: &str = "org.handymeeting.text-models.v1";
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CredentialStatus {
    pub state: String,
    pub provider: &'static str,
    pub message: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SaveCredential {
    pub endpoint: String,
    pub mode: String,
    pub api_key: String,
    pub replace_existing: bool,
}
impl Drop for SaveCredential {
    fn drop(&mut self) {
        self.api_key.zeroize();
    }
}
#[derive(Clone, Copy, Debug)]
enum StoreFailure {
    Unavailable,
    Unreadable,
    Ambiguous,
    WriteDenied,
}
impl StoreFailure {
    fn message(self) -> &'static str {
        match self {
        Self::Unavailable=>"Der geschützte Betriebssystem-Speicher ist gesperrt oder nicht verfügbar. Bitte dort entsperren und erneut versuchen; nach einem Startfehler die App neu starten. Vorhandene Schlüssel bleiben erhalten; es gibt keinen Klartext-Ersatz.",
        Self::Unreadable=>"Ein gespeicherter Schlüssel kann nicht sicher gelesen werden. Er wird weder überschrieben noch entfernt. Bitte den Betriebssystem-Speicher prüfen.",
        Self::Ambiguous=>"Mehrere geschützte Schlüsseleinträge passen zu dieser Adresse. Bitte die Einträge im Betriebssystem prüfen; keiner wird automatisch geändert.",
        Self::WriteDenied=>"Der Betriebssystem-Speicher hat die Änderung nicht bestätigt. Bitte dessen Zugriff prüfen. Es wird kein Klartext-Ersatz gespeichert.",
    }
    }
}
trait SecretStore {
    fn read(&self, account: &str) -> Result<Option<Zeroizing<String>>, StoreFailure>;
    fn write(&self, account: &str, value: &str) -> Result<(), StoreFailure>;
    fn delete(&self, account: &str) -> Result<(), StoreFailure>;
}
struct NativeStore {
    service: String,
}
impl NativeStore {
    fn entry(&self, account: &str) -> Result<keyring::Entry, StoreFailure> {
        keyring::Entry::new(&self.service, account).map_err(map_error)
    }
}
fn map_error(error: keyring::Error) -> StoreFailure {
    match error {
        keyring::Error::BadEncoding(mut value) | keyring::Error::BadDataFormat(mut value, _) => {
            value.zeroize();
            StoreFailure::Unreadable
        }
        keyring::Error::Ambiguous(_) => StoreFailure::Ambiguous,
        _ => StoreFailure::Unavailable,
    }
}
impl SecretStore for NativeStore {
    fn read(&self, account: &str) -> Result<Option<Zeroizing<String>>, StoreFailure> {
        match self.entry(account)?.get_password() {
            Ok(value) => {
                let value = Zeroizing::new(value);
                if valid_key(&value) {
                    Ok(Some(value))
                } else {
                    Err(StoreFailure::Unreadable)
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_error(error)),
        }
    }
    fn write(&self, account: &str, value: &str) -> Result<(), StoreFailure> {
        self.entry(account)?
            .set_password(value)
            .map_err(|_| StoreFailure::WriteDenied)
    }
    fn delete(&self, account: &str) -> Result<(), StoreFailure> {
        self.entry(account)?
            .delete_credential()
            .map_err(|_| StoreFailure::WriteDenied)
    }
}
fn provider() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos-keychain"
    }
    #[cfg(target_os = "linux")]
    {
        "linux-secret-service"
    }
    #[cfg(windows)]
    {
        "windows-credential-manager"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        "unsupported"
    }
}
fn valid_key(key: &str) -> bool {
    !key.trim().is_empty() && key.len() <= 4096 && !key.chars().any(char::is_control)
}
struct Vault<S: SecretStore> {
    store: S,
    lock_root: PathBuf,
}
impl<S: SecretStore> Vault<S> {
    fn lock(&self) -> AppResult<fs::File> {
        fs::create_dir_all(&self.lock_root)
            .map_err(|_| "Die Zugriffssperre für den geschützten Speicher ist nicht verfügbar.")?;
        let path = self.lock_root.join("credential-access.lock");
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.file_type().is_file() {
                return Err(
                    "Ungültige Zugriffssperre; gespeicherte Schlüssel bleiben erhalten.".into(),
                );
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|_| "Geschützter Speicher kann derzeit nicht sicher angesprochen werden.")?;
        file.try_lock_exclusive().map_err(|_|"Ein anderes Fenster greift gerade auf gespeicherte Schlüssel zu. Bitte erneut versuchen.")?;
        Ok(file)
    }
    fn status(&self, account: &str) -> AppResult<CredentialStatus> {
        let _lease = self.lock()?;
        Ok(match self.store.read(account) {
            Ok(Some(_)) => status("stored", None),
            Ok(None) => status("missing", None),
            Err(error) => status("unavailable", Some(error.message().into())),
        })
    }
    fn save(&self, account: &str, key: &str, replace: bool) -> AppResult<CredentialStatus> {
        if !valid_key(key) {
            return Err(
                "Der API-Schlüssel ist leer, zu lang oder enthält unzulässige Steuerzeichen."
                    .into(),
            );
        }
        let _lease = self.lock()?;
        match self.store.read(account).map_err(|e|e.message())? {
            Some(_) if !replace=>return Err("Für diese Adresse existiert bereits ein geschützter Schlüssel. Ersetzen bitte ausdrücklich bestätigen.".into()),
            _=>{},
        }
        self.store.write(account, key).map_err(|e| e.message())?;
        let confirmed = self.store.read(account).map_err(|e| e.message())?;
        if confirmed.as_ref().is_none_or(|value| value.as_str() != key) {
            return Err("Der geschützte Speicher bestätigt den neuen Schlüssel nicht. Bitte dort prüfen; es wird kein Ersatz gespeichert.".into());
        }
        Ok(status("stored", None))
    }
    fn remove(&self, account: &str, confirm: bool) -> AppResult<CredentialStatus> {
        if !confirm {
            return Err(
                "Entfernen des gespeicherten Schlüssels bitte ausdrücklich bestätigen.".into(),
            );
        }
        let _lease = self.lock()?;
        if self.store.read(account).map_err(|e| e.message())?.is_some() {
            self.store.delete(account).map_err(|e| e.message())?;
        }
        if self.store.read(account).map_err(|e| e.message())?.is_some() {
            return Err(
                "Der Betriebssystem-Speicher bestätigt das Entfernen nicht. Bitte dort prüfen."
                    .into(),
            );
        }
        Ok(status("missing", None))
    }
    fn load(&self, account: &str) -> AppResult<Zeroizing<String>> {
        let _lease = self.lock()?;
        self.store.read(account).map_err(|e|e.message())?.ok_or_else(||"Für diese genaue KI-Adresse ist kein Schlüssel gespeichert. Bitte einen Schlüssel nur für diese Sitzung eingeben oder ausdrücklich speichern.".into())
    }
}
fn status(state: &str, message: Option<String>) -> CredentialStatus {
    CredentialStatus {
        state: state.into(),
        provider: provider(),
        message,
    }
}
fn native() -> AppResult<Vault<NativeStore>> {
    let lock_root = dirs::data_local_dir()
        .ok_or("Der geschützte Speicher kann ohne Benutzerprofil nicht sicher verwendet werden.")?
        .join("HandyMeeting")
        .join("credential-access");
    Ok(Vault {
        store: NativeStore {
            service: SERVICE.into(),
        },
        lock_root,
    })
}
pub(crate) fn credential_status(endpoint: &str, mode: &str) -> AppResult<CredentialStatus> {
    let account = meeting_notes::credential_account(endpoint, mode).map_err(|e| e.to_string())?;
    native()?.status(&account)
}
pub(crate) fn save_credential(request: &SaveCredential) -> AppResult<CredentialStatus> {
    let account = meeting_notes::credential_account(&request.endpoint, &request.mode)
        .map_err(|e| e.to_string())?;
    native()?.save(&account, &request.api_key, request.replace_existing)
}
pub(crate) fn remove_credential(
    endpoint: &str,
    mode: &str,
    confirm: bool,
) -> AppResult<CredentialStatus> {
    let account = meeting_notes::credential_account(endpoint, mode).map_err(|e| e.to_string())?;
    native()?.remove(&account, confirm)
}
pub(crate) fn load_for_run(
    endpoint: &str,
    mode: &str,
    cloud_consent: bool,
) -> AppResult<Zeroizing<String>> {
    if mode == "cloud" && !cloud_consent {
        return Err(
            "Cloud benötigt eine ausdrückliche Freigabe ausschließlich für den Text dieses Laufs."
                .into(),
        );
    }
    let account = meeting_notes::credential_account(endpoint, mode).map_err(|e| e.to_string())?;
    native()?.load(&account)
}

/// Explicit CI/manual native-provider test. Unique service, no user secrets.
/// The only deletion is of the synthetic entry created in this invocation.
pub(crate) fn native_checks(output: &Path) -> AppResult<()> {
    if output.exists() {
        return Err(
            "Der Prüfpfad existiert bereits; vorhandene Nachweise bleiben erhalten.".into(),
        );
    }
    fs::create_dir_all(output).map_err(|_| "Prüfpfad konnte nicht angelegt werden.")?;
    let nonce = meeting_notes::new_job_id();
    let service = format!("org.handymeeting.credential-check.{nonce}");
    let vault = Vault {
        store: NativeStore {
            service: service.clone(),
        },
        lock_root: output.join("access"),
    };
    let account = "isolated-synthetic-entry";
    let first = Zeroizing::new(format!("synthetic-first-{nonce}"));
    let second = Zeroizing::new(format!("synthetic-second-{nonce}"));
    let result = (|| -> AppResult<Vec<&str>> {
        let initial = vault.status(account)?;
        if initial.state == "unavailable" {
            return Err(initial
                .message
                .unwrap_or_else(|| "Der native Prüfspeicher ist nicht verfügbar.".into()));
        }
        if initial.state != "missing" {
            return Err(
                "Isolierter Prüfnamespace ist nicht leer; keine Änderung vorgenommen.".into(),
            );
        }
        vault.save(account, &first, false)?;
        if vault.load(account)?.as_str() != first.as_str() {
            return Err("Native Rückleseprüfung fehlgeschlagen.".into());
        }
        if vault.save(account, &second, false).is_ok()
            || vault.load(account)?.as_str() != first.as_str()
        {
            return Err("Unbestätigtes Ersetzen wurde nicht sicher abgewehrt.".into());
        }
        vault.save(account, &second, true)?;
        let reopened = Vault {
            store: NativeStore { service },
            lock_root: output.join("access"),
        };
        if reopened.load(account)?.as_str() != second.as_str() {
            return Err("Native Wiederöffnungsprüfung fehlgeschlagen.".into());
        }
        vault.remove(account, true)?;
        if vault.status(account)?.state != "missing" {
            return Err("Synthetischer Prüfeintrag wurde nicht entfernt.".into());
        }
        Ok(vec![
            "isolated-empty-namespace",
            "native-store-read-back",
            "unconfirmed-overwrite-refused",
            "explicit-replace-and-reopen",
            "delete-only-own-synthetic-entry",
        ])
    })();
    // Best-effort cleanup is restricted to an exact known synthetic value in
    // our unique namespace. Never delete an ambiguous/unreadable/foreign entry.
    if result.is_err() {
        if let Ok(Some(value)) = vault.store.read(account) {
            if value.as_str() == first.as_str() || value.as_str() == second.as_str() {
                let _ = vault.store.delete(account);
            }
        }
    }
    let value = serde_json::json!({"success":result.is_ok(),"applicationVersion":env!("CARGO_PKG_VERSION"),"provider":provider(),"actualNativeStore":true,"simulated":false,"checks":result.as_ref().map_or(0,|names|names.len()),"checkNames":result.as_ref().ok(),"error":result.as_ref().err(),"usesRealProviderCredentials":false,"usesUserEntries":false,"isolatedNamespace":true,"cloudRequests":false});
    fs::write(
        output.join("credential-checks.json"),
        serde_json::to_vec_pretty(&value)
            .map_err(|_| "Prüfergebnis konnte nicht serialisiert werden.")?,
    )
    .map_err(|_| "Prüfergebnis konnte nicht gespeichert werden.")?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    struct Fake {
        value: RefCell<Option<String>>,
        failure: RefCell<Option<StoreFailure>>,
        writes: RefCell<usize>,
        deletes: RefCell<usize>,
        deny: bool,
    }
    impl SecretStore for Fake {
        fn read(&self, _: &str) -> Result<Option<Zeroizing<String>>, StoreFailure> {
            if let Some(e) = *self.failure.borrow() {
                Err(e)
            } else {
                Ok(self.value.borrow().clone().map(Zeroizing::new))
            }
        }
        fn write(&self, _: &str, value: &str) -> Result<(), StoreFailure> {
            if self.deny {
                return Err(StoreFailure::WriteDenied);
            }
            *self.writes.borrow_mut() += 1;
            *self.value.borrow_mut() = Some(value.into());
            Ok(())
        }
        fn delete(&self, _: &str) -> Result<(), StoreFailure> {
            *self.deletes.borrow_mut() += 1;
            *self.value.borrow_mut() = None;
            Ok(())
        }
    }
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "handymeeting-simulated-vault-{}",
            meeting_notes::new_job_id()
        ))
    }
    #[test]
    fn simulated_unreadable_locked_or_ambiguous_entries_are_never_overwritten_or_deleted() {
        for error in [
            StoreFailure::Unavailable,
            StoreFailure::Unreadable,
            StoreFailure::Ambiguous,
        ] {
            let vault = Vault {
                store: Fake {
                    value: RefCell::new(Some("synthetic-original".into())),
                    failure: RefCell::new(Some(error)),
                    ..Default::default()
                },
                lock_root: root(),
            };
            assert_eq!(vault.status("fixture").unwrap().state, "unavailable");
            assert!(vault.save("fixture", "synthetic-new", true).is_err());
            assert!(vault.remove("fixture", true).is_err());
            assert_eq!(*vault.store.writes.borrow(), 0);
            assert_eq!(*vault.store.deletes.borrow(), 0);
            assert_eq!(
                vault.store.value.borrow().as_deref(),
                Some("synthetic-original")
            );
        }
    }
    #[test]
    fn simulated_opt_in_and_write_denial_keep_existing_values_and_secrets_out_of_status() {
        let vault = Vault {
            store: Fake {
                value: RefCell::new(Some("synthetic-original".into())),
                ..Default::default()
            },
            lock_root: root(),
        };
        assert!(vault.save("fixture", "synthetic-new", false).is_err());
        assert!(vault.remove("fixture", false).is_err());
        vault.save("fixture", "synthetic-new", true).unwrap();
        let text = serde_json::to_string(&vault.status("fixture").unwrap()).unwrap();
        assert!(!text.contains("synthetic"));
        let denied = Vault {
            store: Fake {
                value: RefCell::new(Some("synthetic-original".into())),
                deny: true,
                ..Default::default()
            },
            lock_root: root(),
        };
        assert!(denied.save("fixture", "synthetic-new", true).is_err());
        assert_eq!(
            denied.store.value.borrow().as_deref(),
            Some("synthetic-original")
        );
        assert_eq!(
            fs::read(denied.lock_root.join("credential-access.lock")).unwrap(),
            b""
        );
    }
    #[test]
    fn keyring_bad_encoding_is_not_echoed_in_messages() {
        let sentinel = b"synthetic-sensitive-error-bytes".to_vec();
        let mapped = map_error(keyring::Error::BadEncoding(sentinel));
        assert!(!mapped.message().contains("synthetic-sensitive"));
    }
    #[test]
    #[ignore = "Explicit native-provider check; creates and deletes only a unique synthetic entry"]
    fn native_provider_round_trip_with_isolated_namespace() {
        let output = std::env::var_os("HANDYMEETING_CREDENTIAL_CHECK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root().join("native-result"));
        native_checks(&output).unwrap();
        let bytes = fs::read(output.join("credential-checks.json")).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("synthetic-first"));
        assert!(!text.contains("synthetic-second"));
        let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["actualNativeStore"], true);
        assert_eq!(result["simulated"], false);
        assert_eq!(result["checks"], 5);
    }
}
