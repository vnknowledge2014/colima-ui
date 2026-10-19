export interface ParsedTools {
  cleanText: string;
  hasTools: boolean;
  queries: string[][];
  eventApprovals: string[][];
  runs: string[][];
  runApprovals: string[][];
  learns: string[][];
  learnReasoning: string[][];
  prefs: string[][];
  hasDiagnose: boolean;
  hasQueryState: boolean;
  navigates: string[][];
  readReferences: string[][];
  secThreatModels: string[][];
  secVulnScans: string[][];
  secTriages: string[][];
  secPatchGens: string[][];
  schedCrons: string[][];
  schedTimers: string[][];
  schedCancels: string[][];
  /** `[label, prompt?]` per chip. Deliberately excluded from `hasTools`. */
  suggests: string[][];
}

/** The chat panel renders at most this many chips; the rest are dropped. */
export const MAX_SUGGESTIONS = 3;

interface ExtractedTag {
  match: string[];
  start: number;
  end: number;
}

const KNOWN_TAG_OPEN_RE =
  /^\[(?:QUERY|EVENT_APPROVE|RUN|RUN_APPROVE|DIAGNOSE|QUERY_APP_STATE|LEARN|LEARN_REASONING|REMEMBER_PREFERENCE|NAVIGATE|READ_REFERENCE|SECURITY_THREAT_MODEL|SECURITY_VULN_SCAN|SECURITY_TRIAGE|SECURITY_PATCH_GEN|SCHEDULE_CRON|SCHEDULE_TIMER|SCHEDULE_CANCEL|SUGGEST)(?::|\])/i;

function extractColonTags(
  text: string,
  tagName: string,
  options: {
    caseInsensitive?: boolean;
    splitPipe?: boolean;
    trimFirst?: boolean;
    firstPattern?: RegExp;
  } = {},
): ExtractedTag[] {
  const flags = options.caseInsensitive ? "gi" : "g";
  const openRe = new RegExp(`\\[${tagName}:\\s*`, flags);
  const results: ExtractedTag[] = [];

  let openMatch: RegExpExecArray | null;
  while ((openMatch = openRe.exec(text)) !== null) {
    const tagStart = openMatch.index;
    const contentStart = tagStart + openMatch[0].length;

    let braceDepth = 0;
    let bracketDepth = 0;
    let inJsonString = false;
    let topLevelPipeIdx = -1;
    let closeIdx = -1;

    for (let i = contentStart; i < text.length; i++) {
      const ch = text[i];
      if (inJsonString) {
        if (ch === "\\") {
          i++; // skip escaped character inside JSON string
        } else if (ch === '"') {
          inJsonString = false;
        }
        continue;
      }

      if ((braceDepth > 0 || bracketDepth > 0) && ch === '"') {
        inJsonString = true;
        continue;
      }

      if (ch === "{") {
        braceDepth++;
      } else if (ch === "}") {
        if (braceDepth > 0) braceDepth--;
      } else if (ch === "[") {
        if (braceDepth > 0 || bracketDepth > 0) {
          bracketDepth++;
        } else if (!KNOWN_TAG_OPEN_RE.test(text.slice(i))) {
          bracketDepth++;
        } else {
          // Another known tool tag started before this one closed; abort this malformed tag
          break;
        }
      } else if (
        ch === "|" &&
        braceDepth === 0 &&
        bracketDepth === 0 &&
        topLevelPipeIdx === -1
      ) {
        topLevelPipeIdx = i;
      } else if (ch === "]") {
        if (bracketDepth > 0) {
          bracketDepth--;
        } else {
          closeIdx = i;
          break;
        }
      }
    }

    if (closeIdx <= contentStart) {
      continue;
    }

    const fullMatch = text.slice(tagStart, closeIdx + 1);
    const inner = text.slice(contentStart, closeIdx);
    let matchArr: string[];

    if (options.splitPipe) {
      if (topLevelPipeIdx !== -1 && topLevelPipeIdx < closeIdx) {
        const relPipe = topLevelPipeIdx - contentStart;
        const rawFirst = inner.slice(0, relPipe);
        const first = options.trimFirst ? rawFirst.trim() : rawFirst;
        const second = inner.slice(relPipe + 1).replace(/^\s+/, "");
        if (!first || !second) continue;
        if (options.firstPattern && !options.firstPattern.test(first.trim())) {
          continue;
        }
        matchArr = [fullMatch, first, second];
      } else {
        const first = options.trimFirst ? inner.trim() : inner;
        if (!first) continue;
        if (options.firstPattern && !options.firstPattern.test(first.trim())) {
          continue;
        }
        matchArr = [fullMatch, first, undefined as unknown as string];
      }
    } else {
      if (!inner) continue;
      matchArr = [fullMatch, inner];
    }

    results.push({
      match: matchArr,
      start: tagStart,
      end: closeIdx + 1,
    });
    openRe.lastIndex = closeIdx + 1;
  }

  return results;
}

