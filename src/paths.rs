use std::{
    env,
    fs::create_dir_all,
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
│          ├── postgres
│          └── deployments
│                └── 10c1b2a4-39f6-4144-8620-a11e56b3232c
│                      ├── libsql -> this is a libsql branch
│                      └── postgres

*/

pub(crate) fn get_config_path() -> PathBuf {
    get_container_root().join("config.json")
}

pub(crate) fn get_acme_account_path() -> PathBuf {
    get_container_root().join("acme-account")
}

pub(crate) fn get_instance_db_path() -> PathBuf {
    get_container_root().join("app.db")
}

pub(crate) fn get_log_dir() -> PathBuf {
    get_container_root().join("log")
}

fn get_certs_dir() -> PathBuf {
    get_container_root().join("certs")
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

fn get_apps_dir() -> HostFolder {
    HostFolder::new("apps".into())
}

pub(crate) fn get_app_dir(id: &str) -> HostFolder {
    get_apps_dir().join(id)
}

pub(crate) fn get_propd_libqsl_dir(id: &str) -> HostFolder {
    get_app_dir(id).join("libsql")
}

pub(crate) fn get_deployment_dir(app: &str, deployment: &str) -> HostFolder {
    get_app_dir(app).join("deployments").join(deployment)
}

pub(crate) fn get_libsql_branch_dir(app: &str, deployment: &str) -> HostFolder {
    get_deployment_dir(app, deployment).join("libsql")
}

#[derive(Debug, Clone)]
pub(crate) struct HostFolder {
    relative_path: PathBuf,
}

impl HostFolder {
    pub(crate) fn new(relative_path: PathBuf) -> Self {
        // TODO: panic if relative_folder_path is not relative
        Self { relative_path }
    }

    pub(crate) fn get_host_path(&self) -> PathBuf {
        get_host_root().join(&self.relative_path)
    }

    pub(crate) fn get_container_path(&self) -> PathBuf {
        let path = get_container_root().join(&self.relative_path);
        create_dir_all(&path).unwrap(); // TODO: is this good enough?
        path
    }

    fn join(&self, path: &str) -> Self {
        Self {
            relative_path: self.relative_path.join(path),
        }
    }
}

fn get_host_root() -> PathBuf {
    env::var("PREZEL_HOME").unwrap().into()
}

const CONTAINER_ROOT: &'static str = "/opt/prezel";

fn get_container_root() -> &'static Path {
    Path::new(CONTAINER_ROOT)
}
