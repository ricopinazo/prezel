// use std::collections::HashSet;
// use std::hash::Hash;
// use std::ops::Deref;
// use std::path::{Path, PathBuf};
// use std::time::Duration;
// use std::{collections::HashMap, sync::Arc};

// use tokio::sync::{RwLock, RwLockReadGuard};
// use tokio::time::sleep;

// use crate::container::prisma::PrismaContainer;
// use crate::container::ContainerStatus;
// use crate::db::{Db, Deployment as DbDeployment, DeploymentWithProject, Status};
// use crate::deployment_hooks::{DeploymentHooks, StatusHooks};
// use crate::docker::{delete_container, list_managed_container_ids, stop_container, Docker};
// use crate::github::Github;
// use crate::paths::HostFile;
// use crate::{
//     container::{commit::CommitContainer, Container},
//     work_queue::WorkQueue,
// };

// #[derive(Debug)]
// pub(crate) struct Deployment {
//     pub(crate) branch: Option<String>,
//     pub(crate) sha: String,
//     pub(crate) id: i64,
//     pub(crate) project: i64,
//     pub(crate) url_id: String,
//     pub(crate) timestamp: i64,
//     // pub(crate) target_hostname: String,
//     // pub(crate) deployment_hostname: String,
//     // pub(crate) prisma_hostname: String,
//     pub(crate) forced_prod: bool, // TODO: review if im using this
//     pub(crate) app_container: Arc<Container>, // FIXME: try to remove Arc, only needed to make access to socket/public generic
//     pub(crate) prisma_container: Arc<Container>,
// }

// impl Deployment {
//     fn get_all_containers(&self) -> impl Iterator<Item = &Container> {
//         [self.app_container.as_ref(), self.prisma_container.as_ref()].into_iter()
//     }

//     // TODO:  try to merge this with the one above?
//     fn iter_arc_containers(&self) -> impl Iterator<Item = Arc<Container>> {
//         [self.app_container.clone(), self.prisma_container.clone()].into_iter()
//     }

//     fn new(
//         deployment: DeploymentWithProject,
//         box_hostname: &str,
//         github: Github,
//         db: Db,
//         docker: Docker,
//     ) -> Self {
//         let DeploymentWithProject {
//             deployment,
//             project,
//         } = deployment;
//         let DbDeployment {
//             sha,
//             env,
//             branch,
//             id,
//             url_id,
//             timestamp,
//             ..
//         } = deployment;
//         let project_name = project.name.clone();

//         let env = env.into();

//         let deployment_hostname = format!("{url_id}-{project_name}.{box_hostname}");
//         let prisma_hostname = format!("db-{url_id}-{project_name}.{box_hostname}");

//         let dbs_path = get_dbs_path(project.id);
//         let cloned_db_file = if branch.is_some() {
//             let path = dbs_path.join(id.to_string());
//             Some(HostFile::new(path, "preview.db"))
//         } else {
//             None
//         };
//         let main_db_file = HostFile::new(dbs_path, "main.db");

//         // TODO: this boilerplate is also in CommitContainer::new()
//         let db_file = cloned_db_file
//             .clone()
//             .unwrap_or_else(|| main_db_file.clone());

//         let public = branch.is_none();

//         let target_hostname = get_target_hostname(box_hostname, &project.name, &branch);

//         let hooks = StatusHooks::new(db, id);

//         let commit_container = CommitContainer::new(
//             hooks,
//             github,
//             docker.clone(),
//             project.repo_id.clone(),
//             sha.clone(),
//             id,
//             env,
//             public,
//             main_db_file,
//             cloned_db_file,
//         );
//         let prisma_container = PrismaContainer::new(db_file, docker);

//         Self {
//             branch,
//             sha,
//             id,
//             project: project.id,
//             url_id,
//             timestamp,
//             target_hostname,
//             deployment_hostname,
//             prisma_hostname,
//             forced_prod: project.prod_id.is_some_and(|prod_id| id == prod_id),
//             app_container: commit_container.into(),
//             prisma_container: prisma_container.into(),
//         }
//     }
// }

// #[derive(Clone)]
// pub(crate) struct DeploymentManager {
//     deployments: Arc<RwLock<HashMap<String, Deployment>>>,
//     queue: Arc<WorkQueue<Arc<Deployment>>>,
//     github: Github,
//     db: Db,
//     docker: Docker,
//     box_hostname: String,
// }

