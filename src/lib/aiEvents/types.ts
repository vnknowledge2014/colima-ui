import { isRunningInTauri } from "../env";
export type ActionCategory = "SAFE" | "NORMAL" | "DANGEROUS";

export interface EventHandler {
  category: ActionCategory;
  description: string;
  handler: (payload: any) => Promise<string>;
}
