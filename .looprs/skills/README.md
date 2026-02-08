# Skills

Reusable, composable capabilities with progressive disclosure.

## Directory Structure

```
skills/
├── rust/                       # Rust-specific skills
│   ├── error-handling.json
│   ├── testing.json
│   └── performance.json
├── code-quality/               # General code quality
│   ├── refactoring.json
│   ├── testing.json
│   └── documentation.json
├── development/                # Development workflows
│   ├── debugging.json
│   └── optimization.json
└── README.md                   # This file
```

## Skill Format

```json
{
  "name": "skill_name",
  "description": "What this skill teaches",
  "category": "rust|python|testing|refactoring|documentation",
  "tags": ["tag1", "tag2"],
  "prerequisites": ["$other_skill"],
  "difficulty": "beginner|intermediate|advanced",
  "stages": [
    {
      "level": 1,
      "title": "Foundation",
      "description": "Basic concepts and patterns",
      "key_concepts": ["concept1", "concept2"],
      "examples": [
        {
          "title": "Simple Example",
          "code": "code snippet",
          "explanation": "What this teaches"
        }
      ],
      "practice": "Here's a challenge to try"
    },
    {
      "level": 2,
      "title": "Intermediate",
      "description": "Common patterns and best practices",
      "key_concepts": ["concept1"],
      "examples": [
        {
          "title": "Real-world Pattern",
          "code": "code snippet",
          "explanation": "When and why to use this"
        }
      ],
      "practice": "Complex challenge"
    },
    {
      "level": 3,
      "title": "Advanced",
      "description": "Advanced usage and optimization",
      "key_concepts": ["concept1"],
      "examples": [],
      "practice": "Expert challenge"
    }
  ],
  "related_skills": ["$skill1", "$skill2"],
  "resources": [
    {
      "title": "Reference",
      "url": "https://..."
    }
  ]
}
```

## Progressive Disclosure

Skills are designed with 3 levels:

1. **Foundation** - Core concepts, simple examples
2. **Intermediate** - Practical patterns, common use cases
3. **Advanced** - Optimization, edge cases, expert patterns

Users progress through levels as they request more detail.

## Usage

Invoke skills with `$` prefix:

- `$skill_name` - Load skill (starts at user's level)
- `$skill_name:1` - Load specific stage
- `$skill_name explain` - Get explanation
- `$skill_name example` - Show practical example
- `$skill_name practice` - Get practice challenge

## Example: Rust Error Handling

`skills/rust/error-handling.json`:
```json
{
  "name": "error-handling",
  "description": "Master Result<T,E> and error propagation",
  "category": "rust",
  "difficulty": "beginner",
  "stages": [
    {
      "level": 1,
      "title": "Result Basics",
      "key_concepts": ["Result", "Ok/Err", "Pattern Matching"],
      "examples": [
        {
          "title": "Using Result",
          "code": "fn parse_number(s: &str) -> Result<i32, ParseIntError> {\n    s.parse()\n}",
          "explanation": "Result is an enum with Ok and Err variants"
        }
      ],
      "practice": "Write a function that returns Result<String, Box<dyn Error>>"
    },
    {
      "level": 2,
      "title": "Error Propagation",
      "key_concepts": ["? operator", "From trait", "Error context"],
      "examples": [
        {
          "title": "Using ? operator",
          "code": "fn process() -> Result<(), Box<dyn Error>> {\n    let num = \"42\".parse::<i32>()?;\n    Ok(())\n}",
          "explanation": "? automatically converts and returns errors"
        }
      ],
      "practice": "Convert a chain of .unwrap() calls to use ?"
    },
    {
      "level": 3,
      "title": "Custom Error Types",
      "key_concepts": ["thiserror", "anyhow", "Error traits"],
      "examples": [],
      "practice": "Create a custom error type with conversion impl"
    }
  ],
  "related_skills": ["$rust-testing", "$rust-performance"]
}
```

Usage:
- `$error-handling` → Shows Foundation level
- `$error-handling:2` → Shows Intermediate level  
- `$error-handling explain` → Detailed explanation

## Next: Implement Skill System

The system will need to:
1. Load skills from JSON files
2. Track user's current level per skill
3. Parse `$skill_name` syntax
4. Render progressive disclosure (show detail gradually)
5. Link skills together
