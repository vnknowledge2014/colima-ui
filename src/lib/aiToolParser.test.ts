import { describe, it, expect } from "vitest";
import {
  parseAiTools,
  extractChoicePrompt,
  stripStreamingPartialTags,
  rebuildAssistantTurnContent,
} from "./aiToolParser";

describe("aiToolParser", () => {
  it("should parse [QUERY: ... ] tags correctly", () => {
    const input =
      "Here is the data: [QUERY: getContainers | show all] and another [QUERY: getImages]";
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(true);
    expect(parsed.queries.length).toBe(2);
    expect(parsed.queries[0][1]).toBe("getContainers ");
    expect(parsed.queries[0][2]).toBe("show all");
    expect(parsed.queries[1][1]).toBe("getImages");
    expect(parsed.queries[1][2]).toBeUndefined();

    expect(parsed.cleanText).toBe("Here is the data:  and another");
  });

  it("should parse [EVENT_APPROVE: ... ] tags correctly", () => {
    const input = "[EVENT_APPROVE: compose-build | my-project] Please build it.";
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(true);
    expect(parsed.eventApprovals.length).toBe(1);
    expect(parsed.eventApprovals[0][1]).toBe("compose-build ");
    expect(parsed.eventApprovals[0][2]).toBe("my-project");
    expect(parsed.cleanText).toBe("Please build it.");
  });

  it("should parse [NAVIGATE: ... ] tags correctly", () => {
    const input = "Sure, let me take you there: [NAVIGATE: compose]";
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(true);
    expect(parsed.navigates.length).toBe(1);
    expect(parsed.navigates[0][1]).toBe("compose");
    expect(parsed.cleanText).toBe("Sure, let me take you there:");
  });

  it("should correctly handle absence of tools", () => {
    const input = "Just a normal chat response without any tools.";
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(false);
    expect(parsed.queries.length).toBe(0);
    expect(parsed.cleanText).toBe(input);
  });

  it("should parse simple parameterless commands like [DIAGNOSE]", () => {
    const input = "Let me check: [DIAGNOSE]";
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(true);
    expect(parsed.hasDiagnose).toBe(true);
    expect(parsed.cleanText).toBe("Let me check:");
  });

  it("should not swallow adjacent tags when a parameterless tag precedes a piped tag", () => {
    const input =
      "First [QUERY: getImages] and second [QUERY: getContainers | show all]";
    const parsed = parseAiTools(input);

    expect(parsed.queries.length).toBe(2);
    expect(parsed.queries[0][1]).toBe("getImages");
    expect(parsed.queries[0][2]).toBeUndefined();
    expect(parsed.queries[1][1]).toBe("getContainers ");
    expect(parsed.queries[1][2]).toBe("show all");
    expect(parsed.cleanText).toBe("First  and second");
  });

  it("should preserve nested JSON arrays inside [EVENT_APPROVE] and [QUERY] payloads without truncating at inner ]", () => {
    const input =
      'Building image:\n[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["build", "-t", "myapp", "."]}]\n[EVENT_APPROVE: run-container | {"image": "nginx:latest", "ports": ["8080:80"]}]';
    const parsed = parseAiTools(input);

    expect(parsed.eventApprovals.length).toBe(2);
    expect(parsed.eventApprovals[0][1].trim()).toBe("cli-exec");
    expect(JSON.parse(parsed.eventApprovals[0][2])).toEqual({
      command: "docker",
      args: ["build", "-t", "myapp", "."],
    });
    expect(parsed.eventApprovals[1][1].trim()).toBe("run-container");
    expect(JSON.parse(parsed.eventApprovals[1][2])).toEqual({
      image: "nginx:latest",
      ports: ["8080:80"],
    });
    expect(parsed.cleanText).toBe("Building image:");
  });
});

describe("[SUGGEST:] follow-up chips", () => {
  it("parses a label with and without its prompt", () => {
    const input =
      "That failed. [SUGGEST: Show the logs | Show me the last 100 lines][SUGGEST: Restart it]";
    const parsed = parseAiTools(input);

    expect(parsed.suggests.length).toBe(2);
    expect(parsed.suggests[0][1]).toBe("Show the logs");
    expect(parsed.suggests[0][2]).toBe("Show me the last 100 lines");
    expect(parsed.suggests[1][1]).toBe("Restart it");
    // No prompt given — the panel falls back to the label.
    expect(parsed.suggests[1][2]).toBeUndefined();
    expect(parsed.cleanText).toBe("That failed.");
  });

  it("does not count as a tool, so a suggest-only reply is final", () => {
    // agentCore treats "no tools" as the end of the agent loop. A suggestion is
    // a finished answer, so counting it would spend another round.
    const parsed = parseAiTools("I could not identify it. [SUGGEST: Try again]");

    expect(parsed.hasTools).toBe(false);
    expect(parsed.suggests.length).toBe(1);
  });

  it("still reports tools when a reply mixes suggestions with a real one", () => {
    const parsed = parseAiTools("[QUERY: list-containers][SUGGEST: Restart it]");

    expect(parsed.hasTools).toBe(true);
    expect(parsed.suggests.length).toBe(1);
  });

  it("caps a chatty model at three chips", () => {
    const parsed = parseAiTools(
      "[SUGGEST: One][SUGGEST: Two][SUGGEST: Three][SUGGEST: Four][SUGGEST: Five]",
    );

    expect(parsed.suggests.length).toBe(3);
    // Dropped chips are still stripped from the visible text.
    expect(parsed.cleanText).toBe("");
  });

  it("leaves the other tags untouched", () => {
    const parsed = parseAiTools(
      "[NAVIGATE: compose][SUGGEST: Go back | Take me to the dashboard]",
    );

    expect(parsed.navigates.length).toBe(1);
    expect(parsed.navigates[0][1]).toBe("compose");
    expect(parsed.suggests[0][2]).toBe("Take me to the dashboard");
  });
});

