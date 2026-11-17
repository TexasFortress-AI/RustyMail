-- Mailing-list posts (bucket Lists) and code-repository notifications
-- (bucket Repos) go to a folder per list or repo. This is that list or repo
-- name, taken from the message's List-Id and reduced to safe characters.
ALTER TABLE triage_proposals ADD COLUMN list_name TEXT;
