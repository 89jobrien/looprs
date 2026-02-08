# Agents

Specialized agent definitions for different roles and tasks.

## Directory Structure

```
agents/
├── base-agent.yaml             # Default agent configuration
├── specializations/
│   ├── code-reviewer.yaml      # Code review specialist
│   ├── test-writer.yaml        # Test generation specialist
│   ├── debugger.yaml           # Debugging specialist
│   └── documenter.yaml         # Documentation specialist
└── README.md
```

## Agent Format

```yaml
name: "agent_name"
role: "The role/title of this agent"
description: "What this agent specializes in"

# Personality and behavior
system_prompt: |
  You are a {{ role }}.
  Your expertise is in {{ description }}.
  Follow these principles:
  - Be concise and practical
  - Explain your reasoning
  - Ask clarifying questions

# Tools this agent can use
tools:
  - read      # Read files
  - write     # Write files
  - edit      # Edit files
  - bash      # Execute commands
  - grep      # Search files
  - glob      # Find files

# Skills this agent is trained in
skills:
  - $error-handling
  - $testing-patterns
  - $performance-optimization

# Constraints and rules
constraints:
  - "Never modify production files without confirmation"
  - "Always write tests for new code"

# Commands this agent exposes
commands:
  - /code:review
  - /test:write
  - /debug:find-issue

# Knowledge sources
knowledge:
  - reference: "Rust Book"
    url: "https://doc.rust-lang.org"
  - reference: "OWASP Security"
    url: "https://owasp.org"

# Performance settings
config:
  max_tokens: 8192
  temperature: 0.7
  timeout_secs: 120
```

## Agent Types

### Base Agent
Default agent for general tasks. Balanced capabilities.

### Specialists
- **Code Reviewer** - Reviews code for quality, security, style
- **Test Writer** - Generates comprehensive test suites
- **Debugger** - Finds and explains bugs
- **Documenter** - Creates documentation and examples

## Usage

Agents are typically:
1. Invoked automatically based on task type
2. Chained together for complex workflows
3. Specialized by skill and tool access
4. Constrained by rules

## Example: Code Reviewer Agent

`agents/specializations/code-reviewer.yaml`:
```yaml
name: "code-reviewer"
role: "Senior Code Reviewer"
description: "Reviews code for quality, maintainability, and correctness"

system_prompt: |
  You are an experienced code reviewer.
  Focus on:
  - Code correctness and edge cases
  - Performance and memory safety
  - Maintainability and clarity
  - Security vulnerabilities
  - Test coverage
  
  Be constructive and suggest improvements.

tools:
  - read
  - edit
  - bash

skills:
  - $rust-idioms
  - $error-handling
  - $testing-patterns
  - $performance-optimization

constraints:
  - "Always highlight security issues"
  - "Request tests for critical code"
  - "Suggest refactoring for clarity"

commands:
  - /code:review

config:
  max_tokens: 4096
  temperature: 0.5
```

## Agent Orchestration

Multiple agents can work together:

```
User Input
  → Route to appropriate agent
  → Agent 1 (analyzes)
  → Agent 2 (implements)
  → Agent 3 (tests)
  → Return result
```

## Next: Implement Agent System

The system will need to:
1. Load agent definitions from YAML
2. Select agents based on task type
3. Provide appropriate tools and skills
4. Manage context across agents
5. Chain agents for complex workflows
