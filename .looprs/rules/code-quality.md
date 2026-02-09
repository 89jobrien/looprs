# Code Quality Standards

All code changes must meet the following quality standards:

## Testing

- Write unit tests for all new functions
- Integration tests for new features
- Aim for >80% test coverage

## Error Handling

- Use `Result` types for operations that can fail
- Provide context with `.context()` or `.with_context()`
- Never use `unwrap()` or `expect()` in production code

## Documentation

- Public APIs must have doc comments
- Complex algorithms need inline comments
- Update README when adding features

## Performance

- Profile before optimizing
- Avoid unnecessary cloning in hot paths
- Use references where possible
