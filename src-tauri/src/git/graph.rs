//! Calcul des lanes du graphe.
//!
//! Entrée : les commits dans l'ordre d'affichage (du plus récent au plus ancien, ordre
//! topologique), chacun avec ses parents. Sortie : pour chaque ligne, la colonne du commit et
//! les segments à tracer entre cette ligne et la suivante.

use std::collections::HashMap;

/// Mise en page d'une ligne du graphe.
#[derive(Debug, Clone, PartialEq)]
pub struct RowLayout {
    /// Colonne du commit.
    pub lane: u16,
    /// Segments vers la ligne suivante : `[colonne de départ, colonne d'arrivée, lane (couleur)]`.
    pub edges: Vec<[u16; 3]>,
}

fn intern<'a>(ids: &mut HashMap<&'a str, u32>, sha: &'a str) -> u32 {
    let next = ids.len() as u32;
    *ids.entry(sha).or_insert(next)
}

fn free_slot(lanes: &mut Vec<Option<u32>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// Écrit les segments qui partent de la ligne précédente.
///
/// `lanes` est l'état des lanes après cette ligne : chaque lane attend un commit.
/// `next` est le commit de la ligne suivante et la lane où il est attendu, s'il l'est.
fn emit_edges(
    edges: &mut Vec<[u16; 3]>,
    lanes: &[Option<u32>],
    lane: usize,
    started: &[usize],
    joins: &[usize],
    next: Option<(u32, Option<usize>)>,
) {
    let target = |j: usize, expected: u32| match next {
        Some((id, Some(found))) if id == expected => found,
        _ => j,
    };
    for (j, expected) in lanes.iter().enumerate() {
        let Some(expected) = *expected else { continue };
        // Une lane part du nœud si c'est la sienne ou si ce commit vient de l'ouvrir.
        let from = if j == lane || started.contains(&j) { lane } else { j };
        edges.push([from as u16, target(j, expected) as u16, j as u16]);
    }
    // Parent secondaire déjà attendu par une lane existante : un trait de plus depuis le nœud.
    for &j in joins {
        if let Some(Some(expected)) = lanes.get(j).copied() {
            edges.push([lane as u16, target(j, expected) as u16, j as u16]);
        }
    }
}

/// Attribue une lane à chaque commit et calcule les segments entre lignes.
/// Renvoie aussi le nombre maximal de lanes utilisées, pour dimensionner la colonne du graphe.
pub fn assign_lanes<'a>(commits: &[(&'a str, Vec<&'a str>)]) -> (Vec<RowLayout>, u16) {
    let mut ids: HashMap<&str, u32> = HashMap::with_capacity(commits.len() * 2);
    // lanes[j] = identifiant du commit attendu dans la colonne j.
    let mut lanes: Vec<Option<u32>> = Vec::new();
    let mut out: Vec<RowLayout> = Vec::with_capacity(commits.len());
    let mut max_lanes = 1usize;
    // Ligne précédente : (lane, lanes ouvertes par un parent secondaire, lanes rejointes).
    let mut prev: Option<(usize, Vec<usize>, Vec<usize>)> = None;

    for (sha, parents) in commits {
        let id = intern(&mut ids, sha);
        let found = lanes.iter().position(|l| *l == Some(id));

        // On connaît maintenant la ligne suivante de `prev` : ses segments peuvent être tracés.
        if let (Some((lane, started, joins)), Some(row)) = (prev.take(), out.last_mut()) {
            emit_edges(&mut row.edges, &lanes, lane, &started, &joins, Some((id, found)));
        }

        let lane = found.unwrap_or_else(|| free_slot(&mut lanes));
        // Les autres lanes qui attendaient ce commit convergent ici et se libèrent.
        for (j, slot) in lanes.iter_mut().enumerate() {
            if j != lane && *slot == Some(id) {
                *slot = None;
            }
        }

        let mut started = Vec::new();
        let mut joins = Vec::new();
        match parents.split_first() {
            None => lanes[lane] = None,
            Some((first, rest)) => {
                lanes[lane] = Some(intern(&mut ids, first));
                for parent in rest {
                    let pid = intern(&mut ids, parent);
                    match lanes.iter().position(|l| *l == Some(pid)) {
                        Some(existing) => joins.push(existing),
                        None => {
                            let slot = free_slot(&mut lanes);
                            lanes[slot] = Some(pid);
                            started.push(slot);
                        }
                    }
                }
            }
        }

        max_lanes = max_lanes.max(lane + 1).max(lanes.len());
        while matches!(lanes.last(), Some(None)) {
            lanes.pop();
        }
        out.push(RowLayout { lane: lane as u16, edges: Vec::new() });
        prev = Some((lane, started, joins));
    }

    // Dernière ligne chargée : les lanes encore ouvertes continuent vers le bas (historique tronqué).
    if let (Some((lane, started, joins)), Some(row)) = (prev, out.last_mut()) {
        emit_edges(&mut row.edges, &lanes, lane, &started, &joins, None);
    }

    (out, max_lanes.min(u16::MAX as usize) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'historique de la maquette : deux branches de fonctionnalité, deux merges, un WIP en tête.
    fn sample() -> Vec<(&'static str, Vec<&'static str>)> {
        vec![
            ("WIP", vec!["4c8a1d2"]),
            ("a1f3c9e", vec!["b7d2e41"]),
            ("9e04b7a", vec!["4c8a1d2"]),
            ("b7d2e41", vec!["4c8a1d2"]),
            ("4c8a1d2", vec!["e5b90f3", "77ad0c1"]),
            ("77ad0c1", vec!["d31f6a8"]),
            ("e5b90f3", vec!["2fa8c55"]),
            ("d31f6a8", vec!["2fa8c55"]),
            ("2fa8c55", vec!["8b1e7c0"]),
            ("8b1e7c0", vec!["61c4d9a"]),
            ("61c4d9a", vec!["f02a3b7"]),
            ("f02a3b7", vec!["5aa61e0", "3d7e2aa"]),
            ("3d7e2aa", vec!["c9d0b14"]),
            ("5aa61e0", vec!["c9d0b14"]),
            ("c9d0b14", vec!["0e7f2d9"]),
            ("0e7f2d9", vec!["7b3c1aa"]),
            ("7b3c1aa", vec![]),
        ]
    }

    #[test]
    fn lanes_of_the_sample_history() {
        let (rows, max) = assign_lanes(&sample());
        let lanes: Vec<u16> = rows.iter().map(|r| r.lane).collect();
        assert_eq!(lanes, vec![0, 1, 2, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(max, 3);
    }

    #[test]
    fn branches_converge_on_their_common_parent() {
        let (rows, _) = assign_lanes(&sample());
        // b7d2e41 (lane 1) : les trois lanes attendent 4c8a1d2, qui arrive en lane 0.
        assert_eq!(rows[3].edges, vec![[0, 0, 0], [1, 0, 1], [2, 0, 2]]);
    }

    #[test]
    fn merge_opens_a_lane_from_its_node() {
        let (rows, _) = assign_lanes(&sample());
        // 4c8a1d2 (lane 0) : continue en lane 0 et ouvre la lane 1 vers 77ad0c1.
        assert_eq!(rows[4].edges, vec![[0, 0, 0], [0, 1, 1]]);
    }

    #[test]
    fn root_commit_has_no_edge() {
        let (rows, _) = assign_lanes(&sample());
        assert!(rows.last().unwrap().edges.is_empty());
    }

    #[test]
    fn truncated_history_keeps_its_lanes_open() {
        let commits = vec![("b", vec!["a"]), ("c", vec!["a"])];
        let (rows, max) = assign_lanes(&commits);
        assert_eq!(max, 2);
        // Dernière ligne chargée : les deux lanes descendent encore vers `a`, non chargé.
        assert_eq!(rows[1].edges, vec![[0, 0, 0], [1, 1, 1]]);
    }

    #[test]
    fn fifty_thousand_commits_stay_fast() {
        // Historique synthétique : une branche principale, une branche courte fusionnée tous les 10 commits.
        let names: Vec<String> = (0..50_000).map(|i| format!("c{i}")).collect();
        let side: Vec<String> = (0..50_000).map(|i| format!("s{i}")).collect();
        let mut commits: Vec<(&str, Vec<&str>)> = Vec::new();
        for i in 0..49_999 {
            if i % 10 == 0 {
                commits.push((&names[i], vec![&names[i + 1], &side[i]]));
                commits.push((&side[i], vec![&names[i + 2]]));
            } else {
                commits.push((&names[i], vec![&names[i + 1]]));
            }
        }
        let start = std::time::Instant::now();
        let (rows, max) = assign_lanes(&commits);
        let elapsed = start.elapsed();
        println!("{} lignes, {max} lanes, {elapsed:?}", rows.len());
        assert_eq!(rows.len(), commits.len());
        assert!(max <= 3);
        assert!(elapsed.as_millis() < 2_000, "calcul trop lent : {elapsed:?}");
    }
}
