#!/bin/bash
# Must be run from repository root

if [ ! -d ".git" ]; then
    echo "Error: Must be run from the repository root."
    exit 1
fi

# Remove fuzz/target from git tracking (it should be ignored)
if git ls-files --error-unmatch fuzz/target > /dev/null 2>&1; then
    git rm -r --cached fuzz/target
else
    echo "fuzz/target not in git index, skipping removal."
fi

# Apply the patch for other fixes
git apply audit_reports/remediations/001_fixes.patch

echo "Cleanup complete. You can now verify and commit the changes."