// impl DeploymentManager {
//     pub(crate) async fn new(box_hostname: String, github: Github, db: Db) -> Self {
//         let deployments = Arc::new(RwLock::new(HashMap::new()));
//         let queue: Arc<WorkQueue<Arc<Deployment>>> = Arc::new(WorkQueue::new());
//         let cloned_queue = queue.clone();
//         let docker = Default::default();

//         let store = Self {
//             deployments,
//             queue,
//             github,
//             docker,
//             db,
//             box_hostname,
//         };
//         let queue_store = store.clone();
//         let sync_store = store.clone();

//         tokio::spawn(async move {
//             loop {
//                 // FIXME: the fact that container.setup() returns a Result,
//                 // but I also need to check if the container status is actually StandBy,
//                 // seems a bit redundant. Try to fix that
//                 let result = {
//                     let head = cloned_queue.wait_head().await;
//                     head.app_container.setup().await
//                 };

//                 let deployment = cloned_queue.pop_head().await;
//                 if result.is_ok() {
//                     let status = deployment.app_container.status.read().await.clone();
//                     match status {
//                         ContainerStatus::StandBy { .. } => {
//                             queue_store.insert_promoted(deployment).await;
//                         }
//                         _ => {}
//                     }
//                 }
//             }
//         });

//         tokio::spawn(async move {
//             loop {
//                 sleep(Duration::from_secs(30)).await;
//                 sync_store.read_updates_from_db().await
//             }
//         });

//         store
//     }

//     async fn insert_promoted(&self, deployment: Arc<Deployment>) {
//         let mut deployments = self.deployments.write().await;

//         let target = if deployment.branch.is_some() {
//             Target::Branch(deployment.clone())
//         } else {
//             Target::Prod(deployment.clone())
//         };

//         let stale = deployments.insert(deployment.target_hostname.clone(), target);
//         deployments.insert(
//             deployment.deployment_hostname.clone(),
//             Target::Reference(deployment.target_hostname.clone()),
//         );
//         if let Some(stale) = stale.and_then(|target| target.into_deployment()) {
//             deployments.insert(stale.deployment_hostname.clone(), Target::StandBy(stale));
//         }

//         deployments.insert(
//             deployment.prisma_hostname.clone(),
//             Target::Prisma(deployment.deployment_hostname.clone()),
//         );
//     }

//     async fn insert_standby(&self, deployment: Arc<Deployment>) {
//         let mut deployments = self.deployments.write().await;
//         deployments.insert(
//             deployment.deployment_hostname.clone(),
//             Target::StandBy(deployment.clone()),
//         );
//         deployments.insert(
//             deployment.prisma_hostname.clone(),
//             Target::Prisma(deployment.deployment_hostname.clone()),
//         );
//     }

//     pub(crate) async fn get_container_by_hostname(&self, hostname: &str) -> Option<Arc<Container>> {
//         match self.deployments.read().await.get(hostname)? {
//             Target::Prod(deployment) | Target::Branch(deployment) | Target::StandBy(deployment) => {
//                 Some(deployment.app_container.clone())
//             }
//             Target::Reference(hostname) => Some(
//                 self.get_deployment_by_hostname(hostname)
//                     .await?
//                     .app_container
//                     .clone(),
//             ),
//             Target::Prisma(hostname) => Some(
//                 self.get_deployment_by_hostname(hostname)
//                     .await?
//                     .prisma_container
//                     .clone(),
//             ),
//         }
//     }

//     async fn get_deployment_by_hostname(&self, hostname: &str) -> Option<Arc<Deployment>> {
//         match self.deployments.read().await.get(hostname)? {
//             Target::Prod(deployment) | Target::Branch(deployment) | Target::StandBy(deployment) => {
//                 Some(deployment.clone())
//             }
//             Target::Reference(hostname) => {
//                 Box::pin(self.get_deployment_by_hostname(&hostname)).await
//             }
//             _ => None,
//         }
//     }

//     // this also returns deployments for prisma containers
//     async fn get_parent_deployment_by_hostname(&self, hostname: &str) -> Option<Arc<Deployment>> {
//         match self.deployments.read().await.get(hostname)? {
//             Target::Prod(deployment) | Target::Branch(deployment) | Target::StandBy(deployment) => {
//                 Some(deployment.clone())
//             }
//             Target::Reference(hostname) | Target::Prisma(hostname) => {
//                 Box::pin(self.get_parent_deployment_by_hostname(&hostname)).await
//             }
//         }
//     }

//     // async fn delete_deployment(&self, id: i64) {
//     //     // TODO: return result from here

