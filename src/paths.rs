use std::{
    fs::{create_dir_all, read_dir},
    path::{Path, PathBuf},
};

/* structure sample:

prezel
├── config.json
├── app.db
├── acme-account
├── log
│    ├── log
│    └── log.20250301T103835
├── certs
│    ├── httpe5.i.lencr.org.pem
│    ├── httpe6.i.lencr.org.pem
│    └── *.civilian-puffin.018294.xyz
│          ├── cert.pem
│          └── key.pem
├── apps
│    └── 6220587f-4888-4709-989e-95ac08056a5e
│          ├── libsql -> this is the prod libsql db
│          └── postgres
├── deployments
│    └── 10c1b2a4-39f6-4144-8620-a11e56b3232c
│          ├── libsql -> this is the branch libsql db, if any
│          └── postgres

*/

pub(crate) fn get_config_path() -> PathBuf {
    get_root().join("config.json")
}

pub(crate) fn get_acme_account_path() -> PathBuf {
    get_root().join("acme-account")
}

pub(crate) fn get_instance_db_path() -> PathBuf {
    get_root().join("app.db")
}

pub(crate) fn get_log_dir() -> PathBuf {
    get_root().join("log")
}

fn get_certs_dir() -> PathBuf {
    get_root().join("certs")
}

pub(crate) fn get_intermediate_domain_path(intermediate_domain: &str) -> PathBuf {
    let filename = intermediate_domain.to_owned() + ".pem";
    get_certs_dir().join(&filename)
}

fn get_domain_path(domain: &str) -> PathBuf {
    let path = get_certs_dir().join(domain);
    create_dir_all(&path).unwrap(); // FIXME: this is a bit random, need to decide what the general logic for this is
    path
}

pub(crate) fn get_domain_cert_path(domain: &str) -> PathBuf {
    get_domain_path(domain).join("cert.pem")
}

pub(crate) fn get_domain_key_path(domain: &str) -> PathBuf {
    get_domain_path(domain).join("key.pem")
}

// TODO: make this return PathBuf ?
fn get_apps_dir() -> PathBuf {
    get_root().join("apps").create_if_missing()
}

pub(crate) fn get_all_app_dirs() -> impl Iterator<Item = PathBuf> {
    iter_dir(&get_apps_dir())
}

pub(crate) fn get_app_dir(id: &str) -> PathBuf {
    get_apps_dir().join(id)
}

pub(crate) fn get_propd_libqsl_dir(id: &str) -> PathBuf {
    get_app_dir(id).join("libsql").create_if_missing()
}

// TODO: make this return PathBuf ?
pub(crate) fn get_deployments_dir() -> PathBuf {
    get_root().join("deployments").create_if_missing()
}

pub(crate) fn get_deployment_dir(deployment: &str) -> PathBuf {
    get_deployments_dir().join(deployment)
}

pub(crate) fn get_all_deployment_dirs() -> impl Iterator<Item = PathBuf> {
    iter_dir(&get_deployments_dir())
}

pub(crate) fn get_libsql_branch_dir(deployment: &str) -> PathBuf {
    get_deployment_dir(deployment)
        .join("libsql")
        .create_if_missing()
}

fn iter_dir(path: &Path) -> impl Iterator<Item = PathBuf> {
    let paths = read_dir(path)
        .map(|paths| paths.collect::<Vec<_>>())
        .unwrap_or(vec![]);
    paths
        .into_iter()
        .filter_map(|entry| Some(entry.ok()?.path()))
}

const ROOT: &'static str = "/opt/prezel";

fn get_root() -> &'static Path {
    Path::new(ROOT)
}

trait CreateIfMissing {
    fn create_if_missing(self) -> Self;
}

impl CreateIfMissing for PathBuf {
    fn create_if_missing(self) -> Self {
        create_dir_all(&self).unwrap();
        self
    }
}