function stripSpans(
  text: string,
  spans: Array<{ start: number; end: number }>,
): string {
  if (spans.length === 0) return text;
  const sorted = [...spans].sort((a, b) => a.start - b.start);
  const merged: Array<{ start: number; end: number }> = [];
  for (const s of sorted) {
    const last = merged[merged.length - 1];
    if (!last || s.start > last.end) {
      merged.push({ ...s });
    } else if (s.end > last.end) {
      last.end = s.end;
    }
  }
  let out = "";
  let cursor = 0;
  for (const s of merged) {
    out += text.slice(cursor, s.start);
    cursor = s.end;
  }
  out += text.slice(cursor);
  return out;
}

export function parseAiTools(responseText: string): ParsedTools {
  const spans: Array<{ start: number; end: number }> = [];
  const collect = (items: ExtractedTag[]): string[][] => {
    for (const item of items) {
      spans.push({ start: item.start, end: item.end });
    }
    return items.map((item) => item.match);
  };

  const queries = collect(
    extractColonTags(responseText, "QUERY", {
      caseInsensitive: true,
      splitPipe: true,
    }),
  );
  const eventApprovals = collect(
    extractColonTags(responseText, "EVENT_APPROVE", {
      caseInsensitive: true,
      splitPipe: true,
    }),
  );

  const runs = collect(extractColonTags(responseText, "RUN"));
  const runApprovals = collect(extractColonTags(responseText, "RUN_APPROVE"));

  const diagnoseMatches = [...responseText.matchAll(/\[DIAGNOSE\]/g)];
  for (const m of diagnoseMatches) {
    spans.push({ start: m.index!, end: m.index! + m[0].length });
  }
  const hasDiagnose = diagnoseMatches.length > 0;

  const queryStateMatches = [...responseText.matchAll(/\[QUERY_APP_STATE\]/g)];
  for (const m of queryStateMatches) {
    spans.push({ start: m.index!, end: m.index! + m[0].length });
  }
  const hasQueryState = queryStateMatches.length > 0;

  const learns = collect(extractColonTags(responseText, "LEARN"));
  const learnReasoning = collect(
    extractColonTags(responseText, "LEARN_REASONING"),
  );
  const prefs = collect(extractColonTags(responseText, "REMEMBER_PREFERENCE"));
  const navigates = collect(
    extractColonTags(responseText, "NAVIGATE", { caseInsensitive: true }),
  );
  const readReferences = collect(
    extractColonTags(responseText, "READ_REFERENCE", { caseInsensitive: true }),
  );

  const secThreatModels = collect(
    extractColonTags(responseText, "SECURITY_THREAT_MODEL", {
      caseInsensitive: true,
      splitPipe: true,
    }),
  );
  const secVulnScans = collect(
    extractColonTags(responseText, "SECURITY_VULN_SCAN", {
      caseInsensitive: true,
    }),
  );
  const secTriages = collect(
    extractColonTags(responseText, "SECURITY_TRIAGE", {
      caseInsensitive: true,
    }),
  );
  const secPatchGens = collect(
    extractColonTags(responseText, "SECURITY_PATCH_GEN", {
      caseInsensitive: true,
      splitPipe: true,
    }),
  );

  const schedCrons = collect(
    extractColonTags(responseText, "SCHEDULE_CRON", {
      caseInsensitive: true,
      splitPipe: true,
    }),
  );
  const schedTimers = collect(
    extractColonTags(responseText, "SCHEDULE_TIMER", {
      caseInsensitive: true,
      splitPipe: true,
      trimFirst: true,
      firstPattern: /^\d+$/,
    }),
  );
  const schedCancels = collect(
    extractColonTags(responseText, "SCHEDULE_CANCEL", {
      caseInsensitive: true,
    }),
  );

  // Follow-ups the user can click. Not a tool: nothing runs until they do.
  const suggests = collect(
    extractColonTags(responseText, "SUGGEST", {
      caseInsensitive: true,
      splitPipe: true,
      trimFirst: true,
    }),
  );

  const cleanText = stripSpans(responseText, spans).trim();

  const hasTools =
    queries.length > 0 ||
    eventApprovals.length > 0 ||
    hasDiagnose ||
    hasQueryState ||
    runs.length > 0 ||
    runApprovals.length > 0 ||
    learns.length > 0 ||
    learnReasoning.length > 0 ||
    prefs.length > 0 ||
    navigates.length > 0 ||
    readReferences.length > 0 ||
    secThreatModels.length > 0 ||
    secVulnScans.length > 0 ||
    secTriages.length > 0 ||
    secPatchGens.length > 0 ||
    schedCrons.length > 0 ||
    schedTimers.length > 0 ||
    schedCancels.length > 0;

  return {
    cleanText,
    hasTools,
    queries,
    eventApprovals,
    runs,
    runApprovals,
    learns,
    learnReasoning,
    prefs,
    hasDiagnose,
    hasQueryState,
    navigates,
    readReferences,
    secThreatModels,
    secVulnScans,
    secTriages,
    secPatchGens,
    schedCrons,
    schedTimers,
    schedCancels,
    // Capped here rather than at the call site so every consumer gets the same
    // list and a chatty model cannot flood the panel.
    suggests: suggests.slice(0, MAX_SUGGESTIONS),
  };
}
