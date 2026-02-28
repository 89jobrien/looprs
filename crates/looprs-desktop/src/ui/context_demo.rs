use crate::services::context_engine::default_ui_context;
use crate::ui::material::theme::MaterialTheme;
use freya::prelude::*;
use looprs_desktop_baml_client::types::{
    AnomalySeverity, AnomalyType, DataAnomaly, HealthStatus, SystemHealth, UiContext,
};

/// Context-aware UI demo screen - entry point
pub fn context_demo_screen() -> Element {
    Element::from(ContextDemoScreen)
}

#[derive(Clone, Copy, PartialEq)]
pub struct ContextDemoScreen;

impl Component for ContextDemoScreen {
    fn render(&self) -> impl IntoElement {
        let context_state = use_state(|| default_ui_context());
        let theme = MaterialTheme::dark();

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .vertical()
            .padding(Gaps::new_all(24.0))
            .background((18, 18, 18))
            .child(
                // Title
                label()
                    .font_size(28.0)
                    .color((255, 255, 255))
                    .text("Context-Aware UI Demo"),
            )
            .child(
                // Description
                label()
                    .font_size(14.0)
                    .color((200, 200, 200))
                    .text("This demo shows how UI components adapt based on system context."),
            )
            .child(
                // Simulation buttons
                rect()
                    .horizontal()
                    .spacing(8.0)
                    .child(
                        rect()
                            .padding(Gaps::new_all(8.0))
                            .background((76, 175, 80))
                            .corner_radius(8.0)
                            .on_press({
                                let mut context_state = context_state;
                                move |_| {
                                    let mut new_context = default_ui_context();
                                    new_context.system_health = Some(SystemHealth {
                                        status: HealthStatus::Healthy,
                                        cpu_usage: 30.0,
                                        memory_usage: 40.0,
                                        error_rate: 0.0,
                                        response_time_p95: 10.0,
                                        active_alerts: 0,
                                        recommendations: vec![],
                                    });
                                    context_state.set(new_context);
                                }
                            })
                            .child(
                                label()
                                    .color((255, 255, 255))
                                    .font_size(14.0)
                                    .text("Simulate Healthy"),
                            ),
                    )
                    .child(
                        rect()
                            .padding(Gaps::new_all(8.0))
                            .background((255, 152, 0))
                            .corner_radius(8.0)
                            .on_press({
                                let mut context_state = context_state;
                                move |_| {
                                    let mut new_context = default_ui_context();
                                    new_context.system_health = Some(SystemHealth {
                                        status: HealthStatus::Degraded,
                                        cpu_usage: 75.0,
                                        memory_usage: 70.0,
                                        error_rate: 5.0,
                                        response_time_p95: 50.0,
                                        active_alerts: 2,
                                        recommendations: vec!["Monitor resource usage".to_string()],
                                    });
                                    context_state.set(new_context);
                                }
                            })
                            .child(
                                label()
                                    .color((255, 255, 255))
                                    .font_size(14.0)
                                    .text("Simulate Degraded"),
                            ),
                    )
                    .child(
                        rect()
                            .padding(Gaps::new_all(8.0))
                            .background((244, 67, 54))
                            .corner_radius(8.0)
                            .on_press({
                                let mut context_state = context_state;
                                move |_| {
                                    let mut new_context = default_ui_context();
                                    new_context.system_health = Some(SystemHealth {
                                        status: HealthStatus::Critical,
                                        cpu_usage: 95.0,
                                        memory_usage: 90.0,
                                        error_rate: 15.0,
                                        response_time_p95: 200.0,
                                        active_alerts: 5,
                                        recommendations: vec![
                                            "Scale up resources immediately".to_string(),
                                            "Check for memory leaks".to_string(),
                                        ],
                                    });
                                    new_context.anomalies = vec![DataAnomaly {
                                        r#type: AnomalyType::Spike,
                                        severity: AnomalySeverity::Critical,
                                        metric: "cpu_usage".to_string(),
                                        current_value: 95.0,
                                        expected_range: serde_json::json!([20.0, 60.0]),
                                        description: "CPU usage spiked to 95%".to_string(),
                                        timestamp: chrono::Utc::now().to_rfc3339(),
                                    }];
                                    context_state.set(new_context);
                                }
                            })
                            .child(
                                label()
                                    .color((255, 255, 255))
                                    .font_size(14.0)
                                    .text("Simulate Critical"),
                            ),
                    ),
            )
            .child(context_aware_card(context_state.read().clone(), theme))
    }
}

/// Context-aware card that adapts styling based on context
fn context_aware_card(context: UiContext, theme: MaterialTheme) -> impl IntoElement {
    let urgency = theme.calculate_urgency(&context);
    let colors = theme.colors_for_context(&context);

    // Build status text
    let status_text = if let Some(ref health) = context.system_health {
        format!(
            "{:?} - CPU: {:.1}%, Memory: {:.1}%, Errors: {:.1}/min",
            health.status, health.cpu_usage, health.memory_usage, health.error_rate
        )
    } else {
        "No system health data".to_string()
    };

    // Urgency indicator
    let urgency_text = match urgency {
        5 => "🔴 CRITICAL",
        4 => "🟠 HIGH",
        3 => "🟡 MEDIUM",
        2 => "🔵 LOW",
        _ => "🟢 HEALTHY",
    };

    let mut card = rect()
        .width(Size::fill())
        .background(colors.container)
        .corner_radius(colors.corner_radius)
        .padding(Gaps::new_all(colors.spacing))
        .vertical()
        .spacing(8.0);

    // Add border if present
    if let Some(border_color) = colors.border {
        card = card.border(Border::new().fill(border_color).width(colors.border_width));
    }

    // Title with urgency indicator
    card = card.child(
        label()
            .font_size(theme.title_large)
            .color(colors.on_container)
            .text(format!("System Status {}", urgency_text)),
    );

    // Status details
    card = card.child(
        label()
            .font_size(theme.body_medium)
            .color(colors.on_container)
            .text(status_text),
    );

    // Anomalies section
    if !context.anomalies.is_empty() {
        let mut anomalies_section = rect().vertical();
        anomalies_section = anomalies_section.child(
            label()
                .font_size(theme.body_small)
                .color(colors.on_container)
                .text("⚠️ Anomalies Detected:"),
        );

        for anomaly in context.anomalies.iter() {
            anomalies_section = anomalies_section.child(
                label()
                    .font_size(theme.body_small)
                    .color(colors.on_container)
                    .text(format!("  • {}", anomaly.description)),
            );
        }

        card = card.child(anomalies_section);
    }

    // Recommendations section
    if let Some(ref health) = context.system_health {
        if !health.recommendations.is_empty() {
            let mut recs_section = rect().vertical();
            recs_section = recs_section.child(
                label()
                    .font_size(theme.body_small)
                    .color(colors.on_container)
                    .text("💡 Recommendations:"),
            );

            for rec in health.recommendations.iter() {
                recs_section = recs_section.child(
                    label()
                        .font_size(theme.body_small)
                        .color(colors.on_container)
                        .text(format!("  • {}", rec)),
                );
            }

            card = card.child(recs_section);
        }
    }

    card
}
