export type ActionCategory = "SAFE" | "NORMAL" | "DANGEROUS";

export interface EventHandler {
  category: ActionCategory;
  description: string;
  // Event payloads are arbitrary JSON produced by the LLM tool parser; there
  // is no shared shape to type against.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- dynamic LLM event payload
  handler: (payload: any) => Promise<string>;
}
