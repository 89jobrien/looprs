// Sentiment-aware UI generation

enum Sentiment {
  VeryPositive
  Positive
  Neutral
  Negative
  VeryNegative
}

enum Severity {
  Critical
  High
  Medium
  Low
  Info
}

enum Mood {
  Calm
  Urgent
  Celebratory
  Cautious
  Empathetic
  Professional
}

class SentimentContext {
  sentiment Sentiment
  severity Severity
  mood Mood
  confidence float // 0.0 - 1.0
  keywords string[]
  tone_adjustments map<string, string>
}

// Extend existing UiNode from generative_ui.baml with sentiment fields
class SentimentUiNode {
  tag "rect" | "label" | "paragraph" | "image" | "svg"
  text string?
  attrs map<string, string>
  children SentimentUiNode[]

  // Sentiment-driven properties
  sentiment_context SentimentContext?
  dynamic_text_template string?  // Template for runtime text generation
  adaptive_colors AdaptiveColors?
}

class AdaptiveColors {
  background_positive string  // rgb(r, g, b) for positive sentiment
  background_neutral string
  background_negative string

  text_positive string
  text_neutral string
  text_negative string

  accent_color string?
  severity_indicator string?  // Color that changes with severity
}

class MessageAnalysis {
  text string
  sentiment Sentiment
  severity Severity
  mood Mood
  confidence float
  key_phrases string[]
  suggested_response_tone string
}

// Analyze sentiment of user input/messages
function AnalyzeSentiment(text: string, context: string) -> MessageAnalysis {
  client "openai/gpt-4o-mini"

  prompt #"
    Analyze the sentiment, severity, and mood of this text.

    Text: {{ text }}

    Context: {{ context }}

    Provide:
    - sentiment: very_positive/positive/neutral/negative/very_negative
    - severity: critical/high/medium/low/info
    - mood: calm/urgent/celebratory/cautious/empathetic/professional
    - confidence: 0.0-1.0 score
    - key_phrases: important phrases that indicate sentiment
    - suggested_response_tone: how the UI should respond

    {{ ctx.output_format }}
  "#
}

// Generate sentiment-aware UI that adapts to emotional context
function GenerateSentimentAwareUi(
  goal: string,
  state_json: string,
  sentiment_analysis: string
) -> SentimentUiNode {
  client "openai/gpt-4o-mini"

  prompt #"
    Generate a Freya UI component that adapts to emotional context.

    Goal: {{ goal }}
    State: {{ state_json }}
    Sentiment Analysis: {{ sentiment_analysis }}

    Rules:
    1. SentimentUiNode.tag must be: rect, label, paragraph, image, or svg
    2. Use sentiment_context to store emotional metadata with enum values:
       - Sentiment: VeryPositive, Positive, Neutral, Negative, VeryNegative
       - Severity: Critical, High, Medium, Low, Info
       - Mood: Calm, Urgent, Celebratory, Cautious, Empathetic, Professional
    3. Use dynamic_text_template for runtime text generation (use {{variable}} syntax)
    4. Set adaptive_colors for sentiment-based color changes:
       - Positive: greens, blues (calming, success)
       - Neutral: grays, whites (professional)
       - Negative: reds, oranges (warning, attention)
    5. Adjust layout spacing/padding based on severity:
       - Critical: tight spacing, bold borders
       - Low/Info: relaxed spacing, soft edges
    6. Match mood with visual design:
       - Urgent: sharp corners, bright colors, exclamation marks
       - Calm: rounded corners, muted colors, gentle language
       - Celebratory: bright colors, larger text, positive emojis
       - Empathetic: warm colors, comforting language

    Example adaptive_colors:
    {
      "background_positive": "rgb(230, 250, 230)",
      "background_neutral": "rgb(245, 245, 245)",
      "background_negative": "rgb(255, 235, 235)",
      "text_positive": "rgb(0, 120, 0)",
      "text_neutral": "rgb(60, 60, 60)",
      "text_negative": "rgb(180, 0, 0)",
      "severity_indicator": "rgb(255, 100, 0)"
    }

    Example dynamic_text_template:
    "{{severity_prefix}} {{message_count}} messages require attention"

    {{ ctx.output_format }}
  "#
}

// Generate dynamic text based on current sentiment
function GenerateDynamicText(
  template: string,
  variables: map<string, string>,
  sentiment: Sentiment,
  mood: Mood
) -> string {
  client "openai/gpt-4o-mini"

  prompt #"
    Generate text from this template with emotional awareness.

    Template: {{ template }}
    Variables: {{ variables }}
    Sentiment: {{ sentiment }}
    Mood: {{ mood }}

    Replace {{variable}} placeholders with actual values.
    Adjust tone and word choice to match the sentiment and mood.

    Examples:
    - Urgent mood + negative sentiment: "⚠️ Critical: 5 errors need immediate attention"
    - Calm mood + positive sentiment: "All systems operating smoothly ✓"
    - Empathetic mood + negative sentiment: "We noticed some issues - let's work through them together"

    Return only the generated text, no explanation.
  "#
}