describe("[ASK_CHOICE:] interactive choice card parsing", () => {
  it("parses structured JSON choice prompt with single selection", () => {
    const input = `I can help optimize your configuration.
[ASK_CHOICE: {
  "question": "Which VM profile do you want to configure?",
  "multi": false,
  "options": [
    { "label": "Default", "description": "2 CPU, 4GB RAM", "value": "default" },
    { "label": "Heavy", "description": "8 CPU, 16GB RAM", "value": "heavy" }
  ]
}]`;

    const { cleanText, choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt).not.toBeNull();
    expect(choicePrompt?.question).toBe("Which VM profile do you want to configure?");
    expect(choicePrompt?.multi).toBe(false);
    expect(choicePrompt?.options.length).toBe(2);
    expect(choicePrompt?.options[0].label).toBe("Default");
    expect(choicePrompt?.options[0].description).toBe("2 CPU, 4GB RAM");
    expect(choicePrompt?.options[0].value).toBe("default");
    expect(cleanText).toBe("I can help optimize your configuration.");
  });

  it("parses structured JSON choice prompt with string array options", () => {
    const input =
      '[ASK_CHOICE: {"question": "Choose instance", "options": ["colima", "docker", "k8s"]}]';
    const { choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt?.question).toBe("Choose instance");
    expect(choicePrompt?.multi).toBe(false);
    expect(choicePrompt?.options.length).toBe(3);
    expect(choicePrompt?.options[0]).toEqual({ label: "colima", value: "colima" });
  });

  it("parses pipe-delimited format with [multi] prefix", () => {
    const input =
      "Please select networks to remove:\n[ASK_CHOICE: [multi] Select networks | bridge | host | custom-net]";
    const { cleanText, choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt).not.toBeNull();
    expect(choicePrompt?.question).toBe("Select networks");
    expect(choicePrompt?.multi).toBe(true);
    expect(choicePrompt?.options.length).toBe(3);
    expect(choicePrompt?.options[0].label).toBe("bridge");
    expect(cleanText).toBe("Please select networks to remove:");
  });

  it("parses pipe-delimited format with option descriptions", () => {
    const input =
      "[ASK_CHOICE: Action to perform | Boot : Start the VM | Terminate : Stop and delete]";
    const { choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt?.options[0]).toEqual({
      label: "Boot",
      description: "Start the VM",
      value: "Boot",
    });
    expect(choicePrompt?.options[1]).toEqual({
      label: "Terminate",
      description: "Stop and delete",
      value: "Terminate",
    });
  });

  it("integrates into parseAiTools without flagging hasTools = true", () => {
    const input = "I am ready.\n[ASK_CHOICE: Which profile? | Dev | Prod]";
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(false); // Choice cards do not trigger backend tool round
    expect(parsed.choicePrompt).not.toBeNull();
    expect(parsed.choicePrompt?.question).toBe("Which profile?");
    expect(parsed.cleanText).toBe("I am ready.");
  });

  it("rejects pipe-delimited format with empty or whitespace question", () => {
    const inputEmpty = "[ASK_CHOICE: | Opt1 | Opt2]";
    const resEmpty = extractChoicePrompt(inputEmpty);
    expect(resEmpty.choicePrompt).toBeNull();
    expect(resEmpty.cleanText).toBe("");

    const inputWhitespace = "[ASK_CHOICE:    | Opt1 | Opt2]";
    const resWhitespace = extractChoicePrompt(inputWhitespace);
    expect(resWhitespace.choicePrompt).toBeNull();
    expect(resWhitespace.cleanText).toBe("");
  });

  it("rejects pipe-delimited format when question has only multi prefix with no question text", () => {
    const inputBracket = "[ASK_CHOICE: [multi] | Opt1 | Opt2]";
    expect(extractChoicePrompt(inputBracket).choicePrompt).toBeNull();

    const inputParen = "[ASK_CHOICE: (multi)    | Opt1 | Opt2]";
    expect(extractChoicePrompt(inputParen).choicePrompt).toBeNull();
  });

  it("parses pipe-delimited format with standard question and options", () => {
    const input = "[ASK_CHOICE: Which action? | Inspect | Restart]";
    const { cleanText, choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt).not.toBeNull();
    expect(choicePrompt?.question).toBe("Which action?");
    expect(choicePrompt?.multi).toBe(false);
    expect(choicePrompt?.options).toEqual([
      { label: "Inspect", value: "Inspect" },
      { label: "Restart", value: "Restart" },
    ]);
    expect(cleanText).toBe("");
  });

  it("parses pipe-delimited format with (multi) prefix and valid question", () => {
    const input = "[ASK_CHOICE: (multi) Select features | Logs | Shell | Metrics]";
    const { choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt).not.toBeNull();
    expect(choicePrompt?.question).toBe("Select features");
    expect(choicePrompt?.multi).toBe(true);
    expect(choicePrompt?.options).toHaveLength(3);
    expect(choicePrompt?.options.map((o) => o.label)).toEqual(["Logs", "Shell", "Metrics"]);
  });

  it("handles empty options between pipes without producing blank options", () => {
    const input = "[ASK_CHOICE: Question | Opt1 | | Opt2 | ]";
    const { choicePrompt } = extractChoicePrompt(input);

    expect(choicePrompt).not.toBeNull();
    expect(choicePrompt?.question).toBe("Question");
    expect(choicePrompt?.options).toEqual([
      { label: "Opt1", value: "Opt1" },
      { label: "Opt2", value: "Opt2" },
    ]);
  });
});

