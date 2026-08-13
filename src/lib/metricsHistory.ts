/**
 * Bounded per-container history for the Activity page.
 *
 * Two ways this leaks if written carelessly, both of which show up only after the
 * page has been open for a long time:
 *
 * 1. Unbounded series. Every container's history is capped at a fixed number of
 *    points, oldest discarded.
 * 2. Unbounded *containers*. A machine that starts and stops containers all day
 *    would accumulate a series per container ever seen, even though the table
 *    shows none of them. Series for containers absent from a batch are dropped.
 *
 * `null` is a real value here: it marks samples the stream dropped, so a chart can
 * break its line instead of drawing through a gap it invented.
 */

import type { MetricSample } from "./api/metrics";

/** Points kept per container. At a 2s period this is four minutes of history. */
export const DEFAULT_LIMIT = 120;

/** The numeric fields worth charting. */
export type MetricField =
  | "cpuPct"
  | "memBytes"
  | "memPct"
  | "netRxBytes"
  | "netTxBytes"
  | "blockReadBytes"
  | "blockWriteBytes"
  | "pids";

/** Columns the table can be ordered by. */
export type SortKey =
  | "name"
  | "cpuPct"
  | "memBytes"
  | "memPct"
  | "netRxBytes"
  | "blockReadBytes"
  | "pids";

export interface ContainerHistory {
  /** Most recent sample, or null when the container's last point was a gap. */
  latest: MetricSample | null;
  /** Oldest first; `null` marks a dropped sample. */
  points: Array<MetricSample | null>;
}

export class MetricsHistory {
  private readonly limit: number;
  private series = new Map<string, ContainerHistory>();

  constructor(limit: number = DEFAULT_LIMIT) {
    this.limit = Math.max(1, limit);
  }

  /**
   * Record one batch.
   *
   * Containers missing from the batch are forgotten rather than left frozen at
   * their last value: a stale row is a wrong row, and keeping it is what makes
   * memory grow without bound.
   */
  push(samples: MetricSample[]): void {
    const seen = new Set<string>();

    for (const sample of samples) {
      seen.add(sample.containerId);
      let entry = this.series.get(sample.containerId);
      if (!entry) {
        entry = { latest: null, points: [] };
        this.series.set(sample.containerId, entry);
      }
      entry.latest = sample;
      entry.points.push(sample);
      if (entry.points.length > this.limit) {
        entry.points.splice(0, entry.points.length - this.limit);
      }
    }

    for (const id of this.series.keys()) {
      if (!seen.has(id)) this.series.delete(id);
    }
  }

  /**
   * Record that samples were lost.
   *
   * Every live series gets a hole, so charts break their line at the same place
   * and the table can show the row as stale rather than confidently wrong.
   */
  markGap(): void {
    for (const entry of this.series.values()) {
      entry.points.push(null);
      if (entry.points.length > this.limit) {
        entry.points.splice(0, entry.points.length - this.limit);
      }
      entry.latest = null;
    }
  }

  /** Latest sample per container, for the table body. */
  current(): MetricSample[] {
    const out: MetricSample[] = [];
    for (const entry of this.series.values()) {
      if (entry.latest) out.push(entry.latest);
    }
    return out;
  }

  /** True when the last thing recorded for this container was a gap. */
  isStale(containerId: string): boolean {
    const entry = this.series.get(containerId);
    return !!entry && entry.latest === null && entry.points.length > 0;
  }

  /** One field's history, gaps preserved as `null`. */
  seriesFor(containerId: string, field: MetricField): Array<number | null> {
    const entry = this.series.get(containerId);
    if (!entry) return [];
    return entry.points.map((p) => (p === null ? null : (p[field] as number)));
  }

  /** Containers currently tracked. */
  size(): number {
    return this.series.size;
  }

  /** Total points held, across every series — what a memory check asserts on. */
  pointCount(): number {
    let total = 0;
    for (const entry of this.series.values()) total += entry.points.length;
    return total;
  }

  clear(): void {
    this.series.clear();
  }
}
