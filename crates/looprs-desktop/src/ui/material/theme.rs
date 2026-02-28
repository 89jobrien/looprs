use looprs_desktop_baml_client::types::{
    AnomalySeverity, HealthStatus, Sentiment, UiContext, WorkflowStage,
};

/// Material Design 3 theme with context-aware color selection
#[derive(Clone, Debug)]
pub struct MaterialTheme {
    // Base Material Design 3 color tokens
    pub primary: (u8, u8, u8),
    pub on_primary: (u8, u8, u8),
    pub primary_container: (u8, u8, u8),
    pub on_primary_container: (u8, u8, u8),

    pub secondary: (u8, u8, u8),
    pub on_secondary: (u8, u8, u8),

    pub error: (u8, u8, u8),
    pub on_error: (u8, u8, u8),
    pub error_container: (u8, u8, u8),
    pub on_error_container: (u8, u8, u8),

    pub background: (u8, u8, u8),
    pub on_background: (u8, u8, u8),

    pub surface: (u8, u8, u8),
    pub on_surface: (u8, u8, u8),
    pub surface_variant: (u8, u8, u8),
    pub on_surface_variant: (u8, u8, u8),

    // Typography scale (font sizes in logical pixels)
    pub display_large: f32,
    pub display_medium: f32,
    pub display_small: f32,
    pub headline_large: f32,
    pub headline_medium: f32,
    pub headline_small: f32,
    pub title_large: f32,
    pub title_medium: f32,
    pub title_small: f32,
    pub body_large: f32,
    pub body_medium: f32,
    pub body_small: f32,
    pub label_large: f32,
    pub label_medium: f32,
    pub label_small: f32,

    // Spacing system (4dp grid)
    pub spacing_xs: f32,   // 4dp
    pub spacing_sm: f32,   // 8dp
    pub spacing_md: f32,   // 16dp
    pub spacing_lg: f32,   // 24dp
    pub spacing_xl: f32,   // 32dp
    pub spacing_xxl: f32,  // 48dp

    // Elevation shadows
    pub elevation_0: f32,
    pub elevation_1: f32,
    pub elevation_2: f32,
    pub elevation_3: f32,
    pub elevation_4: f32,
    pub elevation_5: f32,

    // Corner radius
    pub radius_none: f32,
    pub radius_xs: f32,    // 4dp
    pub radius_sm: f32,    // 8dp
    pub radius_md: f32,    // 12dp
    pub radius_lg: f32,    // 16dp
    pub radius_xl: f32,    // 28dp
    pub radius_full: f32,  // 999dp
}

impl MaterialTheme {
    /// Create a light theme
    pub fn light() -> Self {
        Self {
            // Material Design 3 color tokens (Blue theme)
            primary: (33, 150, 243),           // Blue 500
            on_primary: (255, 255, 255),
            primary_container: (187, 222, 251), // Blue 100
            on_primary_container: (1, 87, 155), // Blue 900

            secondary: (103, 58, 183),         // Deep Purple 500
            on_secondary: (255, 255, 255),

            error: (244, 67, 54),              // Red 500
            on_error: (255, 255, 255),
            error_container: (255, 205, 210),  // Red 100
            on_error_container: (183, 28, 28), // Red 900

            background: (255, 255, 255),
            on_background: (0, 0, 0),

            surface: (255, 255, 255),
            on_surface: (0, 0, 0),
            surface_variant: (245, 245, 245),
            on_surface_variant: (66, 66, 66),

            // Typography scale
            display_large: 57.0,
            display_medium: 45.0,
            display_small: 36.0,
            headline_large: 32.0,
            headline_medium: 28.0,
            headline_small: 24.0,
            title_large: 22.0,
            title_medium: 16.0,
            title_small: 14.0,
            body_large: 16.0,
            body_medium: 14.0,
            body_small: 12.0,
            label_large: 14.0,
            label_medium: 12.0,
            label_small: 11.0,

            // Spacing (4dp grid)
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 16.0,
            spacing_lg: 24.0,
            spacing_xl: 32.0,
            spacing_xxl: 48.0,

            // Elevation
            elevation_0: 0.0,
            elevation_1: 1.0,
            elevation_2: 3.0,
            elevation_3: 6.0,
            elevation_4: 8.0,
            elevation_5: 12.0,

            // Corner radius
            radius_none: 0.0,
            radius_xs: 4.0,
            radius_sm: 8.0,
            radius_md: 12.0,
            radius_lg: 16.0,
            radius_xl: 28.0,
            radius_full: 999.0,
        }
    }

