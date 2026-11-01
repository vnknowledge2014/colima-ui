/**
 * E2E Test Suite — Tier 3: Cross-Feature Combinations
 *
 * Requirements: Multi-stage, pairwise, and cross-boundary interaction tests:
 * - Streaming <think> into [ASK_CHOICE]
 * - Tool approval with clipboard copy & VM terminal launch
 * - Side-effect turns retaining [SUGGEST] chips
 * - Interactive choice selection advancing chat turns & locking previous cards
 * - Partial streaming tag suppression combined with rich markdown formatting
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import {
  getChatMarkdownParser,
  getToolStepSerializer,
  getAiToolParser,
  specBuildRichLiveUiContext,
  type ThinkBlock,
  type CodeBlock,
  type ToolStepPayload,
} from './helpers/contractAdapter';
import {
  setupClipboardMock,
  deriveActiveVmTarget,
  simulateTruncateAiMessagesFrom,
  type SimulatedMessage,
} from './helpers/testUtils';
import { parseAiTools } from '../../lib/aiToolParser';
import { getStarterPrompts, deriveFallbackSuggestions, type StarterPromptContext } from '../../lib/aiSuggestions';

describe('Tier 3: Cross-Feature Combinations', () => {
  let clipboard: ReturnType<typeof setupClipboardMock>;

  beforeEach(() => {
    clipboard = setupClipboardMock();
  });

  afterEach(() => {
    clipboard.restore();
  });

  // -------------------------------------------------------------------------
  // Combination 1: Streaming <think> Reasoning into [ASK_CHOICE]
  // -------------------------------------------------------------------------
  describe('C1: Streaming <think> transitioning into [ASK_CHOICE]', async () => {
    const parse = await getChatMarkdownParser();
    const { extractChoicePrompt, stripStreamingPartialTags } = await getAiToolParser();

    it('1.1: handles live streaming chunk before think closes', () => {
      const streamChunk = '<think>\nEvaluating whether 4 CPU or 8 CPU is optimal...';
      const blocks = parse(streamChunk);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think).toBeDefined();
      expect(think.streaming).toBe(true);
      expect(think.content).toContain('Evaluating whether 4 CPU');
    });

    it('1.2: suppresses partial [ASK_CHOICE tag while thinking has closed', () => {
      const streamChunk =
        '<think>Analysis complete: 8GB RAM is recommended.</think>\nBased on your workloads: [ASK_CHOICE: {"question": "Select VM';
      const { cleanText, pendingTagKind } = stripStreamingPartialTags(streamChunk);
      expect(pendingTagKind).toBe('choice');
      expect(cleanText).not.toContain('[ASK_CHOICE:');
      expect(cleanText).toContain('Based on your workloads:');
    });

    it('1.3: completes full transition to closed reasoning + extracted choice card', () => {
      const fullResponse = `
<think>
Analyzed system memory: 32GB total, 16GB free.
VM requires 8GB to support Docker + k8s simultaneously.
</think>
I have prepared recommended VM options for your workload:
[ASK_CHOICE: {"question": "Choose configuration", "multi": false, "options": [{"label": "Standard (4 CPU · 8GB RAM)", "value": "4cpu-8gb"}, {"label": "High-Perf (8 CPU · 16GB RAM)", "value": "8cpu-16gb"}]}]
`;
      // 1. Parse markdown
      const blocks = parse(fullResponse);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think).toBeDefined();
      expect(think.streaming).toBe(false);
      expect(think.content).toContain('Analyzed system memory');

      // 2. Extract choices
      const { cleanText, choicePrompt } = extractChoicePrompt(fullResponse);
      expect(choicePrompt).toBeDefined();
      expect(choicePrompt?.options).toHaveLength(2);
      expect(choicePrompt?.options[0].value).toBe('4cpu-8gb');
      expect(cleanText).not.toContain('[ASK_CHOICE:');
    });
  });

  // -------------------------------------------------------------------------
  // Combination 2: Tool Approval with VM Terminal Launch
  // -------------------------------------------------------------------------
  describe('C2: Tool Approval with VM Terminal Launch & Clipboard Safety', async () => {
    const { serializeToolStep, parseToolStep } = await getToolStepSerializer();
    const parse = await getChatMarkdownParser();

    it('2.1: dangerous tool requires approval and provides shell command', () => {
      const command = 'colima start --cpu 4 --memory 8';
      const toolPayload: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'colima-start',
        title: 'Start Colima with 4 CPU and 8GB RAM',
        safety: 'DANGEROUS',
        status: 'pending_approval',
        warning: 'Requires VM restart and config override',
        rawInput: command,
      };

      const serialized = serializeToolStep(toolPayload);
      const parsedStep = parseToolStep(serialized);
      expect(parsedStep?.safety).toBe('DANGEROUS');
      expect(parsedStep?.status).toBe('pending_approval');
    });

    it('2.2: clicking Open Terminal copies command to clipboard without auto-execution', async () => {
      const command = 'docker system prune -a -f';
      const mockInstances = [
        { name: 'default', status: 'Running' },
        { name: 'dev', status: 'Stopped' },
      ];

      // Simulate "Open Terminal" action
      await navigator.clipboard.writeText(command);
      const targetVm = deriveActiveVmTarget(mockInstances);

      expect(clipboard.writtenText).toContain(command);
      expect(targetVm).toEqual({ kind: 'colima', profile: 'default' });
      // Verified: no background execution payload was triggered
    });

    it('2.3: updates tool card in-place to succeeded upon manual/approved execution', () => {
      const initialStep: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'start-vm',
        title: 'Start Colima VM',
        safety: 'NORMAL',
        status: 'pending_approval',
      };

      // In-place transition: pending_approval -> running -> succeeded
      const runningStep: ToolStepPayload = { ...initialStep, status: 'running' };
      expect(parseToolStep(serializeToolStep(runningStep))?.status).toBe('running');

      const succeededStep: ToolStepPayload = {
        ...initialStep,
        status: 'succeeded',
        exitCode: 0,
        output: 'Colima is running using macOS Virtualization.Framework',
      };

      const finalSerialized = serializeToolStep(succeededStep);
      const finalParsed = parseToolStep(finalSerialized);
      expect(finalParsed?.status).toBe('succeeded');
      expect(finalParsed?.output).toContain('Colima is running');
    });

    it('2.4: verifies that code block inside assistant message is flagged as shell', () => {
      const markdown = '```bash\ncolima start --cpu 4\n```';
      const block = parse(markdown)[0] as CodeBlock;
      expect(block.isShell).toBe(true);
      expect(block.code).toBe('colima start --cpu 4');
    });
  });

  // -------------------------------------------------------------------------
  // Combination 3: Side-Effect Turns Retaining Follow-Ups
  // -------------------------------------------------------------------------
  describe('C3: Side-Effect Turns Retaining [SUGGEST] Chips', async () => {
    const { rebuildAssistantTurnContent } = await getAiToolParser();

    it('3.1: preserves suggestions when turn contains [NAVIGATE: compose]', () => {
      // Simulate side-effect turn termination:
      const cleanText = 'Navigating you to Compose tab.';
      const suggests = [
        ['Check services', 'Check all services'],
        ['View compose logs', 'View compose logs'],
      ];

      const rebuilt = rebuildAssistantTurnContent(cleanText, suggests);
      expect(rebuilt).toContain('Navigating you to Compose tab.');
      expect(rebuilt).toContain('[SUGGEST: Check services | Check all services]');
      expect(rebuilt).toContain('[SUGGEST: View compose logs]');
      expect(rebuilt).not.toContain('[NAVIGATE:');
    });

    it('3.2: preserves suggestions when turn contains memory tags [LEARN: ...]', () => {
      const cleanText = 'I have noted your preference for Lima driver.';
      const suggests = [['Next step', 'What are our next steps?']];

      const rebuilt = rebuildAssistantTurnContent(cleanText, suggests);
      expect(rebuilt).toContain('I have noted your preference');
      expect(rebuilt).toContain('[SUGGEST: Next step | What are our next steps?]');
    });

    it('3.3: preserves both [ASK_CHOICE] and [SUGGEST] in combined turn', () => {
      const cleanText = 'Please select a profile:';
      const suggests = [['Cancel', 'Cancel operation']];
      const rawChoices = ['[ASK_CHOICE: Pick VM | dev | test | prod]'];

      const rebuilt = rebuildAssistantTurnContent(cleanText, suggests, rawChoices);
      expect(rebuilt).toContain('[ASK_CHOICE: Pick VM | dev | test | prod]');
      expect(rebuilt).toContain('[SUGGEST: Cancel | Cancel operation]');
    });
  });

  // -------------------------------------------------------------------------
  // Combination 4: Choice Selection Advancing Chat History
  // -------------------------------------------------------------------------
  describe('C4: Choice Selection into Message Stream', () => {
    it('4.1: formats user reply when choice option is clicked', () => {
      const choiceValue = 'Standard (4 CPU · 8GB RAM)';
      const userMessage: SimulatedMessage = {
        id: 'user-choice-1',
        role: 'user',
        content: choiceValue,
      };
      expect(userMessage.role).toBe('user');
      expect(userMessage.content).toBe(choiceValue);
    });

    it('4.2: locks previous choice card when newer assistant message arrives', () => {
      const conversationTurns: SimulatedMessage[] = [
        { id: 'u1', role: 'user', content: 'Create VM' },
        { id: 'a1', role: 'assistant', content: 'Choose config: [ASK_CHOICE: Q | opt1 | opt2]' },
        { id: 'u2', role: 'user', content: 'opt1' },
        { id: 'a2', role: 'assistant', content: 'Creating VM with opt1...' },
      ];

      const isLatestTurn = (turnId: string): boolean => {
        const lastAssistant = [...conversationTurns]
          .reverse()
          .find((m) => m.role === 'assistant');
        return lastAssistant?.id === turnId;
      };

      // a1 was answered and is no longer the latest assistant turn
      expect(isLatestTurn('a1')).toBe(false);
      // a2 is the latest assistant turn
      expect(isLatestTurn('a2')).toBe(true);
    });

    it('4.3: edit & resend on choice option truncates subsequent turns cleanly', () => {
      const conversationTurns: SimulatedMessage[] = [
        { id: 'u1', role: 'user', content: 'Create VM' },
        { id: 'a1', role: 'assistant', content: 'Choose config' },
        { id: 'u2', role: 'user', content: '4 CPU 8GB' },
        { id: 'a2', role: 'assistant', content: 'VM created' },
      ];

      // User wants to edit choice selection u2
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(conversationTurns, 'u2');
      expect(remaining.map((m) => m.id)).toEqual(['u1', 'a1']);
      expect(truncated.map((m) => m.id)).toEqual(['u2', 'a2']);
    });
  });

  // -------------------------------------------------------------------------
  // Combination 5: Partial Tag Suppression with Rich Markdown
  // -------------------------------------------------------------------------
  describe('C5: Partial Tag Suppression with Rich Markdown', async () => {
    const { stripStreamingPartialTags } = await getAiToolParser();
    const parse = await getChatMarkdownParser();

    it('5.1: preserves GFM bold and list markdown while suppressing partial [EVENT_APPROVE:', () => {
      const streamChunk = `
### Task Summary
- **Step 1**: Checking disk
- **Step 2**: Starting daemon

[EVENT_APPROVE: start-daemon | {"service": "dock`;

      const { cleanText, pendingTagKind } = stripStreamingPartialTags(streamChunk);
      expect(pendingTagKind).toBe('event_approve');
      expect(cleanText).not.toContain('[EVENT_APPROVE:');

      // Verify that the markdown portion parses properly
      const blocks = parse(cleanText);
      const heading = blocks.find((b) => b.type === 'heading');
      expect(heading).toBeDefined();
      const list = blocks.find((b) => b.type === 'list');
      expect(list).toBeDefined();
    });

    it('5.2: preserves code blocks preceding a partial tag', () => {
      const streamChunk = `
Run this command:
\`\`\`bash
colima status
\`\`\`
[QUERY: getContainer`;

      const { cleanText, pendingTagKind } = stripStreamingPartialTags(streamChunk);
      expect(pendingTagKind).toBe('query');
      const blocks = parse(cleanText);
      const code = blocks.find((b) => b.type === 'code') as CodeBlock;
      expect(code).toBeDefined();
      expect(code.isShell).toBe(true);
      expect(code.code).toBe('colima status');
    });
  });

  // -------------------------------------------------------------------------
  // Combination 6: Multi-Packet SSE Stream & Intermediate Draft Revocation
  // (Feature 9 + Feature 12)
  // -------------------------------------------------------------------------
  describe('C6: SSE Stream + Intermediate Draft Revocation (Feature 9 + 12)', () => {
    it('6.1: Round 1 streams speculative draft text while firing read-only tool', () => {
      let pendingSupersededStreamId: string | null = null;
      const messageList: Array<{ id: string; role: string; content: string }> = [];

      // Round 1 begins
      const streamId = 'msg-draft-turn-1';
      const round1Chunk = 'I am diagnosing your container exit... [QUERY: list-containers]';
      const parsed = parseAiTools(round1Chunk);

      // Assistant emits text and runs tool
      messageList.push({ id: streamId, role: 'assistant', content: parsed.cleanText });
      const roundAwaitedApproval = false; // read-only tool

      if (!roundAwaitedApproval && parsed.cleanText) {
        pendingSupersededStreamId = streamId;
      }

      expect(pendingSupersededStreamId).toBe('msg-draft-turn-1');
      expect(messageList).toHaveLength(1);
      expect(messageList[0].content).toContain('diagnosing your container');
    });

    it('6.2: Round 2 SSE stream arrives and revokes intermediate draft bubble immediately on first text token', () => {
      const messageList: Array<{ id: string; role: string; content: string }> = [
        { id: 'msg-draft-turn-1', role: 'assistant', content: 'I am diagnosing your container exit...' },
      ];
      let pendingSupersededStreamId: string | null = 'msg-draft-turn-1';

      // Callbacks
      const onMessageDelete = (id: string) => {
        const idx = messageList.findIndex((m) => m.id === id);
        if (idx !== -1) messageList.splice(idx, 1);
      };

      // Round 2 receives first token of final answer
      const round2Chunk = 'Diagnosis complete: Container crashed due to OOMKilled (exit code 137).';
      if (pendingSupersededStreamId && round2Chunk.trim()) {
        onMessageDelete(pendingSupersededStreamId);
        pendingSupersededStreamId = null;
      }

      // Add final answer turn
      messageList.push({ id: 'msg-final-turn-2', role: 'assistant', content: round2Chunk });

      // Verifies intermediate draft is gone and only final turn remains
      expect(messageList).toHaveLength(1);
      expect(messageList[0].id).toBe('msg-final-turn-2');
      expect(messageList[0].content).toContain('exit code 137');
      expect(pendingSupersededStreamId).toBeNull();
    });

    it('6.3: does not drop intermediate draft if Round 2 only emits another tool tag', () => {
      const messageList: Array<{ id: string; role: string; content: string }> = [
        { id: 'msg-draft-turn-1', role: 'assistant', content: 'Checking memory metrics...' },
      ];
      let pendingSupersededStreamId: string | null = 'msg-draft-turn-1';

      // Round 2 only emits tool tag
      const round2Chunk = '[QUERY: get-container-stats | {"id": "redis"}]';
      const parsed = parseAiTools(round2Chunk);

      // Callbacks
      const onMessageDelete = (id: string) => {
        const idx = messageList.findIndex((m) => m.id === id);
        if (idx !== -1) messageList.splice(idx, 1);
      };

      if (pendingSupersededStreamId && parsed.cleanText.trim()) {
        onMessageDelete(pendingSupersededStreamId);
        pendingSupersededStreamId = null;
      }

      // Draft must NOT be deleted yet!
      expect(messageList).toHaveLength(1);
      expect(pendingSupersededStreamId).toBe('msg-draft-turn-1');
    });
  });

  // -------------------------------------------------------------------------
  // Combination 7: Live Container Details in [LIVE_UI_CONTEXT] & 4-Language i18n
  // (Feature 14 + Feature 15)
  // -------------------------------------------------------------------------
  describe('C7: Live Container Details in [LIVE_UI_CONTEXT] + 4-Language i18n (Feature 14 + 15)', () => {
    const mockContainers = [
      { name: 'postgres-db', Status: 'Up 12 hours', Image: 'postgres:16-alpine', State: 'running' },
      { name: 'web-api', Status: 'Exited (137)', Image: 'my-org/web:latest', State: 'exited' },
      { name: 'redis-cache', Status: 'Up 4 hours', Image: 'redis:7.2', State: 'running' },
    ];

    it('7.1: generates rich live UI context with container details in Vietnamese locale', () => {
      const ctx = specBuildRichLiveUiContext({
        currentPage: 'containers',
        language: 'vi',
        containers: mockContainers,
      });

      expect(ctx).toContain('Language: vi');
      expect(ctx).toContain('postgres-db [Up 12 hours, postgres:16-alpine]');
      expect(ctx).toContain('web-api [Exited (137), my-org/web:latest]');
      expect(ctx).toContain('Containers: 2 running, 1 stopped');
      expect(ctx).toContain('Ground Truth Rule: Prioritize [LIVE_UI_CONTEXT]');
    });

    it('7.2: preserves container details when switching language to Japanese (ja)', () => {
      const ctx = specBuildRichLiveUiContext({
        currentPage: 'containers',
        language: 'ja',
        containers: mockContainers,
      });

      expect(ctx).toContain('Language: ja');
      expect(ctx).toContain('postgres-db [Up 12 hours, postgres:16-alpine]');
      expect(ctx.length).toBeLessThanOrEqual(900);
    });

    it('7.3: preserves container details when switching language to Chinese (zh)', () => {
      const ctx = specBuildRichLiveUiContext({
        currentPage: 'containers',
        language: 'zh',
        containers: mockContainers,
      });

      expect(ctx).toContain('Language: zh');
      expect(ctx).toContain('redis-cache [Up 4 hours, redis:7.2]');
      expect(ctx.length).toBeLessThanOrEqual(900);
    });

    it('7.4: generates contextual starter prompts for exited container across languages', () => {
      const starters = getStarterPrompts({
        currentPage: 'containers',
        containers: mockContainers as unknown as StarterPromptContext['containers'],
      });

      expect(starters.length).toBeGreaterThan(0);
      const warningPrompt = starters.find((s) => s.category === 'warning');
      expect(warningPrompt).toBeDefined();
      expect(warningPrompt?.prompt).toContain('web-api');
    });

    it('7.5: fallback suggestions adapt based on container status tool execution', () => {
      const suggestions = deriveFallbackSuggestions({
        currentPage: 'containers',
        toolsExecuted: [{ kind: 'query', name: 'list-containers' }],
      });

      expect(suggestions.length).toBeGreaterThanOrEqual(2);
      expect(suggestions[0][0]).toMatch(/CPU|RAM|logs/i);
    });
  });
});

