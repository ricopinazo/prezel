use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use futures::{stream, StreamExt};

use crate::{
    container::{Container, ContainerStatus},
    db::{BuildResult, Db},
    github::Github,
    tls::CertificateStore,
};

use super::{deployment::Deployment, worker::WorkerHandle};

#[derive(Debug)]
pub(crate) struct DeploymentMap {
    /// FIXME: this having a tuple (i64, String) as the key means every time I access I need to clone a String. There has to be another way
    pub(crate) deployments: HashMap<(i64, String), Deployment>,
    /// values here used to be options, but removing them from the map should be enough
    pub(crate) prod: HashMap<i64, String>,
    // pub(crate) ideal_prod: HashMap<i64, Option<String>>,
    pub(crate) names: HashMap<String, i64>,
    pub(crate) certificates: CertificateStore,
    pub(crate) custom_domains: HashMap<String, i64>,
}

impl DeploymentMap {
    pub(crate) fn new(store: CertificateStore) -> Self {
        Self {
            deployments: Default::default(),
            prod: Default::default(),
            names: Default::default(),
            custom_domains: Default::default(),
            certificates: store,
        }
    }
    pub(crate) fn iter_containers(&self) -> impl Iterator<Item = Arc<Container>> + '_ {
        self.deployments.iter().flat_map(|(_, deployment)| {
            [
                deployment.app_container.clone(),
                deployment.prisma_container.clone(),
            ] // FIXME: use here iter_arc_containers
            .into_iter()
        })
    }

    pub(crate) fn get_deployment(&self, project: &str, deployment: &str) -> Option<&Deployment> {
        let project_id = self.names.get(project)?;
        self.deployments.get(&(*project_id, deployment.to_string()))
    }

    fn get_prod_from_id(&self, id: i64) -> Option<&Deployment> {
        let prod_id = self.prod.get(&id)?;
        self.deployments.get(&(id, prod_id.to_string()))
    }

    pub(crate) fn get_prod(&self, project: &str) -> Option<&Deployment> {
        let project_id = self.names.get(project)?;
        self.get_prod_from_id(*project_id)
    }

    pub(crate) fn get_custom_domain(&self, domain: &str) -> Option<&Deployment> {
        let project = self.custom_domains.get(domain)?;
        self.get_prod_from_id(*project)
    }

    // TODO: this is currently kind of a mutex because is getting &mut,
    // but if that ever changes, I might need a way to make it mutex again
    pub(crate) async fn read_db_and_build_updates(
        &mut self,
        build_queue: &WorkerHandle,
        github: &Github,
        db: &Db,
    ) {
        dbg!();
        let required_deployments = db.get_deployments_with_project().await.collect::<Vec<_>>();
        dbg!();

        let required_ids = required_deployments
            .iter()
            .map(|dep| (dep.project.id, dep.deployment.url_id.clone()))
            .collect::<HashSet<_>>();
        dbg!();

        // sync map.names
        let projects = required_deployments
            .iter()
            .map(|dep| (dep.project.id, dep.project.clone()))
            .collect::<HashMap<_, _>>();
        self.names = projects
            .iter()
            .map(|(id, project)| (project.name.clone(), *id))
            .collect();
        dbg!();

        // sync map.custom_domains
        self.custom_domains = projects
            .iter()
            .flat_map(|(id, project)| {
                project
                    .custom_domains
                    .iter()
                    .map(|domain| (domain.to_owned(), *id))
            })
            .collect();

        // sync map.certificates
        let required_certificates = self.custom_domains.keys();
        for domain in required_certificates {
            if !self.certificates.has_domain(domain) {
                self.certificates.insert_domain(domain.to_owned());
            }
            // TODO: should also remove unneeded certificates?
        }
        dbg!();

        // sync map.deployments
        for deployment in required_deployments {
            if !self
                .deployments
                .contains_key(&(deployment.project.id, deployment.deployment.url_id.clone()))
            {
                let project = deployment.project.id;
                let url_id = deployment.deployment.url_id.clone();
                let deployment =
                    Deployment::new(deployment, build_queue.clone(), github.clone(), db.clone());
                self.deployments.insert((project, url_id), deployment);
            }
        }
        let existing_ids = self.deployments.keys().cloned().collect::<Vec<_>>();
        for id in existing_ids {
            if !required_ids.contains(&id) {
                self.deployments.remove(&id);
            }
        }
        dbg!();

        // sync map.prod
        self.prod = stream::iter(projects)
            .map(|(id, _)| {
                let project_deployments = self
                    .deployments
                    .iter()
                    .map(|(_, deployment)| deployment)
                    .filter(|deployment| deployment.project == id && deployment.branch == None)
                    .map(|deployment| {
                        (
                            deployment.app_container.clone(),
                            deployment.created,
                            deployment.url_id.clone(),
                        )
                    })
                    .collect::<Vec<_>>();
                (id, project_deployments)
            })
            .filter_map(|(id, project_deployments)| async move {
                // TODO: bear in mind prod id saved in the db
                let prod_id =
                        stream::iter(project_deployments)
                            .filter(|(app_container, _, _)| {
                                let app_container = app_container.clone();
                                async move {
                                    *app_container.result.read().await == Some(BuildResult::Built)
                                }
                            })
                            .fold((0, None), |current, (_, created, url_id)| async move {
                                if created > current.0 {
                                    (created, Some(url_id.clone()))
                                } else {
                                    current
                                }
                            })
                            .await
                            .1?;
                Some((id, prod_id))
            })
            .collect()
            .await;
        // TODO: lots of clones going on above, the code below seems so close to work...
        // self.prod = stream::iter(projects)
        //     .filter_map(|(id, _)| async {
        //         // TODO: bear in mind prod id saved in the db
        //         let project_deployments = self
        //             .deployments
        //             .iter()
        //             .map(|(_, deployment)| deployment)
        //             .filter(|deployment| deployment.project == id);
        //         let prod_id = stream::iter(project_deployments)
        //             .filter(|deployment| async {
        //                 deployment.app_container.status.read().await.is_built()
        //             })
        //             .fold((0, None), |current, deployment| async {
        //                 if deployment.timestamp > current.0 {
        //                     (deployment.timestamp, Some(deployment.url_id.clone()))
        //                 } else {
        //                     current
        //                 }
        //             })
        //             .await
        //             .1?;
        //         Some((id, prod_id))
        //     })
        //     .collect()
        //     .await;

        // force build and start for prod containers
        for deployment in self.iter_prod_deployments() {
            let status = deployment.app_container.status.read().await.clone();
            match status {
                ContainerStatus::StandBy { .. } => {
                    deployment.app_container.start().await;
                }
                // the logic to put containers into the queue is a bit duplicated.
                // Maybe everything related to putting containers into the Queue should be here,
                // but that means I need an additional status
                ContainerStatus::Built { .. } => {
                    deployment.app_container.enqueue().await;
                }
                _ => {}
            }
        }
        dbg!();

        // downgrade unused containers
        for container in self.get_all_non_prod_containers().await {
            container.downgrade_if_unused().await;
        }
        dbg!();
    }

    fn iter_prod_deployments(&self) -> impl Iterator<Item = &Deployment> {
        self.names
            .keys()
            .filter_map(|project| self.get_prod(project))
    }

    async fn get_all_non_prod_containers(&self) -> Vec<Arc<Container>> {
        let prod_deployment_ids = self
            .iter_prod_deployments()
            .map(|deployment| deployment.id)
            .collect::<Vec<_>>();
        let all_containers_from_non_prod_deployments = self
            .deployments
            .values()
            .filter(|deployment| !prod_deployment_ids.contains(&deployment.id))
            .flat_map(|deployment| deployment.iter_arc_containers());

        let prisma_containers_from_prod_deployments = self
            .iter_prod_deployments()
            .map(|deployment| deployment.prisma_container.clone());

        all_containers_from_non_prod_deployments
            .chain(prisma_containers_from_prod_deployments)
            .collect()
    }
}
