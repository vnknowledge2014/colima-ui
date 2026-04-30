import { atom } from "jotai";
import type { SearchResult } from "../lib/api";

export interface DiagMessage {
  id: number;
  role: "user" | "assistant" | "system";
  content: string;
  searchResults?: SearchResult[];
  fetchedUrls?: string[];
  timestamp: number;
}

/** Chat messages for the diagnostics bubble */
export const aiMessagesAtom = atom<DiagMessage[]>([]);

/** Whether the bubble chat panel is open */
export const aiBubbleOpenAtom = atom(false);

/** Whether the agent is currently processing */
export const aiProcessingAtom = atom(false);

/** Count of unread errors since last bubble open */
export const aiErrorCountAtom = atom(0);
