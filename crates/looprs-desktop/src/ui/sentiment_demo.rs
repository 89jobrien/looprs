use freya::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use crate::services::sentiment_ui::{
    start_sentiment_ui, SentimentUiCommand, SentimentUiHandle, SentimentUiUpdate,
};

#[component]
pub fn SentimentDemo() -> Element {
    let handle = use_hook(|| start_sentiment_ui(Duration::from_secs(5)));
    let mut updates = use_signal(|| Arc::new(SentimentUiUpdate::default()));

    // Subscribe to updates
    use_effect(move || {
        let handle_clone = handle.clone();
        spawn(async move {
            loop {
                if handle_clone.updates.changed().await.is_ok() {
                    let update = handle_clone.updates.borrow().clone();
                    updates.set(update);
                }
            }
        });
    });

    let test_messages = vec![
        ("🎉 Deployment successful! All tests passed.", "Deployment update"),
        ("⚠️ Critical error in production database", "System alert"),
        ("New pull request ready for review", "GitHub notification"),
        ("🔥 Server experiencing high CPU usage", "Performance warning"),
        ("Build completed in 2.3 seconds", "Build system"),
    ];

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .background((250, 250, 250))
        .padding(Gaps::new_all(20.0))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(60.0))
                .background((60, 90, 132))
                .corner_radius(8.0)
                .padding(Gaps::new_all(16.0))
                .child(
                    label()
                        .text("Sentiment-Aware UI Demo")
                        .color((255, 255, 255))
                        .font_size(24.0)
                )
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(20.0))
        )
        .child(
            // Test message buttons
            rect()
                .width(Size::fill())
                .height(Size::px(200.0))
                .background((255, 255, 255))
                .corner_radius(8.0)
                .padding(Gaps::new_all(16.0))
                .child(
                    label()
                        .text("Test Messages (click to analyze)")
                        .font_size(16.0)
                        .color((60, 60, 60))
                )
                .children(
                    test_messages.iter().map(|(msg, ctx)| {
                        let msg_clone = msg.to_string();
                        let ctx_clone = ctx.to_string();
                        let handle_clone = handle.clone();

                        rect()
                            .width(Size::fill())
                            .height(Size::px(32.0))
                            .background((240, 240, 245))
                            .corner_radius(4.0)
                            .padding(Gaps::new_all(8.0))
                            .onclick(move |_| {
                                handle_clone.send(SentimentUiCommand::AnalyzeText {
                                    text: msg_clone.clone(),
                                    context: ctx_clone.clone(),
                                });
                            })
                            .child(
                                label()
                                    .text(msg)
                                    .font_size(14.0)
                                    .color((40, 40, 40))
                            )
                    })
                )
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(20.0))
        )
        .child(
            // Current sentiment display
            rect()
                .width(Size::fill())
                .height(Size::px(300.0))
                .background((255, 255, 255))
                .corner_radius(8.0)
                .padding(Gaps::new_all(16.0))
                .child(
                    label()
                        .text("Current Sentiment Analysis")
                        .font_size(16.0)
                        .color((60, 60, 60))
                )
                .child(
                    render_sentiment_display(updates.read().clone())
                )
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(20.0))
        )
        .child(
            // Sentiment history
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .background((255, 255, 255))
                .corner_radius(8.0)
                .padding(Gaps::new_all(16.0))
                .child(
                    label()
                        .text("Sentiment History")
                        .font_size(16.0)
                        .color((60, 60, 60))
                )
                .children(
                    updates.read().sentiment_history.iter().rev().map(|entry| {
                        render_history_entry(entry)
                    })
                )
        )
}

fn render_sentiment_display(update: Arc<SentimentUiUpdate>) -> Element {
    let (bg_color, text_color) = if let Some(ref sentiment) = update.current_sentiment {
        match sentiment.as_str() {
            "very_positive" | "positive" => ((230, 250, 230), (0, 120, 0)),
            "negative" | "very_negative" => ((255, 235, 235), (180, 0, 0)),
            _ => ((245, 245, 245), (60, 60, 60)),
        }
    } else {
        ((245, 245, 245), (100, 100, 100))
    };

    rect()
        .width(Size::fill())
        .height(Size::px(150.0))
        .background(bg_color)
        .corner_radius(8.0)
        .padding(Gaps::new_all(16.0))
        .child(
            label()
                .text(format!(
                    "Sentiment: {}",
                    update.current_sentiment.as_deref().unwrap_or("None")
                ))
                .font_size(18.0)
                .color(text_color)
        )
        .child(
            label()
                .text(format!(
                    "Severity: {}",
                    update.current_severity.as_deref().unwrap_or("None")
                ))
                .font_size(16.0)
                .color(text_color)
        )
        .child(
            label()
                .text(format!(
                    "Mood: {}",
                    update.current_mood.as_deref().unwrap_or("None")
                ))
                .font_size(16.0)
                .color(text_color)
        )
        .child(
            label()
                .text(format!("Confidence: {:.2}%", update.sentiment_confidence * 100.0))
                .font_size(14.0)
                .color(text_color)
        )
}

fn render_history_entry(entry: &crate::services::sentiment_ui::SentimentHistoryEntry) -> Element {
    let (indicator_color, _) = match entry.sentiment.as_str() {
        "very_positive" | "positive" => ((50, 200, 100), "🟢"),
        "negative" | "very_negative" => ((240, 80, 80), "🔴"),
        _ => ((200, 200, 200), "⚪"),
    };

    rect()
        .width(Size::fill())
        .height(Size::px(60.0))
        .background((248, 248, 250))
        .corner_radius(4.0)
        .padding(Gaps::new_all(12.0))
        .child(
            rect()
                .width(Size::px(4.0))
                .height(Size::fill())
                .background(indicator_color)
                .corner_radius(2.0)
        )
        .child(
            rect()
                .width(Size::px(8.0))
        )
        .child(
            rect()
                .width(Size::fill())
                .child(
                    label()
                        .text(&entry.text)
                        .font_size(14.0)
                        .color((40, 40, 40))
                )
                .child(
                    label()
                        .text(format!("{} • {} • {:.0}% confidence",
                            entry.sentiment,
                            entry.severity,
                            entry.confidence * 100.0
                        ))
                        .font_size(12.0)
                        .color((120, 120, 120))
                )
        )
}
