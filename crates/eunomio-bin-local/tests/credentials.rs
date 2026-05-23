// SPDX-License-Identifier: Apache-2.0

use eunomio_core::traits::KeyStore;
use eunomio_keystore_file::FileKeyStore;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn set_and_get_roundtrip() {
    let data = TempDir::new().unwrap();
    let ks = FileKeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "secret-key").await.unwrap();
    let got = ks.get("user-a").await.unwrap();
    assert_eq!(got.as_deref(), Some("secret-key"));
}

#[tokio::test]
async fn get_missing_returns_none() {
    let data = TempDir::new().unwrap();
    let ks = FileKeyStore::new(data.path().to_path_buf(), None);
    assert_eq!(ks.get("missing-user").await.unwrap(), None);
}

#[tokio::test]
async fn per_user_isolation() {
    let data = TempDir::new().unwrap();
    let ks = FileKeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "key-a").await.unwrap();
    ks.set("user-b", "key-b").await.unwrap();
    assert_eq!(ks.get("user-a").await.unwrap().as_deref(), Some("key-a"));
    assert_eq!(ks.get("user-b").await.unwrap().as_deref(), Some("key-b"));
}

#[tokio::test]
async fn no_global_credentials_file() {
    let data = TempDir::new().unwrap();
    let ks = FileKeyStore::new(data.path().to_path_buf(), None);
    ks.set("user-a", "key-a").await.unwrap();
    let per_user = data.path().join("users/user-a/credentials");
    assert!(per_user.is_file());
    assert!(!data.path().join("credentials").exists());
    assert!(!PathBuf::from("credentials").exists());
}
