// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[macro_export]
macro_rules! mcp_tool {
    ($struct_name:ident<$generic_param:ident>, $tool_name:expr, $description:expr, $input_schema:expr, $output_schema:expr) => {
        #[derive(Debug)]
        pub struct $struct_name<$generic_param> {
            imap_client: ::std::sync::Arc<$generic_param>,
        }

        impl<$generic_param> $struct_name<$generic_param> {
            pub fn new(imap_client: ::std::sync::Arc<$generic_param>) -> Self {
                Self { imap_client }
            }
        }
    };

    ($struct_name:ident, $tool_name:expr, $description:expr, $input_schema:expr, $output_schema:expr) => {
        #[derive(Debug)]
        pub struct $struct_name {
            imap_client: ::std::sync::Arc<$crate::imap::client::ImapClient>,
        }

        impl $struct_name {
            pub fn new(imap_client: ::std::sync::Arc<$crate::imap::client::ImapClient>) -> Self {
                Self { imap_client }
            }
        }
    };
} 