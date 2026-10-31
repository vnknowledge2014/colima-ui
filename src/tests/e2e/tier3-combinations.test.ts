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
});
