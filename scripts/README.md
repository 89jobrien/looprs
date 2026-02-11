# Scripts

This directory contains automation scripts for the looprs project.

## bump-version.sh

Version bumping and changelog organization script for looprs releases.

### Features

- **Semantic versioning**: Bump major, minor, or patch versions (or set explicit version)
- **Changelog organization**: Automatically organizes git commits into changelog entries
- **Conventional commits**: Categorizes commits by type (feat:, fix:, docs:, etc.)
- **Keep a Changelog format**: Follows standard changelog format
- **Git integration**: Creates annotated git tags for releases
- **Safety**: Dry-run mode for previewing changes

### Usage

#### Quick Start with Makefile

The easiest way to bump versions:

```bash
make version-patch   # 0.1.11 -> 0.1.12
make version-minor   # 0.1.11 -> 0.2.0
make version-major   # 0.1.11 -> 1.0.0
```

#### Direct Script Usage

```bash
# Bump patch version (recommended for bug fixes)
./scripts/bump-version.sh patch

# Bump minor version (new features)
./scripts/bump-version.sh minor

# Bump major version (breaking changes)
./scripts/bump-version.sh major

# Set explicit version
./scripts/bump-version.sh 0.2.0

# Preview changes without applying (dry-run)
./scripts/bump-version.sh patch --dry-run

# Skip git tag creation
./scripts/bump-version.sh patch --no-tag
```

### What It Does

1. **Reads current version** from `Cargo.toml`
2. **Calculates new version** based on bump type
3. **Collects commits** since last git tag (or all if no tags)
4. **Categorizes commits** by conventional commit type:
   - `feat:` → Added section
   - `fix:` → Fixed section
   - `docs:` → Documentation section
   - `refactor:`, `perf:` → Changed section
   - Other commits → Other section
5. **Updates CHANGELOG.md** with new version section and organized entries
6. **Updates Cargo.toml** with new version
7. **Updates Cargo.lock** to match
8. **Commits changes** with message: `chore: bump version to X.Y.Z`
9. **Creates git tag** `vX.Y.Z` (unless `--no-tag` specified)

### Conventional Commits

For best results, use conventional commit format:

```bash
feat: add new feature
fix: resolve bug in parser
docs: update README
refactor: reorganize module structure
perf: optimize search algorithm
test: add unit tests for tools
chore: update dependencies
```

The script will automatically categorize these into appropriate changelog sections.

### Options

- `--dry-run` - Preview changes without modifying any files
- `--no-tag` - Skip creating git tag
- `--help` - Show help message

### Workflow

Typical release workflow:

```bash
# 1. Make sure your changes are committed
git status

# 2. Preview the version bump
make version-patch --dry-run  # or use: bash scripts/bump-version.sh patch --dry-run

# 3. Apply the version bump
make version-patch

# 4. Review the changes
git show HEAD

# 5. Push to remote
git push origin main
git push origin v0.1.12  # or whatever version was created

# 6. (Optional) Create GitHub release
```

### Requirements

- Git repository with commits
- `Cargo.toml` with version field
- `CHANGELOG.md` with `[Unreleased]` section

### Troubleshooting

**"Could not find [Unreleased] section"**
- Ensure your `CHANGELOG.md` has a `## [Unreleased]` section at the top

**"You have uncommitted changes"**
- Commit or stash your changes before running the script
- Or use `--dry-run` to preview without committing

**"No commits found"**
- The script will add a generic "Version bump" entry
- Consider using conventional commit format for better changelog organization
