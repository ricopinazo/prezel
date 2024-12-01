use std::path::PathBuf;

use instant_acme::{Account, AccountCredentials, NewAccount};
use tokio::fs;

use crate::paths::get_container_root;

fn account_credentials_path() -> PathBuf {
    get_container_root().join("acme-account")
}

pub(crate) async fn read_account() -> anyhow::Result<Account> {
    let content = fs::read_to_string(account_credentials_path()).await?;
    let credentials: AccountCredentials = serde_json::from_str(&content)?;
    let account = Account::from_credentials(credentials).await?;
    println!("Using saved acme account credentials");
    Ok(account)
}

pub(crate) async fn persist_credentials(credentials: &AccountCredentials) {
    println!("Saving new acme account credentials");
    let content = serde_json::to_string(credentials).unwrap();
    fs::write(account_credentials_path(), content)
        .await
        .unwrap();
}

pub(crate) async fn create_new_account(acme_service: &str) -> (Account, AccountCredentials) {
    println!("Creating new acme account");
    let (account, credentials) = Account::create(
        &NewAccount {
            contact: &[],
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        acme_service,
        None,
    )
    .await
    .unwrap();
    (account, credentials)
}