describe("stripStreamingPartialTags", () => {
  it("suppresses unclosed [QUERY: at tail of stream", () => {
    const input = 'Reading instances: [QUERY: list-instances | {"filter":';
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe("Reading instances:");
    expect(pendingTagKind).toBe("query");
  });

  it("suppresses unclosed [SUGGEST: at tail of stream", () => {
    const input = "Operation finished. [SUGGEST: View logs | show";
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe("Operation finished.");
    expect(pendingTagKind).toBe("suggest");
  });

  it("suppresses unclosed [ASK_CHOICE: at tail of stream", () => {
    const input = 'Select option: [ASK_CHOICE: {"question":';
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe("Select option:");
    expect(pendingTagKind).toBe("choice");
  });

  it("suppresses unclosed [DIAGNOSE at tail of stream", () => {
    const input = "Running diagnosis: [DIAGNOSE";
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe("Running diagnosis:");
    expect(pendingTagKind).toBe("diagnose");
  });

  it("preserves standard markdown links intact without stripping", () => {
    const input =
      "Check the [Colima Docs](https://github.com/abiosoft/colima) for setup details.";
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe(input);
    expect(pendingTagKind).toBeNull();
  });

  it("preserves bracketed arrays and notes in prose", () => {
    const input = "Configuration flags: [1, 2, 3] and [NOTE: important info].";
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe(input);
    expect(pendingTagKind).toBeNull();
  });

  it("returns null pendingTagKind when stream has no pending tags", () => {
    const input = "Plain message without any tags.";
    const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);

    expect(cleanText).toBe(input);
    expect(pendingTagKind).toBeNull();
  });
});

describe("rebuildAssistantTurnContent", () => {
  it("combines clean prose with suggestions", () => {
    const prose = "VM has been stopped.";
    const suggests = [["Restart", "colima start"], ["View Logs"]];
    const rebuilt = rebuildAssistantTurnContent(prose, suggests);

    expect(rebuilt).toContain("VM has been stopped.");
    expect(rebuilt).toContain("[SUGGEST: Restart | colima start]");
    expect(rebuilt).toContain("[SUGGEST: View Logs]");
  });

  it("preserves choice tags alongside prose and suggestions", () => {
    const prose = "Multiple VMs found.";
    const choices = ["[ASK_CHOICE: Pick one | colima | lima-vm]"];
    const suggests = [["Cancel"]];
    const rebuilt = rebuildAssistantTurnContent(prose, suggests, choices);

    expect(rebuilt).toContain("Multiple VMs found.");
    expect(rebuilt).toContain("[ASK_CHOICE: Pick one | colima | lima-vm]");
    expect(rebuilt).toContain("[SUGGEST: Cancel]");
  });

  it("handles suggestion-only replies without dropping content", () => {
    const prose = "";
    const suggests = [["Try again"]];
    const rebuilt = rebuildAssistantTurnContent(prose, suggests);

    expect(rebuilt).toBe("[SUGGEST: Try again]");
  });

  it("strips leftover execution tags from prose", () => {
    const prose = "Execution output: [QUERY: list-containers] done.";
    const rebuilt = rebuildAssistantTurnContent(prose, [["Next"]]);

    expect(rebuilt).not.toContain("[QUERY:");
    expect(rebuilt).toContain("Execution output:  done.");
    expect(rebuilt).toContain("[SUGGEST: Next]");
  });
});
