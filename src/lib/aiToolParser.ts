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

export function parseAiTools(responseText: string): ParsedTools {
  const queries = [...responseText.matchAll(/\[QUERY:\s*([^|]+)\s*(?:\|\s*(.+?))?\]/gi)];
  const eventApprovals = [...responseText.matchAll(/\[EVENT_APPROVE:\s*([^|]+)\s*(?:\|\s*(.+?))?\]/gi)];
  
  const runs = [...responseText.matchAll(/\[RUN:\s*(.+?)\]/g)];
  const runApprovals = [...responseText.matchAll(/\[RUN_APPROVE:\s*(.+?)\]/g)];
  const hasDiagnose = /\[DIAGNOSE\]/.test(responseText);
  const hasQueryState = /\[QUERY_APP_STATE\]/.test(responseText);
  const learns = [...responseText.matchAll(/\[LEARN:\s*(.+?)\]/g)];
  const learnReasoning = [...responseText.matchAll(/\[LEARN_REASONING:\s*(.+?)\]/g)];
  const prefs = [...responseText.matchAll(/\[REMEMBER_PREFERENCE:\s*(.+?)\]/g)];
  const navigates = [...responseText.matchAll(/\[NAVIGATE:\s*(.+?)\]/gi)];
  const readReferences = [...responseText.matchAll(/\[READ_REFERENCE:\s*(.+?)\]/gi)];

  const secThreatModels = [...responseText.matchAll(/\[SECURITY_THREAT_MODEL:\s*([^|]+)\s*(?:\|\s*(.+?))?\]/gi)];
  const secVulnScans = [...responseText.matchAll(/\[SECURITY_VULN_SCAN:\s*(.+?)\]/gi)];
  const secTriages = [...responseText.matchAll(/\[SECURITY_TRIAGE:\s*(.+?)\]/gi)];
  const secPatchGens = [...responseText.matchAll(/\[SECURITY_PATCH_GEN:\s*([^|]+)\s*(?:\|\s*(.+?))?\]/gi)];

  const schedCrons = [...responseText.matchAll(/\[SCHEDULE_CRON:\s*([^|]+)\s*(?:\|\s*(.+?))?\]/gi)];
  const schedTimers = [...responseText.matchAll(/\[SCHEDULE_TIMER:\s*(\d+)\s*(?:\|\s*(.+?))?\]/gi)];
  const schedCancels = [...responseText.matchAll(/\[SCHEDULE_CANCEL:\s*(.+?)\]/gi)];

  // Follow-ups the user can click. Not a tool: nothing runs until they do.
  const suggests = [...responseText.matchAll(/\[SUGGEST:\s*([^|\]]+?)\s*(?:\|\s*(.+?))?\]/gi)];

  const cleanText = responseText
    .replace(/\[QUERY:\s*[^|]+\s*(?:\|.+?)?\]/gi, "")
    .replace(/\[EVENT_APPROVE:\s*[^|]+\s*(?:\|.+?)?\]/gi, "")
    .replace(/\[DIAGNOSE\]/g, "")
    .replace(/\[QUERY_APP_STATE\]/g, "")
    .replace(/\[RUN:\s*.+?\]/g, "")
    .replace(/\[RUN_APPROVE:\s*.+?\]/g, "")
    .replace(/\[LEARN:\s*.+?\]/g, "")
    .replace(/\[LEARN_REASONING:\s*.+?\]/g, "")
    .replace(/\[REMEMBER_PREFERENCE:\s*.+?\]/g, "")
    .replace(/\[NAVIGATE:\s*.+?\]/gi, "")
    .replace(/\[READ_REFERENCE:\s*.+?\]/gi, "")
    .replace(/\[SECURITY_THREAT_MODEL:\s*[^|]+\s*(?:\|.+?)?\]/gi, "")
    .replace(/\[SECURITY_VULN_SCAN:\s*.+?\]/gi, "")
    .replace(/\[SECURITY_TRIAGE:\s*.+?\]/gi, "")
    .replace(/\[SECURITY_PATCH_GEN:\s*[^|]+\s*(?:\|.+?)?\]/gi, "")
    .replace(/\[SCHEDULE_CRON:\s*[^|]+\s*(?:\|.+?)?\]/gi, "")
    .replace(/\[SCHEDULE_TIMER:\s*\d+\s*(?:\|.+?)?\]/gi, "")
    .replace(/\[SCHEDULE_CANCEL:\s*.+?\]/gi, "")
    .replace(/\[SUGGEST:\s*[^|\]]+?\s*(?:\|.+?)?\]/gi, "")
    .trim();

  const hasTools = queries.length > 0 || eventApprovals.length > 0 || hasDiagnose || hasQueryState || 
                   runs.length > 0 || runApprovals.length > 0 || learns.length > 0 || learnReasoning.length > 0 || 
                   prefs.length > 0 || navigates.length > 0 || readReferences.length > 0 || secThreatModels.length > 0 || 
                   secVulnScans.length > 0 || secTriages.length > 0 || secPatchGens.length > 0 || 
                   schedCrons.length > 0 || schedTimers.length > 0 || schedCancels.length > 0;

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
    suggests: suggests.slice(0, MAX_SUGGESTIONS)
  };
}
