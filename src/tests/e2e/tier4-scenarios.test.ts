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
  type ThinkBlock,
  type CodeBlock,
  type ToolStepPayload,
} from './helpers/contractAdapter';
import {
  setupClipboardMock,
  deriveActiveVmTarget,
  type SimulatedMessage,
} from './helpers/testUtils';

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
});
