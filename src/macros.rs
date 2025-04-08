#[macro_export]
macro_rules! mcp_tool {
    ($struct_name:ident<$generic_param:ident>, $tool_name:expr, $description:expr, $input_schema:expr, $output_schema:expr, $($body:tt)*) => {
        #[derive(Debug)]
        pub struct $struct_name<$generic_param> {
            imap_client: ::std::sync::Arc<$generic_param>,
        }

        impl<$generic_param> $struct_name<$generic_param> {
            pub fn new(imap_client: ::std::sync::Arc<$generic_param>) -> Self {
                Self { imap_client }
            }
            
            $($body)* 
        }

        #[async_trait::async_trait]
        impl<$generic_param: $crate::imap::client::ImapClientTrait + Send + Sync + 'static> $crate::mcp_port::McpTool for $struct_name<$generic_param> {
            fn name(&self) -> &'static str {
                $tool_name
            }
            fn description(&self) -> &'static str {
                $description
            }
            fn input_schema(&self) -> &'static str {
                $input_schema
            }
            fn output_schema(&self) -> &'static str {
                $output_schema
            }
            
            async fn execute(&self, params: ::serde_json::Value) -> Result<::serde_json::Value, $crate::mcp_port::McpPortError> {
                Self::execute(self, params).await
            }
        }
    };

    ($struct_name:ident, $tool_name:expr, $description:expr, $input_schema:expr, $output_schema:expr, $($body:tt)*) => {
        #[derive(Debug)]
        pub struct $struct_name {
            imap_client: ::std::sync::Arc<$crate::imap::client::ImapClient>,
        }

        impl $struct_name {
            pub fn new(imap_client: ::std::sync::Arc<$crate::imap::client::ImapClient>) -> Self {
                Self { imap_client }
            }
            
            $($body)*
        }

        #[async_trait::async_trait]
        impl $crate::mcp_port::McpTool for $struct_name {
            fn name(&self) -> &'static str {
                $tool_name
            }
            fn description(&self) -> &'static str {
                $description
            }
            fn input_schema(&self) -> &'static str {
                $input_schema
            }
            fn output_schema(&self) -> &'static str {
                $output_schema
            }
            
            async fn execute(&self, params: ::serde_json::Value) -> Result<::serde_json::Value, $crate::mcp_port::McpPortError> {
                Self::execute(self, params).await
            }
        }
    };
} 