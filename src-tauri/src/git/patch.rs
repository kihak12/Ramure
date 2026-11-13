//! Staging by hunk or by line: builds, from the diff of a file, the patch that holds only the
//! chosen lines.

use std::collections::HashSet;

/// Reads `@@ -a,b +c,d @@` and returns `(a, c)`.
fn hunk_starts(header: &str) -> Option<(i64, i64)> {
    let mut parts = header.split(' ');
    if parts.next()? != "@@" {
        return None;
    }
    let start = |part: &str, sign: char| -> Option<i64> {
        part.strip_prefix(sign)?.split(',').next()?.parse().ok()
    };
    let old = start(parts.next()?, '-')?;
    let new = start(parts.next()?, '+')?;
    Some((old, new))
}

/// Number of the first line of one side of a hunk: when that side is empty, git gives the line before.
fn start_of(begin: i64, count: i64) -> i64 {
    if count > 0 { begin } else { begin - 1 }
}

/// Builds the patch limited to the `selected` lines (indices in `diff.split('\n')`).
///
/// - `reverse = false`: the diff goes from the index to the working tree, and the patch applies
///   to the index to add the chosen lines to it. A deletion that was not chosen becomes context
///   again, and an addition that was not chosen goes away.
/// - `reverse = true`: the diff goes from HEAD to the index, and the patch applies in reverse to
///   take the chosen lines out of the index. It is then the addition not chosen that becomes
///   context, and the deletion not chosen that goes away.
pub fn partial_patch(diff: &str, selected: &HashSet<usize>, reverse: bool) -> Result<String, String> {
    let lines: Vec<&str> = diff.split('\n').collect();
    let mut out = String::new();
    let mut i = 0;

    // Header of the file, copied as it is.
    let mut files = 0;
    while i < lines.len() && !lines[i].starts_with("@@") {
        let line = lines[i];
        if line.starts_with("diff --git ") {
            files += 1;
        }
        let whole_file_only = ["new file mode", "deleted file mode", "rename from", "copy from", "Binary files", "GIT binary patch"];
        if whole_file_only.iter().any(|p| line.starts_with(p)) {
            return Err("This file can only be staged as a whole: it is new, deleted, renamed or binary.".into());
        }
        out.push_str(line);
        out.push('\n');
        i += 1;
    }
    if files != 1 || i == lines.len() {
        return Err("The diff does not contain a modified text file.".into());
    }

    // Offset between the two sides, built up by the hunks already written.
    let mut delta: i64 = 0;
    let mut written = false;
    while i < lines.len() {
        let header = lines[i];
        if header.is_empty() && i == lines.len() - 1 {
            break; // end of the text: an artifact of splitting on `\n`
        }
        let (old_start, new_start) =
            hunk_starts(header).ok_or_else(|| format!("Unreadable diff hunk: {header}"))?;
        i += 1;

        let mut body: Vec<String> = Vec::new();
        let (mut old_count, mut new_count) = (0i64, 0i64);
        let (mut orig_old, mut orig_new) = (0i64, 0i64);
        let mut changed = false;
        // The “\ No newline at end of file” note shares the fate of the line before it.
        let mut last_kept = true;
        while i < lines.len() && !lines[i].starts_with("@@") {
            let line = lines[i];
            if line.is_empty() && i == lines.len() - 1 {
                i += 1;
                break;
            }
            if line.starts_with("diff --git ") {
                return Err("The diff contains several files.".into());
            }
            let picked = selected.contains(&i);
            let rest = line.get(1..).unwrap_or("");
            match line.as_bytes().first() {
                Some(b'+') => {
                    orig_new += 1;
                    if picked {
                        body.push(line.to_string());
                        new_count += 1;
                        changed = true;
                        last_kept = true;
                    } else if reverse {
                        body.push(format!(" {rest}"));
                        old_count += 1;
                        new_count += 1;
                        last_kept = true;
                    } else {
                        last_kept = false;
                    }
                }
                Some(b'-') => {
                    orig_old += 1;
                    if picked {
                        body.push(line.to_string());
                        old_count += 1;
                        changed = true;
                        last_kept = true;
                    } else if reverse {
                        last_kept = false;
                    } else {
                        body.push(format!(" {rest}"));
                        old_count += 1;
                        new_count += 1;
                        last_kept = true;
                    }
                }
                Some(b'\\') => {
                    if last_kept {
                        body.push(line.to_string());
                    }
                }
                _ => {
                    // Context line (an empty line is context whose space was lost).
                    orig_old += 1;
                    orig_new += 1;
                    body.push(format!(" {rest}"));
                    old_count += 1;
                    new_count += 1;
                    last_kept = true;
                }
            }
            i += 1;
        }
        if !changed {
            continue;
        }

        // The side where git looks for the hunk (the old one forward, the new one in reverse)
        // keeps its original position; the other is shifted by the previous hunks.
        let (old_at, new_at) = if reverse {
            let begin = if orig_new > 0 { new_start } else { new_start + 1 };
            (start_of(begin - delta, old_count), start_of(begin, new_count))
        } else {
            let begin = if orig_old > 0 { old_start } else { old_start + 1 };
            (start_of(begin, old_count), start_of(begin + delta, new_count))
        };
        delta += new_count - old_count;
        out.push_str(&format!("@@ -{old_at},{old_count} +{new_at},{new_count} @@\n"));
        for line in body {
            out.push_str(&line);
            out.push('\n');
        }
        written = true;
    }

    if written { Ok(out) } else { Err("No changed line is selected. Select at least one added or removed line.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,5 @@\n one\n-two\n+TWO\n+two and a half\n three\n four\n@@ -10,3 +11,2 @@\n ten\n-eleven\n twelve\n";

    fn pick(indices: &[usize]) -> HashSet<usize> {
        indices.iter().copied().collect()
    }

    #[test]
    fn forward_patch_keeps_only_selected_lines() {
        // Line 8: “+two and a half”. The deletion of “two” becomes context again.
        let patch = partial_patch(DIFF, &pick(&[8]), false).unwrap();
        assert_eq!(
            patch,
            "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,5 @@\n one\n two\n+two and a half\n three\n four\n"
        );
    }

    #[test]
    fn later_hunks_are_shifted_by_earlier_ones() {
        // Lines 6 and 7 (“-two”, “+TWO”), then 13 (“-eleven”): the first hunk does not change
        // the length, so the second keeps its position.
        let patch = partial_patch(DIFF, &pick(&[6, 7, 13]), false).unwrap();
        assert!(patch.contains("@@ -1,4 +1,4 @@\n one\n-two\n+TWO\n three\n four\n"));
        assert!(patch.contains("@@ -10,3 +10,2 @@\n ten\n-eleven\n twelve\n"));
    }

    #[test]
    fn reverse_patch_turns_unselected_additions_into_context() {
        let patch = partial_patch(DIFF, &pick(&[8]), true).unwrap();
        // “TWO” is in the index: context. “two” is not: absent.
        assert!(patch.contains("@@ -1,4 +1,5 @@\n one\n TWO\n+two and a half\n three\n four\n"));
        assert!(!patch.contains("eleven"));
    }

    #[test]
    fn nothing_selected_is_an_error() {
        assert!(partial_patch(DIFF, &pick(&[5]), false).is_err());
        assert!(partial_patch(DIFF, &HashSet::new(), false).is_err());
    }

    #[test]
    fn new_files_are_refused() {
        let diff = "diff --git a/n b/n\nnew file mode 100644\nindex 000..111\n--- /dev/null\n+++ b/n\n@@ -0,0 +1 @@\n+x\n";
        assert!(partial_patch(diff, &pick(&[6]), false).is_err());
    }
}
