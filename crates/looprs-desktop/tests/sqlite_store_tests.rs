use looprs_desktop::services::sqlite_store::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_load_chat_messages_empty_database() {
    let _tmp = setup_temp_observability_dir();

    let messages = load_chat_messages(100).await;

    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_append_and_load_chat_messages() {
    let _tmp = setup_temp_observability_dir();

    append_chat_message("You", "Hello").await;
    append_chat_message("Assistant", "Hi there!").await;

    let messages = load_chat_messages(100).await;

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "You");
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[1].role, "Assistant");
    assert_eq!(messages[1].content, "Hi there!");
}

#[tokio::test]
async fn test_load_chat_messages_respects_limit() {
    let _tmp = setup_temp_observability_dir();

    for i in 0..10 {
        append_chat_message("You", &format!("Message {}", i)).await;
    }

    let messages = load_chat_messages(5).await;

    assert_eq!(messages.len(), 5);
    // Should return most recent 5 (5-9)
    assert!(messages[4].content.contains("9"));
}

#[tokio::test]
async fn test_clear_chat_messages() {
    let _tmp = setup_temp_observability_dir();

    append_chat_message("You", "Test").await;
    clear_chat_messages().await;

    let messages = load_chat_messages(100).await;
    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_append_observability_event() {
    let _tmp = setup_temp_observability_dir();

    append_observability_event("chat.send", "test payload").await;

    // Verify event was stored by ensuring no errors occurred
    // TODO: Add query function to retrieve events for testing
}

#[tokio::test]
async fn test_concurrent_writes() {
    let _tmp = setup_temp_observability_dir();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            tokio::spawn(async move {
                append_chat_message("User", &format!("Concurrent message {}", i)).await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let messages = load_chat_messages(100).await;
    assert_eq!(messages.len(), 10);
}

#[tokio::test]
async fn test_database_connection_failure_handling() {
    // Set invalid observability dir to force connection failure
    std::env::set_var("LOOPRS_OBSERVABILITY_DIR", "/invalid/path/that/cannot/exist");

    // Should not panic, should return empty vec
    let messages = load_chat_messages(100).await;
    assert_eq!(messages.len(), 0);

    std::env::remove_var("LOOPRS_OBSERVABILITY_DIR");
}

#[tokio::test]
async fn test_messages_ordered_chronologically() {
    let _tmp = setup_temp_observability_dir();

    append_chat_message("User", "First").await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    append_chat_message("User", "Second").await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    append_chat_message("User", "Third").await;

    let messages = load_chat_messages(100).await;

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].content, "First");
    assert_eq!(messages[1].content, "Second");
    assert_eq!(messages[2].content, "Third");
}

#[tokio::test]
async fn test_message_with_special_characters() {
    let _tmp = setup_temp_observability_dir();

    let special_content = "Test with 'quotes' and \"double quotes\" and \n newlines";
    append_chat_message("User", special_content).await;

    let messages = load_chat_messages(100).await;

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, special_content);
}

#[tokio::test]
async fn test_empty_message_content() {
    let _tmp = setup_temp_observability_dir();

    append_chat_message("User", "").await;

    let messages = load_chat_messages(100).await;

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "");
}

// Test helper
fn setup_temp_observability_dir() -> TempDir {
    let tmp = TempDir::new().unwrap();
    std::env::set_var("LOOPRS_OBSERVABILITY_DIR", tmp.path());
    tmp
}
