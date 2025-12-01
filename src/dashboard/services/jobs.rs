// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use serde::Serialize;
use serde_json::Value;
use std::time::Instant;

/// Status of a background job
#[derive(Serialize, Clone)]
#[serde(tag = "status", content = "data")]
pub enum JobStatus {
    Running,
    Completed(Value),
    Failed(String),
}

/// A background job record
#[derive(Clone)]
pub struct JobRecord {
    pub job_id: String,
    pub status: JobStatus,
    pub started_at: Instant,
    pub instruction: Option<String>,
}

// Custom Serialize implementation for JobRecord to control output
impl Serialize for JobRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("JobRecord", 3)?;
        state.serialize_field("job_id", &self.job_id)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("instruction", &self.instruction)?;
        state.end()
    }
}
