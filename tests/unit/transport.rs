// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

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