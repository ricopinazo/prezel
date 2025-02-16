CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    repo_id INTEGER NOT NULL,
    created INTEGER NOT NULL,
    root TEXT NOT NULL,
    prod_id INTENGER -- deployment id used for prod
);

CREATE TABLE IF NOT EXISTS deployments (
    id TEXT PRIMARY KEY NOT NULL,
    url_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL, -- this is the commit timestamp, used for sorting
    created INTEGER NOT NULL,
    sha TEXT NOT NULL,
    branch TEXT NOT NULL,
    default_branch INTEGER NOT NULL, -- 0 false 1 true
    build_started INTEGER,
    build_finished INTEGER,
    result TEXT,
    project TEXT NOT NULL,
    FOREIGN KEY (project) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE(project, url_id)
);

CREATE TABLE IF NOT EXISTS build (
    id INTEGER PRIMARY KEY NOT NULL,
    timestamp INTEGER NOT NULL,
    content TEXT NOT NULL,
    error INTEGER NOT NULL,
    deployment TEXT NOT NULL,
    FOREIGN KEY (deployment) REFERENCES deployments(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS domains (
    domain TEXT PRIMARY KEY NOT NULL,
    project TEXT NOT NULL,
    FOREIGN KEY (project) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS env (
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    edited INTEGER NOT NULL,
    project TEXT NOT NULL,
    FOREIGN KEY (project) REFERENCES projects(id) ON DELETE CASCADE
    PRIMARY KEY (project, name)
);

CREATE TABLE IF NOT EXISTS deployment_env (
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    deployment TEXT NOT NULL,
    FOREIGN KEY (deployment) REFERENCES deployments(id) ON DELETE CASCADE
    PRIMARY KEY (deployment, name)
);
