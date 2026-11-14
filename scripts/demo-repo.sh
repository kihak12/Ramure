#!/usr/bin/env bash
# Builds the demo repository of the screenshots: those at the top of the guidelines (section 14)
# and those of the ux-designer trial (.claude/tests/ux-designer/). Feature branches merged into
# main, two tags, a remote that main is two commits ahead of, a branch never pushed, and two files
# with uncommitted changes, one of them with an added line in src/main.rs.
#
#   scripts/demo-repo.sh <folder>    creates <folder>/workshop, the repository, and <folder>/origin.git
#
# Then: scripts/screen.sh open light <folder>/workshop
set -euo pipefail
base=$(realpath "$1")
rm -rf "$base/workshop" "$base/origin.git"
mkdir -p "$base/workshop"
cd "$base/workshop"
git init -q -b main
git config user.name "Camille Roux"
git config user.email "camille@example.com"

when() { export GIT_AUTHOR_DATE="$1" GIT_COMMITTER_DATE="$1"; }
as() { export GIT_AUTHOR_NAME="$1" GIT_AUTHOR_EMAIL="$2" GIT_COMMITTER_NAME="$1" GIT_COMMITTER_EMAIL="$2"; }
camille() { as "Camille Roux" camille@example.com; }
noah() { as "Noah Martin" noah@example.com; }
ines() { as "Ines Dubois" ines@example.com; }
edit() { mkdir -p "$(dirname "$1")"; printf '%s\n' "$2" >> "$1"; git add "$1"; }
commit() { git commit -q -m "$1"; }

mkdir -p src
camille; when "2026-11-10T09:00:00"
edit src/main.rs 'fn main() {'; edit src/main.rs '    let config = config::load();'; edit src/main.rs '    app::run(config);'; edit src/main.rs '}'
edit src/config.rs '// Reads the configuration file.'; commit "Read the configuration file"
noah; when "2026-11-10T09:40:00"
edit src/log.rs '// Log levels and timestamps.'; commit "Log: levels and timestamps"
camille; when "2026-11-10T10:10:00"
edit src/config.rs '// Default values.'; commit "Configuration: default values"
git tag -a v0.1.0 -m "v0.1.0"

git switch -q -c import/csv
ines; when "2026-11-10T10:30:00"; edit src/csv.rs '// Reads the header row.'; commit "CSV import: read the header row"
ines; when "2026-11-10T11:00:00"; edit src/csv.rs '// Detects the separator.'; commit "CSV import: detect the separator"
ines; when "2026-11-10T11:20:00"; edit src/csv.rs '// Skips empty lines.'; commit "CSV import: skip empty lines"
git switch -q main
camille; when "2026-11-10T11:45:00"; git merge -q --no-ff import/csv -m "Merge the CSV import"
camille; when "2026-11-11T08:30:00"; edit src/log.rs '// Path of the log on Windows.'; commit "Fix the log path on Windows"
git tag -a v0.2.0 -m "v0.2.0"

git switch -q -c export/pdf
ines; when "2026-11-11T09:00:00"; edit src/pdf.rs '// Page layout.'; commit "PDF export: page layout"
ines; when "2026-11-11T09:30:00"; edit src/pdf.rs '// Embedded fonts.'; commit "PDF export: embed the fonts"
git switch -q main
camille; when "2026-11-11T09:50:00"; edit Cargo.toml '[dependencies]'; commit "Update the dependencies"

git switch -q -c ui/search
noah; when "2026-11-10T14:00:00"; edit src/search.rs '// Field and shortcut.'; commit "Search: field and shortcut"
noah; when "2026-11-10T15:00:00"; edit src/search.rs '// Highlights the results.'; commit "Search: highlight the results"
noah; when "2026-11-11T10:10:00"; edit src/search.rs '// Keyboard navigation.'; commit "Search: results from the keyboard"
git switch -q main
camille; when "2026-11-11T10:30:00"; git merge -q --no-ff ui/search -m "Merge the search"
noah; when "2026-11-11T10:50:00"; edit src/search.rs '// Message when nothing matches.'; commit "Search: a message when nothing matches"
camille; when "2026-11-11T11:20:00"; edit src/log.rs '// Rotation of the files.'; commit "Log: rotate the files"

# The remote: everything up to here, then main moves on by two local commits.
git clone -q --bare . "$base/origin.git"
git remote add origin "$base/origin.git"
git fetch -q origin
git branch -q -u origin/main main
for b in import/csv export/pdf ui/search; do git branch -q -u "origin/$b" "$b"; done

camille; when "2026-11-11T11:30:00"; edit src/log.rs '// Older files are compressed.'; commit "Log: compress the older files"
camille; when "2026-11-11T11:40:00"; edit Cargo.toml 'version = "0.3.0"'; commit "Prepare version 0.3.0"
git switch -q export/pdf
ines; when "2026-11-11T12:00:00"; edit src/pdf.rs '// Margins.'; commit "PDF export: set the margins"
git switch -q -c fix/encoding main
noah; when "2026-11-11T12:20:00"; edit src/csv.rs '// Accented names.'; commit "Fix accented characters in names"
git switch -q main

# Two files with uncommitted changes, one of them with an added line in src/main.rs.
sed -i 's/^    app::run(config);$/    log::init(\&config);\n    app::run(config);/' src/main.rs
printf '%s\n' '// Encodings other than UTF-8.' >> src/csv.rs
unset GIT_AUTHOR_DATE GIT_COMMITTER_DATE
echo "Demo repository: $base/workshop"
