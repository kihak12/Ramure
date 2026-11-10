//! Indexation par bloc ou par ligne : construit, à partir du diff d'un fichier, le patch
//! qui ne contient que les lignes choisies.

use std::collections::HashSet;

/// Lit `@@ -a,b +c,d @@` et renvoie `(a, c)`.
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

/// Numéro de la première ligne d'un côté de bloc : s'il est vide, git note la ligne qui précède.
fn start_of(begin: i64, count: i64) -> i64 {
    if count > 0 { begin } else { begin - 1 }
}

/// Construit le patch limité aux lignes `selected` (indices dans `diff.split('\n')`).
///
/// - `reverse = false` : le diff va de l'index au répertoire de travail, et le patch s'applique
///   à l'index pour y ajouter les lignes choisies. Une suppression non choisie redevient du
///   contexte, un ajout non choisi disparaît.
/// - `reverse = true` : le diff va de HEAD à l'index, et le patch s'applique à l'envers pour
///   retirer de l'index les lignes choisies. C'est alors l'ajout non choisi qui devient du
///   contexte, et la suppression non choisie qui disparaît.
pub fn partial_patch(diff: &str, selected: &HashSet<usize>, reverse: bool) -> Result<String, String> {
    let lines: Vec<&str> = diff.split('\n').collect();
    let mut out = String::new();
    let mut i = 0;

    // En-tête du fichier, recopié tel quel.
    let mut files = 0;
    while i < lines.len() && !lines[i].starts_with("@@") {
        let line = lines[i];
        if line.starts_with("diff --git ") {
            files += 1;
        }
        let whole_file_only = ["new file mode", "deleted file mode", "rename from", "copy from", "Binary files", "GIT binary patch"];
        if whole_file_only.iter().any(|p| line.starts_with(p)) {
            return Err("Ce fichier ne peut être indexé qu'en entier (nouveau, supprimé, renommé ou binaire).".into());
        }
        out.push_str(line);
        out.push('\n');
        i += 1;
    }
    if files != 1 || i == lines.len() {
        return Err("Le diff ne contient pas un fichier texte modifié.".into());
    }

    // Décalage accumulé entre les deux côtés par les blocs déjà écrits.
    let mut delta: i64 = 0;
    let mut written = false;
    while i < lines.len() {
        let header = lines[i];
        if header.is_empty() && i == lines.len() - 1 {
            break; // fin du texte : artefact du découpage sur `\n`
        }
        let (old_start, new_start) =
            hunk_starts(header).ok_or_else(|| format!("bloc de diff illisible : {header}"))?;
        i += 1;

        let mut body: Vec<String> = Vec::new();
        let (mut old_count, mut new_count) = (0i64, 0i64);
        let (mut orig_old, mut orig_new) = (0i64, 0i64);
        let mut changed = false;
        // La mention « \ No newline at end of file » suit le sort de la ligne qui la précède.
        let mut last_kept = true;
        while i < lines.len() && !lines[i].starts_with("@@") {
            let line = lines[i];
            if line.is_empty() && i == lines.len() - 1 {
                i += 1;
                break;
            }
            if line.starts_with("diff --git ") {
                return Err("Le diff contient plusieurs fichiers.".into());
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
                    // Ligne de contexte (une ligne vide est un contexte dont l'espace a été perdu).
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

        // Le côté sur lequel git cherche le bloc (l'ancien à l'endroit, le nouveau à l'envers)
        // garde sa position d'origine ; l'autre est décalé par les blocs précédents.
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

    if written { Ok(out) } else { Err("Aucune ligne modifiée n'est sélectionnée.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,5 @@\n un\n-deux\n+DEUX\n+deux et demi\n trois\n quatre\n@@ -10,3 +11,2 @@\n dix\n-onze\n douze\n";

    fn pick(indices: &[usize]) -> HashSet<usize> {
        indices.iter().copied().collect()
    }

    #[test]
    fn forward_patch_keeps_only_selected_lines() {
        // Ligne 8 : « +deux et demi ». La suppression de « deux » redevient du contexte.
        let patch = partial_patch(DIFF, &pick(&[8]), false).unwrap();
        assert_eq!(
            patch,
            "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,5 @@\n un\n deux\n+deux et demi\n trois\n quatre\n"
        );
    }

    #[test]
    fn later_hunks_are_shifted_by_earlier_ones() {
        // Lignes 6 et 7 (« -deux », « +DEUX ») puis 13 (« -onze ») : le premier bloc ne change
        // pas la longueur, le second garde donc sa position.
        let patch = partial_patch(DIFF, &pick(&[6, 7, 13]), false).unwrap();
        assert!(patch.contains("@@ -1,4 +1,4 @@\n un\n-deux\n+DEUX\n trois\n quatre\n"));
        assert!(patch.contains("@@ -10,3 +10,2 @@\n dix\n-onze\n douze\n"));
    }

    #[test]
    fn reverse_patch_turns_unselected_additions_into_context() {
        let patch = partial_patch(DIFF, &pick(&[8]), true).unwrap();
        // « DEUX » est dans l'index : contexte. « deux » n'y est pas : absent.
        assert!(patch.contains("@@ -1,4 +1,5 @@\n un\n DEUX\n+deux et demi\n trois\n quatre\n"));
        assert!(!patch.contains("onze"));
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
