export interface ChoiceOption {
  label: string;
  description?: string;
  value: string;
}

export interface ParsedChoicePrompt {
  question: string;
  multi: boolean;
  options: ChoiceOption[];
}

export type StreamingTagKind =
  | "query"
  | "event_approve"
  | "run"
  | "choice"
  | "suggest"
  | "diagnose"
  | "security"
  | "schedule"
  | "navigate"
  | "memory";

export interface StreamingPartialTagResult {
  cleanText: string;
  pendingTagKind: StreamingTagKind | null;
}

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
  /** Interactive choice card prompt, if present. Deliberately excluded from `hasTools`. */
  choicePrompt?: ParsedChoicePrompt | null;
}

/** The chat panel renders at most this many chips; the rest are dropped. */
export const MAX_SUGGESTIONS = 3;

interface ExtractedTag {
  match: string[];
  start: number;
  end: number;
}

const KNOWN_TAG_OPEN_RE =
  /^\[(?:QUERY_APP_STATE|QUERY|EVENT_APPROVE|RUN_APPROVE|RUN|DIAGNOSE|LEARN_REASONING|LEARN|REMEMBER_PREFERENCE|NAVIGATE|READ_REFERENCE|SECURITY_THREAT_MODEL|SECURITY_VULN_SCAN|SECURITY_TRIAGE|SECURITY_PATCH_GEN|SCHEDULE_CRON|SCHEDULE_TIMER|SCHEDULE_CANCEL|SUGGEST|ASK_CHOICE)(?::|\])/i;

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

/**
 * Extracts [ASK_CHOICE: ...] tag, parsing both structured JSON and compact pipe syntax.
 */
export function extractChoicePrompt(text: string): {
  cleanText: string;
  choicePrompt: ParsedChoicePrompt | null;
  rawTag?: string;
} {
  const choiceTags = extractColonTags(text, "ASK_CHOICE", { caseInsensitive: true });
  if (choiceTags.length === 0) {
    return { cleanText: text, choicePrompt: null };
  }

  const tag = choiceTags[0];
  const inner = tag.match[1]?.trim() ?? "";
  const rawTag = tag.match[0];
  let choicePrompt: ParsedChoicePrompt | null = null;

  // 1. Try parsing JSON format
  let jsonParsed = false;
  if (inner.startsWith("{")) {
    try {
      const parsed = JSON.parse(inner);
      jsonParsed = true;
      const question = typeof parsed?.question === "string" ? parsed.question.trim() : "";
      if (question && Array.isArray(parsed.options)) {
        const options: ChoiceOption[] = parsed.options
          .map((opt: unknown) => {
            if (typeof opt === "string") {
              const trimmed = opt.trim();
              return trimmed ? { label: trimmed, value: trimmed } : null;
            } else if (opt && typeof opt === "object") {
              const o = opt as Record<string, unknown>;
              const label = String(o.label ?? "").trim();
              if (!label) return null;
              return {
                label,
                description: o.description ? String(o.description).trim() : undefined,
                value: String(o.value ?? label).trim(),
              };
            }
            return null;
          })
          .filter((o: ChoiceOption | null): o is ChoiceOption => o !== null);

        if (options.length > 0) {
          choicePrompt = {
            question,
            multi: Boolean(parsed.multi),
            options,
          };
        }
      }
    } catch {
      // Fall through to pipe format
    }
  }

  // 2. Try parsing pipe format: [ASK_CHOICE: Question | Opt1 | Opt2]
  if (!choicePrompt && !jsonParsed) {
    const rawParts = inner.split("|").map((p) => p.trim());
    if (rawParts.length >= 2) {
      let rawQuestion = rawParts[0];
      let multi = false;

      if (/^\[multi\]/i.test(rawQuestion)) {
        multi = true;
        rawQuestion = rawQuestion.replace(/^\[multi\]\s*/i, "");
      } else if (/^\(multi\)/i.test(rawQuestion)) {
        multi = true;
        rawQuestion = rawQuestion.replace(/^\(multi\)\s*/i, "");
      }

      const question = rawQuestion.trim();
      if (question.length > 0) {
        const optionParts = rawParts.slice(1).filter(Boolean);
        const options: ChoiceOption[] = optionParts.map((optStr) => {
          const colonIdx = optStr.indexOf(":");
          if (colonIdx > 0 && colonIdx < optStr.length - 1) {
            const label = optStr.slice(0, colonIdx).trim();
            const description = optStr.slice(colonIdx + 1).trim();
            return { label, description, value: label };
          }
          return { label: optStr, value: optStr };
        });

        if (options.length > 0) {
          choicePrompt = {
            question,
            multi,
            options,
          };
        }
      }
    }
  }

  // Strip all choice tag spans from cleanText
  const spans = choiceTags.map((t) => ({ start: t.start, end: t.end }));
  const cleanText = stripSpans(text, spans).trim();

  return { cleanText, choicePrompt, rawTag };
}

/**
 * Balance-aware bracket checker to determine if a tag starting at tail[0] is closed.
 * Correctly accounts for nested brackets (e.g. JSON arrays), braces, quotes, and escape characters.
 */
