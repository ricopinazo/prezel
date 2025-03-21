use anyhow::{anyhow, ensure};

/// The prefix of the hostname that refers to a resource of a particular app hosted in the server
#[derive(Debug)]
pub(crate) enum Label {
    Prod { project: String },
    ProdDb { project: String },
    Deployment { project: String, deployment: String },
    BranchDb { project: String, deployment: String },
}

impl Label {
    pub(crate) fn format_hostname(&self, box_domain: &str) -> String {
        match self {
            Label::Prod { project } => format!("{project}.{box_domain}"),
            Label::ProdDb { project } => format!("{project}--libsql.{box_domain}"),
            Label::Deployment {
                project,
                deployment,
            } => format!("{project}--{deployment}.{box_domain}"),
            Label::BranchDb {
                project,
                deployment,
            } => format!("{project}--{deployment}-libsql.{box_domain}"),
        }
    }

    pub(crate) fn strip_from_domain(hostname: &str, box_domain: &str) -> anyhow::Result<Self> {
        let label_with_dot = hostname.strip_suffix(box_domain).ok_or(anyhow::Error::msg(
            "invalid hostname not ending with the box domain",
        ))?;
        // FIXME: double check len > 0 ?
        let label = &label_with_dot[..label_with_dot.len() - 1];
        ensure!(
            label.find(".").is_none(),
            "invalid label, more dots than expected"
        );
        let parsed = parse_label(label).ok_or(anyhow!("invalid label"))?;
        Ok(parsed)
    }
}

fn parse_label(label: &str) -> Option<Label> {
    match label.split("--").collect::<Vec<_>>().as_slice() {
        [project] => Some(Label::Prod {
            project: project.to_string(),
        }),
        [project, sublabel] => match sublabel.split("-").collect::<Vec<_>>().as_slice() {
            ["libsql"] => Some(Label::ProdDb {
                project: project.to_string(),
            }),
            [deployment] => Some(Label::Deployment {
                project: project.to_string(),
                deployment: deployment.to_string(),
            }),
            [deployment, "libsql"] => Some(Label::BranchDb {
                project: project.to_string(),
                deployment: deployment.to_string(),
            }),
            _ => None,
        },
        _ => None,
    }
}
