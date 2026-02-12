#!/usr/bin/env bash

# bump-version.sh - Version bumping and changelog organization script for looprs
#
# Usage:
#   ./scripts/bump-version.sh [major|minor|patch|VERSION] [OPTIONS]
#
# Examples:
#   ./scripts/bump-version.sh patch           # Bump patch version (0.1.11 -> 0.1.12)
#   ./scripts/bump-version.sh minor           # Bump minor version (0.1.11 -> 0.2.0)
#   ./scripts/bump-version.sh major           # Bump major version (0.1.11 -> 1.0.0)
#   ./scripts/bump-version.sh 0.2.0           # Set explicit version
#   ./scripts/bump-version.sh patch --dry-run # Preview changes without applying
#
# Options:
#   --dry-run    Preview changes without modifying files
#   --no-tag     Don't create git tag
#   --help       Show this help message

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script configuration
DRY_RUN=false
CREATE_TAG=true
CARGO_TOML="Cargo.toml"
CHANGELOG="CHANGELOG.md"

# Helper functions
log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1" >&2
}

show_help() {
    head -n 20 "$0" | grep "^#" | sed 's/^# //' | sed 's/^#//'
    exit 0
}

# Parse command line arguments
BUMP_TYPE=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --no-tag)
            CREATE_TAG=false
            shift
            ;;
        --help|-h)
            show_help
            ;;
        *)
            if [[ -z "$BUMP_TYPE" ]]; then
                BUMP_TYPE="$1"
            else
                log_error "Unknown argument: $1"
                exit 1
            fi
            shift
            ;;
    esac
done

# Validate bump type
if [[ -z "$BUMP_TYPE" ]]; then
    log_error "Version bump type required (major|minor|patch|VERSION)"
    echo "Run with --help for usage information"
    exit 1
fi

# Check required files exist
if [[ ! -f "$CARGO_TOML" ]]; then
    log_error "Cargo.toml not found"
    exit 1
fi

if [[ ! -f "$CHANGELOG" ]]; then
    log_error "CHANGELOG.md not found"
    exit 1
fi

# Check for uncommitted changes
if [[ -n "$(git status --porcelain)" ]]; then
    log_warning "You have uncommitted changes"
    if [[ "$DRY_RUN" == "false" ]]; then
        read -p "Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep -m 1 '^version = ' "$CARGO_TOML" | sed 's/version = "\(.*\)"/\1/')
log_info "Current version: $CURRENT_VERSION"

