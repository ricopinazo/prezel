use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    deployments::{map::DeploymentMap, worker::Worker},
    docker::{delete_container, list_managed_container_ids, stop_container},
};

pub(crate) struct DockerWorker {
    pub(crate) map: Arc<RwLock<DeploymentMap>>,
}

impl Worker for DockerWorker {
    fn work(&self) -> impl std::future::Future<Output = ()> + Send {
        async {
            dbg!("running docker garbage collector");
            // Careful, don't remove a container that was just started but not wrote yet into a Ready status
            for container in list_managed_container_ids().await.unwrap() {
                dbg!(&container);
                if !self.is_container_in_use(&container).await {
                    dbg!("stopping");
                    stop_container(&container).await;
                    delete_container(&container).await;
                }
            }

            // TODO: remove all the images that are not in use.
            // Careful don't remove an image that was just built but not wrote yet into an StandBy status
            // I can probably aquire the lock for the docker builder
        }
    }
}

impl DockerWorker {
    async fn is_container_in_use(&self, id: &String) -> bool {
        for container in self.map.read().await.iter_containers() {
            if container.get_container_id().await.as_ref() == Some(id) {
                return true;
            }
        }
        false
    }
}
