-- Add migration script here
ALTER TABLE deployments
    ADD COLUMN deleted INTEGER; -- not null = deleted