# Calculate new version
calculate_new_version() {
    local version="$1"
    local bump="$2"
    
    IFS='.' read -ra PARTS <<< "$version"
    local major="${PARTS[0]}"
    local minor="${PARTS[1]}"
    local patch="${PARTS[2]}"
    
    case "$bump" in
        major)
            echo "$((major + 1)).0.0"
            ;;
        minor)
            echo "${major}.$((minor + 1)).0"
            ;;
        patch)
            echo "${major}.${minor}.$((patch + 1))"
            ;;
        *)
            # Assume it's an explicit version number
            if [[ "$bump" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                echo "$bump"
            else
                log_error "Invalid version format: $bump (expected X.Y.Z)"
                exit 1
            fi
            ;;
    esac
}

NEW_VERSION=$(calculate_new_version "$CURRENT_VERSION" "$BUMP_TYPE")
log_info "New version: $NEW_VERSION"

if [[ "$DRY_RUN" == "true" ]]; then
    log_warning "DRY RUN MODE - No changes will be made"
fi

# Get the current date in YYYY-MM-DD format
RELEASE_DATE=$(date +%Y-%m-%d)

# Get commits since last tag (or all if no tags)
get_commits_since_last_tag() {
    local last_tag=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    
    if [[ -z "$last_tag" ]]; then
        log_info "No previous tags found, using all commits" >&2
        git log --pretty=format:"%s" --no-merges
    else
        log_info "Collecting commits since tag: $last_tag" >&2
        git log "${last_tag}..HEAD" --pretty=format:"%s" --no-merges
    fi
}

# Categorize commits by conventional commit type
categorize_commits() {
    local feat_commits=()
    local fix_commits=()
    local docs_commits=()
    local style_commits=()
    local refactor_commits=()
    local perf_commits=()
    local test_commits=()
    local chore_commits=()
    local other_commits=()
    
    while IFS= read -r commit; do
        if [[ -z "$commit" ]]; then
            continue
        fi
        
        case "$commit" in
            feat:*|feat\(*)
                feat_commits+=("$commit")
                ;;
            fix:*|fix\(*)
                fix_commits+=("$commit")
                ;;
            docs:*|docs\(*)
                docs_commits+=("$commit")
                ;;
            style:*|style\(*)
                style_commits+=("$commit")
                ;;
            refactor:*|refactor\(*)
                refactor_commits+=("$commit")
                ;;
            perf:*|perf\(*)
                perf_commits+=("$commit")
                ;;
            test:*|test\(*)
                test_commits+=("$commit")
                ;;
            chore:*|chore\(*)
                chore_commits+=("$commit")
                ;;
            *)
                other_commits+=("$commit")
                ;;
        esac
    done
    
    # Generate changelog entries
    local changelog_entries=""
    
    if [[ ${#feat_commits[@]} -gt 0 ]]; then
        changelog_entries+="### Added\n"
        for commit in "${feat_commits[@]}"; do
            # Remove "feat:" or "feat(scope):" prefix
            local clean_commit=$(echo "$commit" | sed -E 's/^feat(\([^)]+\))?:[[:space:]]*//')
            changelog_entries+="- $clean_commit\n"
        done
        changelog_entries+="\n"
    fi
    
    if [[ ${#fix_commits[@]} -gt 0 ]]; then
        changelog_entries+="### Fixed\n"
        for commit in "${fix_commits[@]}"; do
            local clean_commit=$(echo "$commit" | sed -E 's/^fix(\([^)]+\))?:[[:space:]]*//')
            changelog_entries+="- $clean_commit\n"
        done
        changelog_entries+="\n"
    fi
    
    if [[ ${#docs_commits[@]} -gt 0 ]]; then
        changelog_entries+="### Documentation\n"
        for commit in "${docs_commits[@]}"; do
            local clean_commit=$(echo "$commit" | sed -E 's/^docs(\([^)]+\))?:[[:space:]]*//')
            changelog_entries+="- $clean_commit\n"
        done
        changelog_entries+="\n"
    fi
    
    if [[ ${#refactor_commits[@]} -gt 0 || ${#perf_commits[@]} -gt 0 ]]; then
        changelog_entries+="### Changed\n"
        for commit in "${refactor_commits[@]}"; do
            local clean_commit=$(echo "$commit" | sed -E 's/^refactor(\([^)]+\))?:[[:space:]]*//')
            changelog_entries+="- $clean_commit\n"
        done
        for commit in "${perf_commits[@]}"; do
            local clean_commit=$(echo "$commit" | sed -E 's/^perf(\([^)]+\))?:[[:space:]]*//')
            changelog_entries+="- $clean_commit (performance improvement)\n"
        done
        changelog_entries+="\n"
    fi
    
    if [[ ${#other_commits[@]} -gt 0 ]]; then
        changelog_entries+="### Other\n"
        for commit in "${other_commits[@]}"; do
            changelog_entries+="- $commit\n"
        done
        changelog_entries+="\n"
    fi
    
    echo -e "$changelog_entries"
}

# Update Cargo.toml
update_cargo_toml() {
    log_info "Updating $CARGO_TOML..."
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would update version in $CARGO_TOML: $CURRENT_VERSION -> $NEW_VERSION"
        return
    fi
    
    # Use sed to update the version line
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS requires an empty string after -i
        sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"
    else
        sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"
    fi
    
    log_success "Updated $CARGO_TOML"
}

# Update CHANGELOG.md
update_changelog() {
    log_info "Updating $CHANGELOG..."
    
    # Get commits and categorize them
    local commits=$(get_commits_since_last_tag)
    local changelog_entries=$(echo "$commits" | categorize_commits)
    
    if [[ -z "$changelog_entries" ]]; then
        log_warning "No commits found to add to changelog"
        changelog_entries="### Changed\n- Version bump\n"
    fi
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would add to $CHANGELOG:"
        echo "----------------------------------------"
        echo -e "## [$NEW_VERSION] - $RELEASE_DATE\n"
        echo -e "$changelog_entries"
        echo "----------------------------------------"
        return
    fi
    
    # Create new version section
    local new_section="## [$NEW_VERSION] - $RELEASE_DATE\n\n$changelog_entries"
    
    # Find the line number of [Unreleased]
    local unreleased_line=$(grep -n "^## \[Unreleased\]" "$CHANGELOG" | cut -d: -f1)
    
    if [[ -z "$unreleased_line" ]]; then
        log_error "Could not find [Unreleased] section in $CHANGELOG"
        exit 1
    fi
    
    # Insert new version section after [Unreleased] and an empty line
    local insert_line=$((unreleased_line + 2))
    
    # Create temporary file with new content
    {
        head -n "$unreleased_line" "$CHANGELOG"
        echo ""
        echo -e "$new_section"
        tail -n "+$insert_line" "$CHANGELOG"
    } > "${CHANGELOG}.tmp"
    
    mv "${CHANGELOG}.tmp" "$CHANGELOG"
    
    log_success "Updated $CHANGELOG"
}

# Update Cargo.lock
update_cargo_lock() {
    log_info "Updating Cargo.lock..."
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would run: cargo update -p looprs"
        return
    fi
    
    cargo update -p looprs --quiet
    log_success "Updated Cargo.lock"
}

# Create git tag
create_git_tag() {
    if [[ "$CREATE_TAG" == "false" ]]; then
        log_info "Skipping git tag creation (--no-tag)"
        return
    fi
    
    log_info "Creating git tag v$NEW_VERSION..."
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would create tag: v$NEW_VERSION"
        return
    fi
    
    git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"
    log_success "Created tag v$NEW_VERSION"
    log_info "Push tag with: git push origin v$NEW_VERSION"
}

# Commit changes
commit_changes() {
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would commit changes with message: 'chore: bump version to $NEW_VERSION'"
        return
    fi
    
    log_info "Committing changes..."
    git add "$CARGO_TOML" "$CHANGELOG" Cargo.lock
    git commit -m "chore: bump version to $NEW_VERSION"
    log_success "Changes committed"
}

# Main execution
echo ""
log_info "=== Version Bump Script ==="
echo ""

# Perform updates
update_cargo_toml
update_changelog
update_cargo_lock

if [[ "$DRY_RUN" == "false" ]]; then
    commit_changes
    create_git_tag
fi

echo ""
log_success "Version bump complete: $CURRENT_VERSION -> $NEW_VERSION"

if [[ "$DRY_RUN" == "true" ]]; then
    echo ""
    log_info "This was a dry run. Run without --dry-run to apply changes."
else
    echo ""
    log_info "Next steps:"
    log_info "  1. Review the changes: git show HEAD"
    log_info "  2. Push changes: git push origin $(git branch --show-current)"
    if [[ "$CREATE_TAG" == "true" ]]; then
        log_info "  3. Push tag: git push origin v$NEW_VERSION"
    fi
    log_info "  4. Create GitHub release (optional)"
fi

echo ""