function isTagClosed(tail: string): boolean {
  let bracketDepth = 0;
  let braceDepth = 0;
  let inString = false;
  let inEscape = false;

  for (let i = 0; i < tail.length; i++) {
    const ch = tail[i];

    if (inString) {
      if (inEscape) {
        inEscape = false;
      } else if (ch === "\\") {
        inEscape = true;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }

    if (ch === '"') {
      inString = true;
      continue;
    }

    if (ch === "{") {
      braceDepth++;
    } else if (ch === "}") {
      if (braceDepth > 0) braceDepth--;
    } else if (ch === "[") {
      bracketDepth++;
    } else if (ch === "]") {
      if (bracketDepth > 1) {
        bracketDepth--;
      } else if (bracketDepth === 1) {
        if (braceDepth === 0) {
          return true;
        }
      }
    }
  }

  return false;
}

/**
 * Strips unclosed tool/interactive tags at the tail of streaming text,
 * preventing flickering while preserving regular markdown links and brackets.
 */
export function stripStreamingPartialTags(text: string): StreamingPartialTagResult {
  if (!text) {
    return { cleanText: "", pendingTagKind: null };
  }

  // 1. Strip completed known tags first
  const parsed = parseAiTools(text);
  const workingText = parsed.cleanText;

  // 2. Check for an unclosed tag at the tail of workingText
  // Matches [TAG_NAME: or [TAG_NAME that has not closed.
  // QUERY_APP_STATE precedes QUERY to avoid prefix shadowing.
  const partialTagMatch = workingText.match(
    /\[(QUERY_APP_STATE|QUERY|EVENT_APPROVE|RUN_APPROVE|RUN|ASK_CHOICE|SUGGEST|DIAGNOSE|SECURITY_[A-Z_]+|SCHEDULE_[A-Z_]+|NAVIGATE|READ_REFERENCE|LEARN_REASONING|LEARN|REMEMBER_PREFERENCE)(?::|\]|\s|$|[^\n\]]*$)/i
  );

  if (!partialTagMatch || partialTagMatch.index === undefined) {
    return { cleanText: workingText, pendingTagKind: null };
  }

  // Verify that this tag is indeed unclosed (no matching ']' closing this tag)
  const tagStart = partialTagMatch.index;
  const tail = workingText.slice(tagStart);
  if (isTagClosed(tail)) {
    // If it has a closing bracket, it was closed or handled
    return { cleanText: workingText, pendingTagKind: null };
  }

  const rawTagName = partialTagMatch[1].toUpperCase();
  let pendingTagKind: StreamingTagKind;

  if (rawTagName === "QUERY") pendingTagKind = "query";
  else if (rawTagName === "EVENT_APPROVE") pendingTagKind = "event_approve";
  else if (rawTagName === "RUN" || rawTagName === "RUN_APPROVE") pendingTagKind = "run";
  else if (rawTagName === "ASK_CHOICE") pendingTagKind = "choice";
  else if (rawTagName === "SUGGEST") pendingTagKind = "suggest";
  else if (rawTagName === "DIAGNOSE" || rawTagName === "QUERY_APP_STATE") pendingTagKind = "diagnose";
  else if (rawTagName.startsWith("SECURITY_")) pendingTagKind = "security";
  else if (rawTagName.startsWith("SCHEDULE_")) pendingTagKind = "schedule";
  else if (rawTagName === "NAVIGATE" || rawTagName === "READ_REFERENCE") pendingTagKind = "navigate";
  else pendingTagKind = "memory";

  const cleanText = workingText.slice(0, tagStart).trimEnd();
  return { cleanText, pendingTagKind };
}

/**
 * Rebuilds assistant turn content for database persistence, preserving [SUGGEST]
 * and [ASK_CHOICE] tags while stripping execution tags ([QUERY], [RUN], etc.).
 */
export function rebuildAssistantTurnContent(
  cleanText: string,
  suggests?: string[][],
  rawChoices?: string[] | ParsedChoicePrompt | null
): string {
  // Strip any accidental execution tags from prose
  const parsed = parseAiTools(cleanText || "");
  let out = parsed.cleanText.trim();

  // 1. Append [ASK_CHOICE] if present
  if (Array.isArray(rawChoices) && rawChoices.length > 0) {
    for (const choice of rawChoices) {
      if (choice.trim()) {
        out = out ? `${out}\n\n${choice.trim()}` : choice.trim();
      }
    }
  } else if (rawChoices && typeof rawChoices === "object" && "question" in rawChoices) {
    const choiceTag = `[ASK_CHOICE: ${JSON.stringify(rawChoices)}]`;
    out = out ? `${out}\n\n${choiceTag}` : choiceTag;
  }

  // 2. Append [SUGGEST] chips if present
  if (suggests && suggests.length > 0) {
    const chipTags: string[] = [];
    for (const item of suggests) {
      // Handles both [fullMatch, label, prompt] and [label, prompt]
      const label = (item.length >= 3 ? item[1] : item[0])?.trim();
      const prompt = (item.length >= 3 ? item[2] : item[1])?.trim();

      if (!label) continue;
      if (prompt && prompt !== label) {
        chipTags.push(`[SUGGEST: ${label} | ${prompt}]`);
      } else {
        chipTags.push(`[SUGGEST: ${label}]`);
      }
    }

    if (chipTags.length > 0) {
      const joinedChips = chipTags.join("\n");
      out = out ? `${out}\n\n${joinedChips}` : joinedChips;
    }
  }

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

  // Choice prompt. Not a tool: nothing runs until user selects an option.
  const choiceTags = extractColonTags(responseText, "ASK_CHOICE", {
    caseInsensitive: true,
  });
  let choicePrompt: ParsedChoicePrompt | null = null;
  if (choiceTags.length > 0) {
    for (const item of choiceTags) {
      spans.push({ start: item.start, end: item.end });
    }
    const extracted = extractChoicePrompt(responseText);
    choicePrompt = extracted.choicePrompt;
  }

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
    choicePrompt,
  };
}
