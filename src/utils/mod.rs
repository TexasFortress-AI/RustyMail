// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub mod folder_arg;
pub mod mime_decoder;

pub use folder_arg::{check_folder_query_keys, resolve_folder_arg, FolderArgError};
pub use mime_decoder::decode_mime_header;