//     //     if let Some(deployment) = self.get_deployment(id).await {
//     //         deployment.deployment.app_container.full_delete().await;
//     //         deployment.deployment.prisma_container.full_delete().await;

//     //         // FIXME: fail if the deployment is promoted
//     //         // also, try deleting deployments from the list before stopping the containers!!
//     //         let hostname = &deployment.deployment.deployment_hostname;
//     //         self.deployments
//     //             .write()
//     //             .await
//     //             .retain(|key, target| match (key, target) {
//     //                 (key, Target::StandBy(_)) if key == hostname => false,
//     //                 (_, Target::Reference(key)) if key == hostname => false,
//     //                 (_, Target::Prisma(key)) if key == hostname => false,
//     //                 _ => true,
//     //             });
//     //     }
//     // }

//     pub(crate) async fn get_prod_deployment(&self, project_id: i64) -> Option<DeploymentInfo> {
//         self.deployments.read().await.values().find_map(|target| {
//             if let Target::Prod(deployment) = target {
//                 if deployment.project == project_id {
//                     Some(DeploymentInfo {
//                         deployment: deployment.clone(),
//                         promoted: true,
//                     })
//                 } else {
//                     None
//                 }
//             } else {
//                 None
//             }
//         })
//     }

//     pub(crate) async fn get_deployment(&self, id: i64) -> Option<DeploymentInfo> {
//         self.get_all_deployments()
//             .await
//             .find(move |deployment| deployment.deployment.id == id)
//     }

//     // TODO: use read_updates_from_db for this instead
//     fn stop_unused_resources(&self) {
//         // I can simply call remove upon all Target::Deployment elements
//         // and stop upon all Target::Branch elements
//         // and if I need to free up more resources even stop older Target::Branch

//         todo!()
//     }

//     async fn is_container_in_use(&self, id: &String) -> bool {
//         for deployment in self.get_all_deployments().await {
//             for container in deployment.deployment.get_all_containers() {
//                 if container.get_container_id().await.as_ref() == Some(id) {
//                     return true;
//                 }
//             }
//         }
//         false
//     }

//     // TODO: try to return an iter
//     async fn get_all_non_prod_containers(&self) -> Vec<Arc<Container>> {
//         let deployments = self.deployments.read().await;
//         let all_containers_from_non_prod_deployments = deployments
//             .iter()
//             .filter_map(|(_, target)| match target {
//                 Target::Branch(deployment) | Target::StandBy(deployment) => Some(deployment),
//                 Target::Prod(_) | Target::Reference(_) | Target::Prisma(_) => None,
//             })
//             .flat_map(|deployment| deployment.iter_arc_containers());
//         let prisma_containers_from_prod_deployments =
//             deployments.iter().filter_map(|(_, target)| match target {
//                 Target::Prod(deployment) => Some(deployment.prisma_container.clone()),
//                 Target::Branch(_)
//                 | Target::StandBy(_)
//                 | Target::Reference(_)
//                 | Target::Prisma(_) => None,
//             });
//         all_containers_from_non_prod_deployments
//             .chain(prisma_containers_from_prod_deployments)
//             .collect()
//     }

//     pub(crate) async fn read_updates_from_db(&self) {
//         self.queue.clear().await;

//         let required = self.get_required_non_failed_deployments(&self.db).await;
//         let required_ids: HashSet<i64> = required
//             .iter()
//             .map(|request| request.deployment().id)
//             .collect();

//         let existing_deployment_ids: HashSet<i64> = self
//             .get_all_deployments()
//             .await
//             .map(|deployment| deployment.deployment.id)
//             .collect();

//         // add new deployments
//         for request in required {
//             if !existing_deployment_ids.contains(&request.deployment().id) {
//                 match request {
//                     Request::Promoted(deployment) => self.queue.insert(deployment.into()).await,
//                     Request::StandBy(deployment) => self.insert_standby(deployment.into()).await,
//                 }
//             }
//         }

//         // downgrade unused containers
//         for container in self.get_all_non_prod_containers().await {
//             container.downgrade_if_unused().await;
//         }

//         // remove unused containers
//         let mut hostnames_to_remove = vec![];
//         for (hostname, target) in self.deployments.read().await.deref() {
//             if let Some(deployment) = self.get_parent_deployment_by_hostname(hostname).await {
//                 // FIXME: what happens if a container is the prod postgres db and the parent deployment is a prod deployment
//                 // maybe that doesnt make sense?
//                 if !required_ids.contains(&deployment.id) {
//                     hostnames_to_remove.push(hostname.clone());
//                 }
//             }
//         }
//         for hostname in hostnames_to_remove {
//             self.deployments.write().await.remove(&hostname);
//         }

