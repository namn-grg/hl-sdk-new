#[macro_export]
macro_rules! hyperliquid_action {
    // With prefix (default)
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                pub $field:ident: $type:ty
            ),* $(,)?
        }
        => $type_string:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(
                $(#[$field_meta])*
                pub $field: $type,
            )*
        }

        impl $crate::types::eip712::HyperliquidAction for $name {
            const TYPE_STRING: &'static str = $type_string;
            const USE_PREFIX: bool = true;
        }
    };
    
    // Without prefix
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                pub $field:ident: $type:ty
            ),* $(,)?
        }
        => $type_string:literal,
        no_prefix
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(
                $(#[$field_meta])*
                pub $field: $type,
            )*
        }

        impl $crate::types::eip712::HyperliquidAction for $name {
            const TYPE_STRING: &'static str = $type_string;
            const USE_PREFIX: bool = false;
        }
    };
}