    /// Create a dark theme
    pub fn dark() -> Self {
        Self {
            primary: (144, 202, 249),          // Blue 200
            on_primary: (1, 87, 155),
            primary_container: (1, 87, 155),   // Blue 900
            on_primary_container: (187, 222, 251),

            secondary: (179, 136, 255),        // Deep Purple 200
            on_secondary: (49, 27, 146),

            error: (239, 83, 80),              // Red 300
            on_error: (183, 28, 28),
            error_container: (183, 28, 28),
            on_error_container: (255, 205, 210),

            background: (18, 18, 18),
            on_background: (255, 255, 255),

            surface: (18, 18, 18),
            on_surface: (255, 255, 255),
            surface_variant: (33, 33, 33),
            on_surface_variant: (200, 200, 200),

            ..Self::light() // Inherit typography, spacing, elevation, radius
        }
    }

    /// Get context-aware colors based on unified context
    pub fn colors_for_context(&self, context: &UiContext) -> ContextColors {
        let urgency = self.calculate_urgency(context);

        match urgency {
            5 => ContextColors {
                primary: (244, 67, 54),       // Red 500 - Critical
                container: (255, 205, 210),   // Red 100
                on_container: (183, 28, 28),  // Red 900
                border: Some((244, 67, 54)),
                corner_radius: 2.0,
                spacing: 4.0,
                border_width: 2.0,
            },
            4 => ContextColors {
                primary: (255, 152, 0),       // Orange 500 - High
                container: (255, 224, 178),   // Orange 100
                on_container: (230, 81, 0),   // Orange 900
                border: Some((255, 152, 0)),
                corner_radius: 4.0,
                spacing: 8.0,
                border_width: 1.5,
            },
            3 => ContextColors {
                primary: (255, 235, 59),      // Yellow 500 - Medium
                container: (255, 249, 196),   // Yellow 100
                on_container: (245, 127, 23), // Yellow 900
                border: Some((255, 235, 59)),
                corner_radius: 8.0,
                spacing: 12.0,
                border_width: 1.0,
            },
            2 => ContextColors {
                primary: (33, 150, 243),      // Blue 500 - Low
                container: (187, 222, 251),   // Blue 100
                on_container: (1, 87, 155),   // Blue 900
                border: None,
                corner_radius: 12.0,
                spacing: 16.0,
                border_width: 1.0,
            },
            _ => ContextColors {
                primary: (76, 175, 80),       // Green 500 - Healthy
                container: (200, 230, 201),   // Green 100
                on_container: (27, 94, 32),   // Green 900
                border: None,
                corner_radius: 16.0,
                spacing: 24.0,
                border_width: 0.0,
            },
        }
    }

    /// Calculate urgency level (0-5) from unified context
    pub fn calculate_urgency(&self, context: &UiContext) -> i32 {
        let mut urgency = 0;

        // System health contributes most
        if let Some(ref health) = context.system_health {
            urgency += match health.status {
                HealthStatus::Critical => 3,
                HealthStatus::Degraded => 2,
                HealthStatus::Healthy => 0,
                HealthStatus::Unknown => 1,
            };
        }

        // Critical anomalies add urgency
        let critical_anomalies = context
            .anomalies
            .iter()
            .filter(|a| matches!(a.severity, AnomalySeverity::Critical))
            .count();
        urgency += critical_anomalies.min(2) as i32;

        // Negative sentiment adds some urgency
        if let Some(ref sentiment_ctx) = context.sentiment {
            urgency += match sentiment_ctx.sentiment {
                Sentiment::VeryNegative => 1,
                Sentiment::Negative => 0,
                _ => 0,
            };
        }

        // Workflow failures add urgency
        if let Some(ref workflow) = context.workflow_state {
            if matches!(workflow.stage, WorkflowStage::Failed) {
                urgency += 2;
            }
        }

        urgency.min(5)
    }
}

