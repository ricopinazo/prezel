CREATE TABLE projects_new (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    repo_id INTEGER NOT NULL,
    created INTEGER NOT NULL,
    root TEXT NOT NULL,
    prod_id INTEGER -- deployment id used for prod
);

INSERT INTO projects_new (id, name, repo_id, created, root, prod_id)
SELECT id, name, CAST(repo_id AS INTEGER), created, root, prod_id
FROM projects;

-- Step 3: Drop the old table
DROP TABLE projects;

-- Step 4: Rename the new table to the old table's name
ALTER TABLE projects_new
RENAME TO projects;
