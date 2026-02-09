#!/bin/bash
# Check error handling coverage in Rust codebase

set -e

echo "Checking error handling patterns..."

# Find functions returning Result
result_count=$(rg "fn .*-> Result<" --type rust -c 2>/dev/null | awk -F: '{sum+=$2} END {print sum}')
echo "Functions returning Result: $result_count"

# Check for unwrap() usage (potential panics)
unwrap_count=$(rg "\.unwrap\(\)" --type rust -c 2>/dev/null | awk -F: '{sum+=$2} END {print sum}')
echo "⚠️  unwrap() calls found: $unwrap_count"

# Check for expect() with context
expect_count=$(rg "\.expect\(" --type rust -c 2>/dev/null | awk -F: '{sum+=$2} END {print sum}')
echo "expect() calls found: $expect_count"

# Check for ? operator usage
question_count=$(rg "\?\;" --type rust -c 2>/dev/null | awk -F: '{sum+=$2} END {print sum}')
echo "? operator uses: $question_count"

# Check for error types
echo ""
echo "Error type definitions:"
rg "Error.*\{" --type rust -A 2 2>/dev/null | head -20

echo ""
echo "Summary:"
echo "- Consider replacing unwrap() with proper error handling"
echo "- Ensure expect() messages are descriptive"
echo "- Verify error types implement std::error::Error"
