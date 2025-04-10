use serde::{Serialize, Deserialize};
// Comment out unused transport import
// use crate::transport::*;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TestMessage {
    id: u32,
    content: String,
}

// --- Mock Transport ---
// ... rest of file ... 