/// Context-aware colors for components
#[derive(Clone, Debug)]
pub struct ContextColors {
    pub primary: (u8, u8, u8),
    pub container: (u8, u8, u8),
    pub on_container: (u8, u8, u8),
    pub border: Option<(u8, u8, u8)>,
    pub corner_radius: f32,
    pub spacing: f32,
    pub border_width: f32,
}

impl ContextColors {
    /// Convert RGB tuple to CSS color string
    pub fn to_css(&self, color: (u8, u8, u8)) -> String {
        format!("rgb({}, {}, {})", color.0, color.1, color.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_theme_creation() {
        let theme = MaterialTheme::light();
        assert_eq!(theme.primary, (33, 150, 243));
        assert_eq!(theme.spacing_md, 16.0);
    }

    #[test]
    fn test_dark_theme_creation() {
        let theme = MaterialTheme::dark();
        assert_eq!(theme.background, (18, 18, 18));
    }

    #[test]
    fn test_urgency_calculation_healthy() {
        let theme = MaterialTheme::light();
        let mut context = super::super::super::super::services::context_engine::default_ui_context();
        context.system_health = Some(looprs_desktop_baml_client::types::SystemHealth {
            status: HealthStatus::Healthy,
            cpu_usage: 30.0,
            memory_usage: 40.0,
            error_rate: 0.0,
            response_time_p95: 10.0,
            active_alerts: 0,
            recommendations: vec![],
        });

        let urgency = theme.calculate_urgency(&context);
        assert_eq!(urgency, 0);
    }

    #[test]
    fn test_urgency_calculation_critical() {
        let theme = MaterialTheme::light();
        let mut context = super::super::super::super::services::context_engine::default_ui_context();
        context.system_health = Some(looprs_desktop_baml_client::types::SystemHealth {
            status: HealthStatus::Critical,
            cpu_usage: 95.0,
            memory_usage: 90.0,
            error_rate: 15.0,
            response_time_p95: 200.0,
            active_alerts: 5,
            recommendations: vec!["Scale up resources".to_string()],
        });
        context.anomalies = vec![
            looprs_desktop_baml_client::types::DataAnomaly {
                r#type: looprs_desktop_baml_client::types::AnomalyType::Spike,
                severity: AnomalySeverity::Critical,
                metric: "cpu".to_string(),
                current_value: 95.0,
                expected_range: serde_json::json!([20.0, 60.0]),
                description: "CPU spike".to_string(),
                timestamp: "2026-02-28T12:00:00Z".to_string(),
            },
        ];

        let urgency = theme.calculate_urgency(&context);
        assert!(urgency >= 4); // Critical health (3) + critical anomaly (1)
    }

    #[test]
    fn test_colors_for_critical_context() {
        let theme = MaterialTheme::light();
        let mut context = super::super::super::super::services::context_engine::default_ui_context();
        context.system_health = Some(looprs_desktop_baml_client::types::SystemHealth {
            status: HealthStatus::Critical,
            cpu_usage: 95.0,
            memory_usage: 90.0,
            error_rate: 15.0,
            response_time_p95: 200.0,
            active_alerts: 5,
            recommendations: vec![],
        });

        let colors = theme.colors_for_context(&context);
        assert_eq!(colors.primary, (244, 67, 54)); // Red
        assert_eq!(colors.corner_radius, 2.0);     // Sharp corners
        assert_eq!(colors.spacing, 4.0);           // Tight spacing
        assert!(colors.border.is_some());          // Has border
    }
}
