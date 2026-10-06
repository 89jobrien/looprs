//! Shared `macro_rules!` macros for looprs crates.

/// Define a newtype wrapper around `String` with common trait impls.
#[macro_export]
macro_rules! newtype_id {
    ($name:ident) => {
        #[doc = concat!("Typed string identifier for `", stringify!($name), "`.")]
        #[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wrap a string value in this newtype.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Borrow the wrapped value as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

/// Define a domain event enum with auto-generated `name()` method.
#[macro_export]
macro_rules! domain_event {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        #[doc = concat!("Domain event enum `", stringify!($name), "`.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                #[doc = concat!("`", stringify!($variant), "` event kind.")]
                $variant
            ),*
        }

        impl $name {
            /// The variant name exactly as written in the source.
            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),*
                }
            }
        }
    };
}

/// Define a `NamedTool` adapter struct for a fixed binary name.
#[macro_export]
macro_rules! define_tool {
    ($name:ident, $bin:literal) => {
        pub struct $name<'a> {
            plugins: &'a Plugins,
        }

        impl<'a> $name<'a> {
            pub fn new(plugins: &'a Plugins) -> Self {
                Self { plugins }
            }

            pub fn system() -> $name<'static> {
                $name {
                    plugins: super::system(),
                }
            }
        }

        impl NamedTool for $name<'_> {
            const NAME: &'static str = $bin;

            fn plugins(&self) -> &Plugins {
                self.plugins
            }
        }
    };
}
