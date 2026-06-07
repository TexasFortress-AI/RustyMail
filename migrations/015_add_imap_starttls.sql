-- Track IMAP STARTTLS separately from implicit TLS.
ALTER TABLE accounts ADD COLUMN imap_use_starttls BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE provider_templates ADD COLUMN imap_use_starttls BOOLEAN NOT NULL DEFAULT FALSE;
