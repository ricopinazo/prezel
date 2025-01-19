ALTER TABLE projects DROP COLUMN env;
ALTER TABLE deployments DROP COLUMN env;

CREATE TABLE IF NOT EXISTS env (
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    edited INTEGER NOT NULL,
    project INTEGER NOT NULL,
    FOREIGN KEY (project) REFERENCES projects(id) ON DELETE CASCADE
    PRIMARY KEY (project, name)
);

CREATE TABLE IF NOT EXISTS deployment_env (
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    deployment INTEGER NOT NULL,
    FOREIGN KEY (deployment) REFERENCES deployments(id) ON DELETE CASCADE
    PRIMARY KEY (deployment, name)
);