//         // Careful, don't remove a container that was just started but not wrote yet into a Ready status
//         for container in list_managed_container_ids().await.unwrap() {
//             if !self.is_container_in_use(&container).await {
//                 stop_container(&container).await;
//                 delete_container(&container).await;
//             }
//         }

//         // TODO: remove all the images that are not in use.
//         // Careful don't remove an image that was just built but not wrote yet into an StandBy status
//         // I can probably aquire the lock for the docker builder
//     }

//     async fn get_all_deployments(&self) -> impl Iterator<Item = DeploymentInfo> {
//         // TODO: try using RwLockReadGuard::try_map so I don't need Arcs, same for the other methods
//         let building = self
//             .queue
//             .read_full()
//             .await
//             .iter()
//             .cloned()
//             .map(|deployment| DeploymentInfo {
//                 deployment,
//                 promoted: false,
//             })
//             .collect::<Vec<_>>();
//         let ready = self
//             .deployments
//             .read()
//             .await
//             .values()
//             .filter_map(Target::get_info)
//             .collect::<Vec<_>>();
//         building.into_iter().chain(ready)
//     }

//     async fn get_required_non_failed_deployments(&self, db: &Db) -> Vec<Request> {
//         let box_hostname = &self.box_hostname;
//         let deployments = db
//             .get_deployments_with_project()
//             .await
//             .filter(|deployment| deployment.result != Some(Status::Failed))
//             .map(|deployment| {
//                 Deployment::new(
//                     deployment,
//                     box_hostname,
//                     self.github.clone(),
//                     db.clone(),
//                     self.docker.clone(),
//                 )
//             });

//         let mut requests = vec![];
//         let groups = group_by(deployments, |deployment| deployment.target_hostname.clone());
//         for (_, mut group) in groups.into_iter() {
//             group.sort_by_key(|deployment| {
//                 if deployment.forced_prod {
//                     i64::MAX
//                 } else {
//                     deployment.timestamp
//                 }
//             });
//             if let Some(deployment) = group.pop() {
//                 requests.push(Request::Promoted(deployment));
//             }
//             for deployment in group {
//                 requests.push(Request::StandBy(deployment));
//             }
//         }
//         requests
//     }
// }

// enum Request {
//     Promoted(Deployment),
//     StandBy(Deployment),
// }

// impl Request {
//     fn deployment(&self) -> &Deployment {
//         match self {
//             Self::Promoted(deployment) => deployment,
//             Self::StandBy(deployment) => deployment,
//         }
//     }
// }

// pub fn group_by<T, K, F>(x: T, f: F) -> HashMap<K, Vec<T::Item>>
// where
//     T: IntoIterator,
//     F: Fn(&T::Item) -> K,
//     K: Eq + Hash,
// {
//     let mut map = HashMap::new();
//     for item in x {
//         let hash = f(&item);
//         map.entry(hash).or_insert(vec![]).push(item);
//     }
//     map
// }

// fn get_dbs_path(project_id: i64) -> PathBuf {
//     Path::new("sqlite").join(project_id.to_string()) // FIXME: should use the id!!!!!!!!!!
// }

// fn get_target_hostname(box_hostname: &str, project_name: &str, branch: &Option<String>) -> String {
//     match branch {
//         Some(branch) => {
//             let safe_branch_name = branch.replace("/", "-");
//             format!("{safe_branch_name}-{project_name}.{box_hostname}")
//         }
//         None => format!("{project_name}.{box_hostname}"),
//     }
// }

// #[cfg(test)]
// mod deployment_tests {
//     use regex::Regex;

//     #[test]
//     fn test_env() {
//         let env = "DATABASE_URL=/eferge/ergerg";
//         let matches = Regex::new(r"([A-Za-z0-9_-]*)(?:=?)([\s\S]*)")
//             .unwrap()
//             .captures(env)
//             .unwrap();
//         if matches.get(2).unwrap().as_str() == "" {
//             // No value, pull from the current environment
//             let name = matches.get(1).unwrap().as_str();
//             // if let Ok(value) = env::var(name) {
//             //     environment.set_variable(name.to_string(), value);
//             // }
//         } else {
//             // Use provided name, value pair
//             // environment.set_variable(
//             //     matches.get(1).unwrap().as_str().to_string(),
//             //     matches.get(2).unwrap().as_str().to_string(),
//             // );
//         }
//     }
// }
