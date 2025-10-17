-- Provider Templates Table
-- Stores email provider configuration templates (Gmail, Outlook, etc.)

CREATE TABLE IF NOT EXISTS provider_templates (
    provider_type TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    domain_patterns TEXT NOT NULL,  -- JSON array of domain patterns
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL DEFAULT 993,
    imap_use_tls BOOLEAN NOT NULL DEFAULT TRUE,
    smtp_host TEXT NOT NULL,
    smtp_port INTEGER NOT NULL DEFAULT 587,
    smtp_use_tls BOOLEAN NOT NULL DEFAULT TRUE,
    smtp_use_starttls BOOLEAN NOT NULL DEFAULT TRUE,
    supports_oauth BOOLEAN NOT NULL DEFAULT FALSE,
    oauth_provider TEXT,
    auto_discover BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert common provider templates
INSERT OR IGNORE INTO provider_templates (provider_type, display_name, domain_patterns, imap_host, imap_port, smtp_host, smtp_port, smtp_use_starttls, supports_oauth, oauth_provider)
VALUES
    ('gmail', 'Gmail', '["gmail.com", "googlemail.com"]', 'imap.gmail.com', 993, 'smtp.gmail.com', 587, 1, 1, 'google'),
    ('outlook', 'Outlook/Hotmail', '["outlook.com", "hotmail.com", "live.com"]', 'outlook.office365.com', 993, 'smtp.office365.com', 587, 1, 1, 'microsoft'),
    ('yahoo', 'Yahoo Mail', '["yahoo.com", "ymail.com"]', 'imap.mail.yahoo.com', 993, 'smtp.mail.yahoo.com', 587, 1, 0, NULL),
    ('icloud', 'iCloud Mail', '["icloud.com", "me.com", "mac.com"]', 'imap.mail.me.com', 993, 'smtp.mail.me.com', 587, 1, 0, NULL),
    ('godaddy', 'GoDaddy', '["secureserver.net"]', 'imap.secureserver.net', 993, 'smtpout.secureserver.net', 587, 1, 0, NULL);
