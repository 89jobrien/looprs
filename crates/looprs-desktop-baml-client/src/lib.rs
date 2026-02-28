#[allow(
    clippy::derivable_impls,
    clippy::empty_line_after_doc_comments,
    clippy::map_clone,
    clippy::new_without_default,
    clippy::unwrap_or_default
)]
pub mod baml_client;

pub use baml_client::B;
pub use baml_client::types;

pub fn init() {
    baml_client::init();
}
