class UiNode {
  tag "rect" | "label"
  text string?
  attrs map<string, string>
  children UiNode[]
}

function GenerateUiTree(goal: string, state_json: string) -> UiNode {
  client "openai/gpt-4o-mini"

  prompt #"
    You are generating a UI description for a Freya (rsx!) desktop app.

    Goal:
    {{ goal }}

    State (JSON):
    {{ state_json }}

    Rules:
    - Return a UiNode tree.
    - UiNode.tag MUST be either \"rect\" or \"label\".
    - UiNode.text is only used when tag is \"label\".
    - UiNode.attrs keys should be Freya-friendly layout attrs (examples: width, height, padding, background, direction, spacing).
    - Colors should be rgb(r, g, b) strings when used.
    - Keep the tree small (< 50 nodes).

    {{ ctx.output_format }}
  "#
}
