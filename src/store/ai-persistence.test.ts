import { describe, it, expect, beforeEach, vi } from "vitest";

/**
 * An assistant turn is created empty and filled by the stream. It used to be
 * written to the history at creation and never rewritten, so every reply came
 * back blank after a reload — the thread showed a row of bare "Colima AI"
 * labels. These tests pin down when a message reaches the database.
 */

const saved: { id: string; content: string }[] = [];
let history: { id: string; role: string; content: string }[] = [];

vi.mock("../lib/api", () => ({
  aiApi: {
    saveMessage: async (message: { id: string; content: string }) => {
      saved.push({ id: message.id, content: message.content });
    },
    loadHistory: async () => history,
    listConversations: async () => [],
  },
}));

vi.mock("../lib/settingsStore.svelte", () => ({
  getAppSetting: (_key: string, fallback = "") => fallback,
  setAppSetting: () => {},
}));

const { aiState, pushAiMessage, updateAiMessage, deleteAiMessage, persistAiMessages, switchConversation } =
  await import("./ai.svelte");

beforeEach(() => {
  saved.length = 0;
  history = [];
  aiState.messages = [];
});

describe("AI chat persistence", () => {
  it("does not write the empty placeholder an assistant turn starts as", async () => {
    await pushAiMessage({ id: "a1", role: "assistant", content: "" });

    expect(saved).toEqual([]);
    // It is still on screen — only the database is spared.
    expect(aiState.messages).toHaveLength(1);
  });

  it("writes the reply once the turn settles", async () => {
    await pushAiMessage({ id: "a1", role: "assistant", content: "" });
    updateAiMessage("a1", "Colima is running.");
    await persistAiMessages(["a1"]);

    expect(saved).toEqual([{ id: "a1", content: "Colima is running." }]);
  });

  it("writes a message that already has text straight away", async () => {
    await pushAiMessage({ id: "u1", role: "user", content: "status?" });

    expect(saved).toEqual([{ id: "u1", content: "status?" }]);
  });

  it("skips a turn that never produced text", async () => {
    await pushAiMessage({ id: "a1", role: "assistant", content: "" });
    await persistAiMessages(["a1"]);

    expect(saved).toEqual([]);
  });

  it("does not write a turn that was discarded mid-flight", async () => {
    await pushAiMessage({ id: "a1", role: "assistant", content: "" });
    updateAiMessage("a1", "partial");
    deleteAiMessage("a1");
    await persistAiMessages([]);

    expect(saved).toEqual([]);
    expect(aiState.messages).toHaveLength(0);
  });

  it("hides blank rows left in threads saved before the fix", async () => {
    history = [
      { id: "u1", role: "user", content: "hello" },
      { id: "a1", role: "assistant", content: "" },
      { id: "a2", role: "assistant", content: "" },
    ];

    await switchConversation("default");

    expect(aiState.messages.map((m) => m.id)).toEqual(["u1"]);
  });
});
