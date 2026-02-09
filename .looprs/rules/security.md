# Security Guidelines

Security is paramount. Follow these guidelines:

## Input Validation

- Validate all user input
- Sanitize file paths (no directory traversal)
- Prevent injection attacks

## Secrets Management

- Never commit secrets to git
- Use environment variables for API keys
- Rotate credentials regularly

## Dependencies

- Review security advisories regularly
- Update dependencies promptly
- Use `cargo audit` before releases
