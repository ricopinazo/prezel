use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    container::{sqld::SqldContainer, Container},
    deployments::worker::WorkerHandle,
    paths::HostFile,
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
    pub(crate) fn new(project_id: i64, build_queue: WorkerHandle) -> anyhow::Result<Self> {
        let project_folder = Path::new("sqlite").join(project_id.to_string());
        let file = HostFile::new(project_folder.clone(), "main.db");

        let main_db_path = file.get_container_file();
        if !main_db_path.exists() {
            std::fs::File::create_new(&main_db_path)?;
        }

        let container = SqldContainer::new(file.clone(), build_queue.clone()).into();

        Ok(Self {
            setup: SqliteDbSetup { file, container },
            project_folder,
            build_queue,
        })
    }

    pub(crate) fn branch(&self, deployment_id: i64) -> BranchSqliteDb {
        let path = self.project_folder.join(deployment_id.to_string());
        let branch_file = HostFile::new(path, "preview.db");
        BranchSqliteDb {
            project_folder: self.project_folder.clone(),
            base_file: self.setup.file.clone(),
            branch_file,
            build_queue: self.build_queue.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BranchSqliteDb {
    project_folder: PathBuf,
    base_file: HostFile,
    pub(crate) branch_file: HostFile,
    build_queue: WorkerHandle,
}

impl BranchSqliteDb {
    pub(crate) async fn setup(&self) -> anyhow::Result<SqliteDbSetup> {
        tokio::fs::copy(
            self.base_file.get_container_file(),
            self.branch_file.get_container_file(),
        )
        .await?;
        let container =
            SqldContainer::new(self.branch_file.clone(), self.build_queue.clone()).into();
        Ok(SqliteDbSetup {
            file: self.branch_file.clone(),
            container,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SqliteDbSetup {
    pub(crate) file: HostFile,
    pub(crate) container: Arc<Container>,
}
