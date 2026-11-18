/**
 * The branch that a lane of the graph stands for, to make it stand out under the pointer
 * (guidelines, 10.2 and 10.3). A column is taken again by another branch once the first one has
 * ended: the branch is found by following its line, not its column.
 */
import type { CommitRow } from './types';

/** The rows of a branch. */
export interface LaneRun {
  /** Its column, which is also its color. */
  lane: number;
  /** First and last row whose segments toward the next row belong to the branch. */
  top: number;
  bottom: number;
}

type Row = Pick<CommitRow, 'lane' | 'edges' | 'wip'>;

/** The branch whose line passes through row `index` in column `lane`. */
export function laneRun(rows: Row[], index: number, lane: number): LaneRun {
  const of = (r: number) => rows[r].edges.filter((e) => e[2] === lane);

  // Down: the segments of the row belong to the branch; it goes on as long as one of them goes on
  // in its column, and stops at the curve that merges it into another branch.
  let bottom = index - 1;
  for (let r = index; r < rows.length; r++) {
    const edges = of(r);
    if (!edges.length) break;
    bottom = r;
    if (!edges.some((e) => e[1] === lane)) break;
  }

  // Up: a row belongs to the branch when one of its segments comes into the column; the branch
  // goes on above as long as one of them enters the row in its column, and stops at the merge
  // that opens it. The changes in progress are not part of a branch.
  let top = index;
  for (let r = index - 1; r >= 0; r--) {
    if (rows[r].wip) break;
    const edges = of(r);
    if (!edges.some((e) => e[1] === lane)) break;
    top = r;
    if (!edges.some((e) => e[0] === lane)) break;
  }
  return { lane, top, bottom };
}

/** Whether the commit of row `index` is on the branch. */
export function onRun(run: LaneRun, rows: Row[], index: number): boolean {
  return index >= run.top && index <= run.bottom + 1 && rows[index].lane === run.lane && !rows[index].wip;
}

/** Whether a line of the graph passes through the middle of row `index` in column `lane`. */
export function passes(rows: Row[], index: number, lane: number): boolean {
  const straight = (e: [number, number, number]) => e[0] === lane && e[1] === lane;
  return rows[index].lane === lane || rows[index].edges.some(straight) || (index > 0 && rows[index - 1].edges.some(straight));
}
