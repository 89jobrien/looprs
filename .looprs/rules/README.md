# Rules

System rules and constraints that govern behavior.

## Directory Structure

```
rules/
├── system-rules.md             # Core system behavior
├── code-rules.md               # Code quality and style
├── security-rules.md           # Security constraints
├── testing-rules.md            # Testing requirements
├── languages/
│   ├── rust-rules.md           # Rust-specific rules
│   ├── python-rules.md         # Python-specific rules
│   └── javascript-rules.md     # JavaScript-specific rules
└── README.md
```

## Rule Format

Plain markdown or text files with clear, actionable rules.

### System Rules

Core behavioral rules that apply universally.

**system-rules.md** example:
```markdown
# System Rules

## General
1. Always verify file paths are safe before operations
2. Ask for confirmation on destructive operations
3. Provide clear explanations for recommendations
4. Respect user preferences and project standards

## Output
1. Be concise and direct
2. Show code examples when helpful
3. Highlight important warnings or security issues
4. Suggest next steps when appropriate

## Error Handling
1. Never silently fail - always report errors
2. Provide actionable error messages
3. Suggest recovery steps
```

### Code Rules

Standards for code quality.

**code-rules.md** example:
```markdown
# Code Quality Rules

## General
1. Follow the project's existing code style
2. Write clear variable and function names
3. Add comments for non-obvious logic
4. Keep functions focused and single-purpose

## Performance
1. Avoid unnecessary allocations
2. Use appropriate data structures
3. Profile before optimizing
4. Document performance-critical code

## Maintainability
1. Write self-documenting code
2. Reduce coupling between modules
3. Follow DRY (Don't Repeat Yourself)
4. Use standard patterns from the ecosystem
```

### Language-Specific Rules

**rust-rules.md** example:
```markdown
# Rust Coding Rules

## Safety
1. Prefer Result over Option for errors
2. Handle all error cases explicitly
3. Use owned types for invariants
4. Avoid unsafe blocks without justification

## Idioms
1. Use iterators instead of manual loops
2. Leverage pattern matching
3. Use type system to prevent bugs
4. Prefer composition over inheritance

## Testing
1. Write unit tests alongside code
2. Test error paths, not just happy path
3. Use property-based testing for complex logic
4. Achieve >80% code coverage
```

## Rule Categories

| Category | Purpose | Examples |
|----------|---------|----------|
| System | Core behavior | Safety, user interaction, I/O |
| Code | Quality standards | Style, performance, maintainability |
| Security | Security constraints | Input validation, secrets, permissions |
| Testing | Testing requirements | Coverage, test types, edge cases |
| Language | Language-specific | Idioms, conventions, patterns |

## Using Rules

Rules are:
1. Provided to agents in system prompts
2. Referenced in skill examples
3. Enforced in code review agents
4. Customizable per project

## Example: Security Rules

**security-rules.md**:
```markdown
# Security Rules

## Input Validation
1. Sanitize all user input
2. Validate file paths are within project
3. Reject unexpectedly large inputs
4. Use allowlists instead of denylists

## Secrets Management
1. Never commit secrets to git
2. Use environment variables for API keys
3. Rotate secrets regularly
4. Log security events, not secrets

## Code Safety
1. No eval or code execution from input
2. Validate all external data
3. Use parameterized queries
4. Implement rate limiting

## Reporting
1. Log all security-relevant events
2. Report vulnerabilities internally first
3. Include timestamps and context
4. Never expose internal details to users
```

## Next: Implement Rule System

The system will need to:
1. Load rules from markdown/text files
2. Include relevant rules in agent system prompts
3. Enforce rules in validation phases
4. Allow projects to extend/override rules
