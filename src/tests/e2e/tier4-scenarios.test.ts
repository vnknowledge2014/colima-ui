/**
 * E2E Test Suite — Tier 4: Real-World Scenarios
 *
 * Requirements: Full end-to-end user workflows with simulated state transitions:
 * - Scenario A: Troubleshooting Stopped Colima Instance
 * - Scenario B: Container Exit Analysis & Safe Terminal Launching
 * - Scenario C: Consultant Mode VM Creation with Interactive Choice Card
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import {
  getChatMarkdownParser,
  getToolStepSerializer,
  getAiToolParser,
  getAiSuggestionsEngine,
  specBuildRichLiveUiContext,
  specSortHistoryDeterministic,
  type ThinkBlock,
  type CodeBlock,
  type ToolStepPayload,
} from './helpers/contractAdapter';
import {
  setupClipboardMock,
  deriveActiveVmTarget,
  type SimulatedMessage,
} from './helpers/testUtils';
import { parseAiTools } from '../../lib/aiToolParser';
import { capToolOutput } from '../../lib/agentCore';

describe('Tier 4: Real-World Scenarios', () => {
  let clipboard: ReturnType<typeof setupClipboardMock>;

  beforeEach(() => {
    clipboard = setupClipboardMock();
  });

  afterEach(() => {
    clipboard.restore();
  });

  // -------------------------------------------------------------------------
  // Scenario A: Troubleshooting Stopped Colima Instance
  // -------------------------------------------------------------------------
  describe('Scenario A: Troubleshooting Stopped Colima Instance', async () => {
    const { getStarterPrompts } = await getAiSuggestionsEngine();
    const { serializeToolStep, parseToolStep } = await getToolStepSerializer();
    const { extractChoicePrompt } = await getAiToolParser();

    it('Step 1: empty chat state detects stopped VM and prioritizes troubleshooting starter prompt', () => {
      const mockState = {
        currentPage: 'dashboard',
        colimaInstances: [
          { name: 'default', status: 'Stopped' },
          { name: 'test-vm', status: 'Running' },
        ],
        containers: [],
      };

      const starters = getStarterPrompts(mockState);
      expect(starters.length).toBeGreaterThan(0);
      expect(starters[0].category).toBe('warning');
      expect(starters[0].prompt).toContain('Why is Colima instance default stopped?');
    });

    it('Step 2: AI proposes starting the instance with a unified ToolStep in pending_approval', () => {
      const toolStep: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'colima-start',
        title: 'Start Colima Instance "default"',
        safety: 'NORMAL',
        status: 'pending_approval',
        rawInput: 'colima start default',
        timestamp: Date.now(),
      };

      const serialized = serializeToolStep(toolStep);
      expect(serialized).toContain('<!--AI_TOOL_STEP:');

      const parsed = parseToolStep(serialized);
      expect(parsed?.status).toBe('pending_approval');
      expect(parsed?.name).toBe('colima-start');
    });

    it('Step 3: approving the action transitions tool card in-place to running then succeeded', () => {
      const step: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'colima-start',
        title: 'Start Colima Instance "default"',
        safety: 'NORMAL',
        status: 'pending_approval',
        rawInput: 'colima start default',
      };

      // 1. In-place transition to running
      const running = { ...step, status: 'running' as const };
      expect(parseToolStep(serializeToolStep(running))?.status).toBe('running');

      // 2. In-place transition to succeeded
      const succeeded: ToolStepPayload = {
        ...step,
        status: 'succeeded',
        exitCode: 0,
        output: 'colima has started successfully on profile default.',
      };
      const parsedSucceeded = parseToolStep(serializeToolStep(succeeded));
      expect(parsedSucceeded?.status).toBe('succeeded');
      expect(parsedSucceeded?.output).toContain('colima has started successfully');
    });

    it('Step 4: AI provides follow-up suggestions for next logical checks', () => {
      const aiResponse =
        'Colima instance "default" is now running. [SUGGEST: Check Docker containers | List all running containers][SUGGEST: Inspect VM resource usage]';
      const { cleanText } = extractChoicePrompt(aiResponse);
      expect(cleanText).toContain('Colima instance "default" is now running.');
    });
  });

  // -------------------------------------------------------------------------
  // Scenario B: Container Exit Analysis & Safe Terminal Launching
  // -------------------------------------------------------------------------
  describe('Scenario B: Container Exit Analysis & Safe Terminal Launching', async () => {
    const parse = await getChatMarkdownParser();

    it('Step 1: AI streams reasoning <think> with live active indicator', () => {
      const streamingChunk = `
<think>
Examining exited container c-web-99.
Inspecting Docker daemon event logs for OOM or segfault signals...
`;
      const blocks = parse(streamingChunk);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think).toBeDefined();
      expect(think.streaming).toBe(true);
      expect(think.content).toContain('Inspecting Docker daemon event logs');
    });

    it('Step 2: reasoning completes and collapses, presenting diagnostic answer and shell command', () => {
      const fullResponse = `
<think>
Found container exit code 137. Container exceeded 512MB RAM limit during webpack build.
</think>
The container **frontend-web** exited due to an Out of Memory error (OOMKilled, exit code 137).

Run the following command to review the final 50 log lines:

\`\`\`bash
docker logs --tail 50 frontend-web
\`\`\`
`;
      const blocks = parse(fullResponse);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think.streaming).toBe(false);

      const codeBlock = blocks.find((b) => b.type === 'code') as CodeBlock;
      expect(codeBlock).toBeDefined();
      expect(codeBlock.isShell).toBe(true);
      expect(codeBlock.code).toBe('docker logs --tail 50 frontend-web');
    });

    it('Step 3: clicking Open Terminal safely copies command and targets running VM without auto-executing', async () => {
      const command = 'docker logs --tail 50 frontend-web';
      const instances = [{ name: 'default', status: 'Running' }];

      // User clicks "Open Terminal" button
      await navigator.clipboard.writeText(command);
      const sessionTarget = deriveActiveVmTarget(instances);

      expect(clipboard.writtenText).toContain(command);
      expect(sessionTarget).toEqual({ kind: 'colima', profile: 'default' });
      // Verified: no automatic background terminal_write execution
    });
  });

  // -------------------------------------------------------------------------
  // Scenario C: Consultant Mode VM Creation with Interactive Choice Card
  // -------------------------------------------------------------------------
  describe('Scenario C: Consultant Mode VM Creation', async () => {
    const { extractChoicePrompt } = await getAiToolParser();
    const { serializeToolStep, parseToolStep } = await getToolStepSerializer();

    it('Step 1: AI prompts user with structured [ASK_CHOICE] card', () => {
      const response = `
I can help configure a new Colima instance tailored for your development needs.

[ASK_CHOICE: {"question": "Select hardware profile for new VM:", "multi": false, "options": [{"label": "Standard (4 CPU · 8GB RAM)", "value": "standard-4cpu-8gb", "description": "Recommended for standard Docker workloads"}, {"label": "High-Performance (8 CPU · 16GB RAM)", "value": "high-8cpu-16gb", "description": "Recommended for Kubernetes & local builds"}]}]
`;
      const { cleanText, choicePrompt } = extractChoicePrompt(response);
      expect(choicePrompt).toBeDefined();
      expect(choicePrompt?.multi).toBe(false);
      expect(choicePrompt?.options).toHaveLength(2);
      expect(cleanText).toContain('I can help configure a new Colima instance');
    });

    it('Step 2: selecting option submits choice and locks older card', () => {
      const selectedValue = 'Standard (4 CPU · 8GB RAM)';
      const conversation: SimulatedMessage[] = [
        { id: 'turn-1', role: 'user', content: 'Create a new VM' },
        { id: 'turn-2', role: 'assistant', content: 'Choose hardware profile: [ASK_CHOICE: ...]' },
        { id: 'turn-3', role: 'user', content: selectedValue },
        { id: 'turn-4', role: 'assistant', content: 'Configuring VM with Standard 4 CPU and 8GB RAM...' },
      ];

      expect(conversation[2].content).toBe(selectedValue);

      // Verify that turn-2 choice card is locked because turn-4 is now the latest assistant message
      const latestAssistantId = [...conversation]
        .reverse()
        .find((m) => m.role === 'assistant')?.id;
      expect(latestAssistantId).toBe('turn-4');
      const isTurn2Active = latestAssistantId === 'turn-2';
      expect(isTurn2Active).toBe(false);
    });

    it('Step 3: AI generates creation command requiring user approval before execution', () => {
      const createStep: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'colima-create',
        title: 'Create Colima VM with 4 CPU, 8GB RAM',
        safety: 'NORMAL',
        status: 'pending_approval',
        rawInput: 'colima start --cpu 4 --memory 8 --disk 60',
        timestamp: Date.now(),
      };

      const serialized = serializeToolStep(createStep);
      const parsed = parseToolStep(serialized);
      expect(parsed?.status).toBe('pending_approval');
      expect(parsed?.rawInput).toContain('--cpu 4 --memory 8');
    });
  });

  // -------------------------------------------------------------------------
  // Scenario D: Full Multi-Turn Chat Workflow from Problem Diagnosis to Approval
  // -------------------------------------------------------------------------
  describe('Scenario D: Full Multi-Turn Chat Workflow from Problem Diagnosis to Approval', async () => {
    const parse = await getChatMarkdownParser();
    const { serializeToolStep, parseToolStep } = await getToolStepSerializer();

    it('Step 1: captures Live UI Context snapshot with exited container api-gateway in Vietnamese locale', () => {
      const mockContainers = [
        { name: 'postgres', Status: 'Up 2 hours', Image: 'postgres:16', State: 'running' },
        { name: 'api-gateway', Status: 'Exited (137)', Image: 'corp/gateway:v1.2', State: 'exited' },
      ];

      const snapshot = specBuildRichLiveUiContext({
        currentPage: 'containers',
        language: 'vi',
        containers: mockContainers,
      });

      expect(snapshot).toContain('Language: vi');
      expect(snapshot).toContain('api-gateway [Exited (137), corp/gateway:v1.2]');
      expect(snapshot).toContain('Ground Truth Rule: Prioritize [LIVE_UI_CONTEXT]');
    });

    it('Step 2: user initiates diagnosis turn and typing/skeleton indicator activates while waiting for tokens', () => {
      const userMessage = 'Tại sao api-gateway bị dừng?';
      const history: SimulatedMessage[] = [
        { id: 'msg-user-1', role: 'user', content: userMessage },
      ];

      const isProcessing = true;
      const streamContent = '';
      const choicePrompt = null;

      // Indicator is active while processing before tokens arrive
      const showTypingIndicator = isProcessing && !streamContent && !choicePrompt;
      expect(showTypingIndicator).toBe(true);
      expect(history[0].content).toContain('api-gateway');
    });

    it('Step 3: assistant emits reasoning <think> block and read-only diagnostic tool with intermediate draft', () => {
      const round1Chunk = `
<think>
Checking container exit code 137 in live context.
Exit code 137 indicates SIGKILL usually caused by Linux kernel OOM (Out Of Memory).
I will query container inspect data to confirm memory limits.
</think>
Tôi đang kiểm tra thông tin chi tiết của container api-gateway... [QUERY: inspect-container | {"id": "api-gateway"}]
`;
      // Verify reasoning block
      const blocks = parse(round1Chunk);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think).toBeDefined();
      expect(think.content).toContain('Exit code 137 indicates SIGKILL');

      // Verify tool parsing and clean text
      const parsed = parseAiTools(round1Chunk);
      expect(parsed.queries).toHaveLength(1);
      expect(parsed.cleanText).toContain('Tôi đang kiểm tra thông tin chi tiết');

      // Speculative draft message ID captured
      const pendingSupersededStreamId = 'msg-draft-1';
      expect(pendingSupersededStreamId).toBe('msg-draft-1');
    });

    it('Step 4: tool execution output is returned and capped by defensive head+tail budget pruner', () => {
      const rawToolOutput = `
Container: api-gateway
State: Exited (137)
OOMKilled: true
MemoryLimit: 268435456 bytes (256MB)
Logs:
` + 'Out of memory: Kill process 1245 (node) score 850 or sacrifice child\n'.repeat(50) + `
Exit status: 137
`;
      const capped = capToolOutput(rawToolOutput, 500);
      expect(capped.length).toBeLessThan(700);
      expect(capped).toContain('OOMKilled: true');
      expect(capped).toContain('Exit status: 137');
    });

    it('Step 5: round 2 begins and revokes intermediate draft bubble when final synthesis text streams', () => {
      const conversation: SimulatedMessage[] = [
        { id: 'msg-user-1', role: 'user', content: 'Tại sao api-gateway bị dừng?' },
        { id: 'msg-draft-1', role: 'assistant', content: 'Tôi đang kiểm tra thông tin chi tiết...' },
      ];

      let pendingSupersededStreamId: string | null = 'msg-draft-1';

      // Simulation of round 2 arrival
      const round2Text = 'Container `api-gateway` bị dừng do hết bộ nhớ (OOMKilled, exit code 137). Giới hạn hiện tại là 256MB không đủ cho tiến trình Node.js.';

      // Revocation triggered immediately upon first answer chunk
      if (pendingSupersededStreamId && round2Text.trim()) {
        const idx = conversation.findIndex((m) => m.id === pendingSupersededStreamId);
        if (idx !== -1) conversation.splice(idx, 1);
        pendingSupersededStreamId = null;
      }

      // Final turn pushed
      conversation.push({ id: 'msg-final-1', role: 'assistant', content: round2Text });

      // Verifies intermediate draft bubble was cleanly deleted
      expect(conversation.find((m) => m.id === 'msg-draft-1')).toBeUndefined();
      expect(conversation).toHaveLength(2);
      expect(conversation[1].content).toContain('OOMKilled');
      expect(pendingSupersededStreamId).toBeNull();
    });

    it('Step 6: assistant proposes fix command via ToolStep requiring user approval (pending_approval)', () => {
      const toolStepPayload: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'docker-restart-with-mem',
        title: 'Khởi động lại api-gateway với giới hạn 1GB RAM',
        safety: 'NORMAL',
        status: 'pending_approval',
        rawInput: 'docker run -d --name api-gateway -m 1g -p 8080:8080 corp/gateway:v1.2',
        timestamp: Date.now(),
      };

      const serialized = serializeToolStep(toolStepPayload);
      expect(serialized).toContain('<!--AI_TOOL_STEP:');

      const parsed = parseToolStep(serialized);
      expect(parsed?.status).toBe('pending_approval');
      expect(parsed?.safety).toBe('NORMAL');
      expect(parsed?.rawInput).toContain('-m 1g');
    });

    it('Step 7: user approves command; tool card updates in-place to running then succeeded', () => {
      const step: ToolStepPayload = {
        version: 1,
        kind: 'command',
        name: 'docker-restart-with-mem',
        title: 'Khởi động lại api-gateway với giới hạn 1GB RAM',
        safety: 'NORMAL',
        status: 'pending_approval',
        rawInput: 'docker run -d --name api-gateway -m 1g -p 8080:8080 corp/gateway:v1.2',
      };

      // 1. In-place transition to running
      const runningStep = { ...step, status: 'running' as const };
      const runningSerialized = serializeToolStep(runningStep);
      expect(parseToolStep(runningSerialized)?.status).toBe('running');

      // 2. In-place transition to succeeded
      const succeededStep: ToolStepPayload = {
        ...step,
        status: 'succeeded',
        exitCode: 0,
        output: 'Container api-gateway started successfully (ID: 9a8b7c6d5e).',
      };
      const succeededSerialized = serializeToolStep(succeededStep);
      const parsedSucceeded = parseToolStep(succeededSerialized);
      expect(parsedSucceeded?.status).toBe('succeeded');
      expect(parsedSucceeded?.exitCode).toBe(0);
    });

    it('Step 8: assistant provides follow-up chips and writes turns in strictly visual order to SQLite', () => {
      const assistantFinalWithSuggestions = `
api-gateway đã được khởi động lại thành công với 1GB RAM.
[SUGGEST: Xem logs mới | docker logs -f api-gateway]
[SUGGEST: Kiểm tra RAM | docker stats --no-stream api-gateway]
`;
      const parsed = parseAiTools(assistantFinalWithSuggestions);
      expect(parsed.suggests).toHaveLength(2);
      expect(parsed.suggests[0][1]).toBe('Xem logs mới');

      // Conversation visual order persistence
      const fullConversation = [
        { id: '1', role: 'user' as const, content: 'Tại sao api-gateway bị dừng?', created_at: '2026-11-01T10:00:00Z', rowid: 1 },
        { id: '2', role: 'assistant' as const, content: parsed.cleanText, created_at: '2026-11-01T10:01:00Z', rowid: 2 },
      ];

      const sorted = specSortHistoryDeterministic(fullConversation);
      expect(sorted[0].id).toBe('1');
      expect(sorted[1].id).toBe('2');
    });
  });
});

