use eunomia::credentials::KeyStore;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn set_and_get_roundtrip() {
    let data = TempDir::new().unwrap();
    let ks = KeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "secret-key").await.unwrap();
    let got = ks.get("user-a").await.unwrap();
    assert_eq!(got.as_deref(), Some("secret-key"));
}

#[tokio::test]
async fn get_missing_returns_none() {
    let data = TempDir::new().unwrap();
    let ks = KeyStore::new(data.path().to_path_buf(), None);
    assert_eq!(ks.get("missing-user").await.unwrap(), None);
}

#[tokio::test]
async fn per_user_isolation() {
    let data = TempDir::new().unwrap();
    let ks = KeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "key-a").await.unwrap();
    ks.set("user-b", "key-b").await.unwrap();
    assert_eq!(ks.get("user-a").await.unwrap().as_deref(), Some("key-a"));
    assert_eq!(ks.get("user-b").await.unwrap().as_deref(), Some("key-b"));
}

#[tokio::test]
#[cfg(unix)]
async fn file_permissions_0600() {
    let data = TempDir::new().unwrap();
    let ks = KeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "key-a").await.unwrap();
    let path = data.path().join("users/user-a/credentials");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[tokio::test]
async fn no_global_credentials_file() {
    let data = TempDir::new().unwrap();
    let ks = KeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "key-a").await.unwrap();
    let per_user = data.path().join("users/user-a/credentials");
    assert!(per_user.is_file());
    assert!(!data.path().join("credentials").exists());
    assert!(!PathBuf::from("credentials").exists());
}
