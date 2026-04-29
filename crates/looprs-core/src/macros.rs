/// Define a newtype wrapper around `String` with common trait impls.
///
/// Generates a struct with:
/// - `new(impl Into<String>) -> Self`
/// - `as_str() -> &str`
/// - `Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`
/// - `#[serde(transparent)]`
/// - `Display` (delegates to inner string)
///
/// # Example
///
/// ```
/// use looprs_core::newtype_id;
///
/// newtype_id!(UserId);
///
/// let id = UserId::new("u-42");
/// assert_eq!(id.as_str(), "u-42");
/// assert_eq!(format!("{id}"), "u-42");
/// ```
#[macro_export]
macro_rules! newtype_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

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
///
/// Generates an enum with:
/// - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`
/// - `fn name(&self) -> &'static str` returning the variant name as a string
///
/// # Example
///
/// ```
/// use looprs_core::domain_event;
///
/// domain_event!(MyEvent {
///     Started,
///     Stopped,
/// });
///
/// assert_eq!(MyEvent::Started.name(), "Started");
/// assert_eq!(MyEvent::Stopped.name(), "Stopped");
/// ```
#[macro_export]
macro_rules! domain_event {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),*
                }
            }
        }
    };
}
