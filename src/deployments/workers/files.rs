use std::sync::Arc;

use crate::{
    deployments::{manager::InstrumentedRwLock, map::DeploymentMap, worker::Worker},
    paths::{get_all_app_dirs, get_all_deployment_dirs},
};

#[derive(Debug)]
pub(crate) struct FilesWorker {
    pub(crate) map: Arc<InstrumentedRwLock<DeploymentMap>>,
}

impl Worker for FilesWorker {
    fn work(&self) -> impl std::future::Future<Output = ()> + Send {
        async {
            for path in get_all_app_dirs() {
                let app_id = path.file_name().unwrap().to_str().unwrap().to_owned();
                if !self.map.read().await.prod.contains_key(&app_id.into()) {
                    let _ = tokio::fs::remove_dir_all(path);
                }
            }
            for path in get_all_deployment_dirs() {
                let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
                let deployment_id = file_name.into();
                if !self.map.read().await.has_deployment_id(&deployment_id) {
                    let _ = tokio::fs::remove_dir_all(path);
                }
            }
        }
    }
}
