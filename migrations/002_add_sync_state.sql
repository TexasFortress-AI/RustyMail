-- Add sync_state table for tracking email synchronization state
CREATE TABLE sync_state (
    folder_id INTEGER PRIMARY KEY,
    last_uid_synced INTEGER,
    last_full_sync TIMESTAMP,
    last_incremental_sync TIMESTAMP,
    sync_status TEXT NOT NULL DEFAULT 'Idle',
    error_message TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);

CREATE INDEX idx_sync_state_folder ON sync_state(folder_id);
CREATE INDEX idx_sync_state_status ON sync_state(sync_status);

CREATE TRIGGER update_sync_state_timestamp
    AFTER UPDATE ON sync_state
    BEGIN
        UPDATE sync_state SET updated_at = CURRENT_TIMESTAMP WHERE folder_id = NEW.folder_id;
    END;
