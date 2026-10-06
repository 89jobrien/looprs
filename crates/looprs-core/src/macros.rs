/// Forwarding macro for typed string IDs.
#[macro_export]
macro_rules! newtype_id {
    ($($tt:tt)*) => {
        looprs_macros::newtype_id!($($tt)*);
    };
}

/// Forwarding macro for domain event enums.
#[macro_export]
macro_rules! domain_event {
    ($($tt:tt)*) => {
        looprs_macros::domain_event!($($tt)*);
    };
}
