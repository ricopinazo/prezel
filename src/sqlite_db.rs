use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, bail};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::EncodingKey;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::Serialize;
use walkdir::WalkDir;

use crate::{
    container::{sqld::SqldContainer, Container},
    db::nano_id::NanoId,
    deployments::worker::WorkerHandle,
    paths::HostFolder,
};

#[derive(Debug)]
pub(crate) struct ProdSqliteDb {
    pub(crate) setup: SqliteDbSetup,
    project_folder: PathBuf,
    build_queue: WorkerHandle,
}

impl ProdSqliteDb {
    // TODO: build_queue is needed in case the container needs to trigger its own build because
    // someone is trying to access it. But this never happens for sqld containers...
    #[tracing::instrument]
    pub(crate) fn new(project_id: &NanoId, build_queue: WorkerHandle) -> anyhow::Result<Self> {
        let project_folder = Path::new("sqlite").join(project_id.as_str());
        let path = project_folder.join("main");
        let folder = HostFolder::new(path);

        let auth = SqldAuth::generate();
        let container = SqldContainer::new(folder.clone(), &auth.key, build_queue.clone()).into();

        Ok(Self {
            setup: SqliteDbSetup {
                folder,
                container,
                auth,
            },
            project_folder,
            build_queue,
        })
    }

    #[tracing::instrument]
    pub(crate) fn branch(&self, deployment_id: &NanoId) -> BranchSqliteDb {
        let path = self.project_folder.join(deployment_id.as_str());
        let branch_folder = HostFolder::new(path);
        let auth = SqldAuth::generate();
        BranchSqliteDb {
            base_folder: self.setup.folder.clone(),
            branch_folder,
            build_queue: self.build_queue.clone(),
            auth,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BranchSqliteDb {
    base_folder: HostFolder,
    pub(crate) branch_folder: HostFolder,
    build_queue: WorkerHandle,
    pub(crate) auth: SqldAuth,
}

impl BranchSqliteDb {
    #[tracing::instrument]
    pub(crate) async fn setup(&self) -> anyhow::Result<SqliteDbSetup> {
        recursive_copy(
            &self.base_folder.get_container_path(),
            &self.branch_folder.get_container_path(),
        )
        .await?;
        let container = SqldContainer::new(
            self.branch_folder.clone(),
            &self.auth.key,
            self.build_queue.clone(),
        )
        .into();
        Ok(SqliteDbSetup {
            folder: self.branch_folder.clone(),
            container,
            auth: self.auth.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SqliteDbSetup {
    pub(crate) folder: HostFolder,
    pub(crate) container: Arc<Container>,
    pub(crate) auth: SqldAuth,
}

#[derive(Debug, Clone)]
pub(crate) struct SqldAuth {
    pub(crate) token: String,
    key: String,
}

impl SqldAuth {
    #[tracing::instrument]
    fn generate() -> Self {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let key = URL_SAFE_NO_PAD.encode(pair.public_key().as_ref());

        let token = Token {
            id: None,
            a: None,
            p: None,
            exp: None,
        };
        let token = encode(&token, &encoding_key);

        Self { key, token }
    }
}

// usize is not the actual type accepted by sqld. Have a look at libsql repo for more info
#[derive(serde::Deserialize, serde::Serialize, Debug, Default)]
pub struct Token {
    #[serde(default)]
    id: Option<usize>,
    #[serde(default)]
    a: Option<usize>,
    #[serde(default)]
    pub(crate) p: Option<usize>,
    #[serde(default)]
    exp: Option<usize>,
}

fn encode<T: Serialize>(claims: &T, key: &EncodingKey) -> String {
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::EdDSA);
    jsonwebtoken::encode(&header, &claims, key).unwrap()
}

#[tracing::instrument]
async fn recursive_copy(from: &Path, to: &Path) -> anyhow::Result<()> {
    for entry_result in WalkDir::new(from) {
        let entry = entry_result?;
        let relative = entry.path().strip_prefix(from)?;
        let new_path = to.join(relative);
        if entry.file_type().is_dir() {
            // we ignore errors expecting file creations to fail later on if something
            // maybe I should check the error is actually:File exists
            // or maybe check if the directory already exists, if not, create it
            let _ = tokio::fs::create_dir(&new_path).await;
        } else if entry.file_type().is_file() {
            tokio::fs::copy(entry.path(), &new_path)
                .await
                .map_err(|e| {
                    anyhow!(
                        "error when trying to copy file from {:?} to {new_path:?}: {e}",
                        entry.path(),
                    )
                })?;
        } else if entry.file_type().is_symlink() {
            let points_to = std::fs::read_link(entry.path())?;
            if points_to.is_relative() {
                let as_absolute = entry.path().join(&points_to);
                let inside_folder = as_absolute.strip_prefix(from);
                if inside_folder.is_err() {
                    bail!("trying to copy folder that contains relative link pointing outside the folder")
                }
                std::os::unix::fs::symlink(points_to, new_path)?;
            } else {
                bail!("trying to copy folder that contains absolute links")
            }
        } else {
            bail!(
                "trying to copy folder that contains unsupported file type {:?}",
                entry.file_type()
            )
        }
    }
    Ok(())
}
