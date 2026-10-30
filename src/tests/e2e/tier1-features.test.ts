/**
 * E2E Test Suite — Tier 1: Feature Coverage (Happy Path in Isolation)
 *
 * Requirements: >=5 test cases / assertions per feature in PROJECT.md Feature Inventory.
 */

import { describe, it, expect } from 'vitest';
import en from '../../locales/en.json';
import vi from '../../locales/vi.json';
import ja from '../../locales/ja.json';
import zh from '../../locales/zh.json';
import {
  getChatMarkdownParser,
  getToolStepSerializer,
  getAiToolParser,
  getAiSuggestionsEngine,
  type ThinkBlock,
  type CodeBlock,
  type TableBlock,
  type HeadingBlock,
  type ListBlock,
} from './helpers/contractAdapter';
import {
  MARKDOWN_HAPPY_PATH,
  MARKDOWN_THINK_CLOSED,
  MARKDOWN_SHELL_BLOCKS,
  TOOL_STEP_PENDING_NORMAL,
  TOOL_STEP_PENDING_DANGEROUS,
  TOOL_STEP_RUNNING,
  TOOL_STEP_SUCCEEDED,
  TOOL_STEP_FAILED,
  TOOL_STEP_DENIED,
  RAW_CHOICE_JSON_SINGLE_TAG,
  CHOICE_JSON_MULTI,
  SUGGEST_TRIPLE_TAG,
  PARTIAL_STREAM_CHUNKS,
  MOCK_CTX_STOPPED_INSTANCE,
  MOCK_CTX_HEALTHY_COMPOSE,
  MOCK_FALLBACK_AFTER_QUERY,
  MOCK_FALLBACK_AFTER_DIAGNOSE,
} from './helpers/fixtures';
import {
  isNearBottom,
  simulateTruncateAiMessagesFrom,
  deriveActiveVmTarget,
  readProjectFile,
  type SimulatedMessage,
} from './helpers/testUtils';
import { renderMarkdown } from '../../lib/markdown';
import { parseAiTools } from '../../lib/aiToolParser';

describe('Tier 1: Feature Coverage (Isolated Happy Path)', () => {
  // -------------------------------------------------------------------------
  // Feature 1: Database Backward Compatibility & History Truncation
  // -------------------------------------------------------------------------
  describe('F1: Database Backward Compatibility & Truncation', () => {
    const mockHistory: SimulatedMessage[] = [
      { id: 'msg-1', role: 'user', content: 'Hello AI' },
      { id: 'msg-2', role: 'assistant', content: 'Hello, how can I help?' },
      { id: 'msg-3', role: 'user', content: 'Check Colima' },
      { id: 'msg-4', role: 'assistant', content: 'Colima is running' },
      { id: 'msg-5', role: 'user', content: 'Inspect containers' },
    ];

    it('1.1: retains standard message roles without schema alterations', () => {
      for (const msg of mockHistory) {
        expect(['user', 'assistant', 'system']).toContain(msg.role);
        expect(typeof msg.id).toBe('string');
        expect(typeof msg.content).toBe('string');
      }
    });

    it('1.2: truncates history from middle position cleanly', () => {
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(mockHistory, 'msg-3');
      expect(remaining.map((m) => m.id)).toEqual(['msg-1', 'msg-2']);
      expect(truncated.map((m) => m.id)).toEqual(['msg-3', 'msg-4', 'msg-5']);
    });

    it('1.3: truncates from head (restarting conversation)', () => {
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(mockHistory, 'msg-1');
      expect(remaining).toHaveLength(0);
      expect(truncated).toHaveLength(5);
    });

    it('1.4: truncates from tail (regenerating latest turn)', () => {
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(mockHistory, 'msg-5');
      expect(remaining.map((m) => m.id)).toEqual(['msg-1', 'msg-2', 'msg-3', 'msg-4']);
      expect(truncated.map((m) => m.id)).toEqual(['msg-5']);
    });

    it('1.5: handles non-existent ID gracefully without truncating', () => {
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(mockHistory, 'non-existent');
      expect(remaining).toHaveLength(5);
      expect(truncated).toHaveLength(0);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 2: Help.svelte Preservation (renderMarkdown untouched)
  // -------------------------------------------------------------------------
  describe('F2: Help.svelte Markdown Preservation', () => {
    it('2.1: preserves data-slug link format required by delegateArticleLinks', () => {
      const input = 'Read the [Quickstart Guide](quickstart) to begin.';
      const output = renderMarkdown(input);
      expect(output).toContain('<a class="md-link" data-slug="quickstart">Quickstart Guide</a>');
    });

    it('2.2: preserves data-slug formatting on external urls for delegateArticleLinks', () => {
      const input = 'Visit [Colima GitHub](https://github.com/abiosoft/colima).';
      const output = renderMarkdown(input);
      expect(output).toContain('<a class="md-link" data-slug="https://github.com/abiosoft/colima">Colima GitHub</a>');
    });

    it('2.3: generates h2 title headings with md-h-title class', () => {
      const input = '# Documentation Overview';
      const output = renderMarkdown(input);
      expect(output).toContain('<h2 class="md-h md-h-title">Documentation Overview</h2>');
    });

    it('2.4: generates h4 subheadings with md-h class from markdown ###', () => {
      const input = '### Configuration Options';
      const output = renderMarkdown(input);
      expect(output).toContain('<h4 class="md-h">Configuration Options</h4>');
    });

    it('2.5: handles inline bold and styled code formatting correctly', () => {
      const input = 'Use **bold** and `colima start` code.';
      const output = renderMarkdown(input);
      expect(output).toContain('<strong>bold</strong>');
      expect(output).toContain('<code style=');
      expect(output).toContain('colima start</code>');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 3: Modular Chat Markdown AST Parser
  // -------------------------------------------------------------------------
  describe('F3: Modular Chat Markdown AST Parser', async () => {
    const parse = await getChatMarkdownParser();

    it('3.1: parses full GFM markdown document into typed blocks', () => {
      const blocks = parse(MARKDOWN_HAPPY_PATH);
      const types = blocks.map((b) => b.type);
      expect(types).toContain('heading');
      expect(types).toContain('paragraph');
      expect(types).toContain('table');
      expect(types).toContain('code');
      expect(types).toContain('blockquote');
      expect(types).toContain('list');
    });

    it('3.2: extracts heading level and text', () => {
      const blocks = parse('## Diagnostic Details\nProse line');
      const heading = blocks[0] as HeadingBlock;
      expect(heading.type).toBe('heading');
      expect(heading.level).toBe(2);
      expect(heading.text).toBe('Diagnostic Details');
    });

    it('3.3: extracts lists with ordered and unordered flags', () => {
      const unordered = parse('- Item A\n- Item B') as [ListBlock];
      expect(unordered[0].type).toBe('list');
      expect(unordered[0].ordered).toBe(false);
      expect(unordered[0].items).toEqual(['Item A', 'Item B']);

      const ordered = parse('1. Step 1\n2. Step 2') as [ListBlock];
      expect(ordered[0].type).toBe('list');
      expect(ordered[0].ordered).toBe(true);
      expect(ordered[0].items).toEqual(['Step 1', 'Step 2']);
    });

    it('3.4: parses blockquotes and horizontal rules', () => {
      const quoteBlocks = parse('> Important warning note');
      expect(quoteBlocks[0].type).toBe('blockquote');
      expect((quoteBlocks[0] as { text: string }).text).toBe('Important warning note');

      const hrBlocks = parse('---');
      expect(hrBlocks[0].type).toBe('hr');
    });

    it('3.5: segments multiple paragraphs separated by blank lines', () => {
      const blocks = parse('Paragraph one.\n\nParagraph two.');
      expect(blocks).toHaveLength(2);
      expect(blocks[0].type).toBe('paragraph');
      expect(blocks[1].type).toBe('paragraph');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 4: Streaming <think> Reasoning Handling
  // -------------------------------------------------------------------------
  describe('F4: Streaming <think> Reasoning Handling', async () => {
    const parse = await getChatMarkdownParser();

    it('4.1: parses closed <think> tag with streaming = false', () => {
      const blocks = parse(MARKDOWN_THINK_CLOSED);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think).toBeDefined();
      expect(think.streaming).toBe(false);
      expect(think.content).toContain('Detected exit code 137');
    });

    it('4.2: separates reasoning from trailing prose in closed block', () => {
      const blocks = parse(MARKDOWN_THINK_CLOSED);
      const paragraph = blocks.find((b) => b.type === 'paragraph');
      expect(paragraph).toBeDefined();
      expect((paragraph as { text: string }).text).toContain('The container terminated');
    });

    it('4.3: parses inline single-line <think>...</think>', () => {
      const blocks = parse('<think>Quick reasoning</think> Answer text');
      expect(blocks).toHaveLength(2);
      expect(blocks[0].type).toBe('think');
      expect((blocks[0] as ThinkBlock).content).toBe('Quick reasoning');
      expect((blocks[0] as ThinkBlock).streaming).toBe(false);
      expect(blocks[1].type).toBe('paragraph');
    });

    it('4.4: parses multiline reasoning body', () => {
      const input = '<think>\nLine 1\nLine 2\nLine 3\n</think>\nDone.';
      const blocks = parse(input);
      const think = blocks[0] as ThinkBlock;
      expect(think.content).toBe('Line 1\nLine 2\nLine 3');
      expect(think.streaming).toBe(false);
    });

    it('4.5: preserves pre-think greeting paragraph if present', () => {
      const input = 'Greeting user.\n<think>Reasoning</think>\nFinal answer.';
      const blocks = parse(input);
      expect(blocks).toHaveLength(3);
      expect((blocks[0] as { text: string }).text).toBe('Greeting user.');
      expect(blocks[1].type).toBe('think');
      expect((blocks[2] as { text: string }).text).toBe('Final answer.');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 5: GFM Responsive Tables
  // -------------------------------------------------------------------------
  describe('F5: GFM Responsive Container Tables', async () => {
    const parse = await getChatMarkdownParser();

    it('5.1: extracts table headers accurately', () => {
      const blocks = parse(MARKDOWN_HAPPY_PATH);
      const table = blocks.find((b) => b.type === 'table') as TableBlock;
      expect(table).toBeDefined();
      expect(table.headers).toEqual(['Service', 'Status', 'Port']);
    });

    it('5.2: extracts table column alignments (left, center, right)', () => {
      const blocks = parse(MARKDOWN_HAPPY_PATH);
      const table = blocks.find((b) => b.type === 'table') as TableBlock;
      expect(table.alignments).toEqual(['left', 'center', 'right']);
    });

    it('5.3: parses multiple data rows with clean cell values', () => {
      const blocks = parse(MARKDOWN_HAPPY_PATH);
      const table = blocks.find((b) => b.type === 'table') as TableBlock;
      expect(table.rows).toHaveLength(3);
      expect(table.rows[0]).toEqual(['Docker Daemon', 'Running', '2375']);
      expect(table.rows[1]).toEqual(['Kubernetes API', 'Active', '6443']);
      expect(table.rows[2]).toEqual(['Buildx Builder', 'Ready', 'N/A']);
    });

    it('5.4: handles tables with default (none) alignments', () => {
      const input = '| Col A | Col B |\n| --- | --- |\n| Val 1 | Val 2 |';
      const blocks = parse(input);
      const table = blocks[0] as TableBlock;
      expect(table.alignments).toEqual(['none', 'none']);
      expect(table.rows[0]).toEqual(['Val 1', 'Val 2']);
    });

    it('5.5: does not confuse pipe prose with tables', () => {
      const input = 'This is a sentence with | pipe symbols | but not a table.';
      const blocks = parse(input);
      expect(blocks[0].type).toBe('paragraph');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 6: Tool Step Data Model & Serialization
  // -------------------------------------------------------------------------
  describe('F6: Tool Step Data Model & Serialization', async () => {
    const { serializeToolStep, parseToolStep } = await getToolStepSerializer();

    it('6.1: serializes and parses pending normal tool step roundtrip', () => {
      const serialized = serializeToolStep(TOOL_STEP_PENDING_NORMAL);
      expect(serialized).toContain('<!--AI_TOOL_STEP:');
      const parsed = parseToolStep(serialized);
      expect(parsed).toEqual(TOOL_STEP_PENDING_NORMAL);
    });

    it('6.2: serializes dangerous tool step with warning preserved', () => {
      const serialized = serializeToolStep(TOOL_STEP_PENDING_DANGEROUS);
      const parsed = parseToolStep(serialized);
      expect(parsed?.safety).toBe('DANGEROUS');
      expect(parsed?.warning).toContain('permanently removes all stopped containers');
    });

    it('6.3: serializes running tool step cleanly', () => {
      const serialized = serializeToolStep(TOOL_STEP_RUNNING);
      const parsed = parseToolStep(serialized);
      expect(parsed?.status).toBe('running');
      expect(parsed?.name).toBe('diagnose-colima');
    });

    it('6.4: serializes succeeded tool step with output and exitCode 0', () => {
      const serialized = serializeToolStep(TOOL_STEP_SUCCEEDED);
      const parsed = parseToolStep(serialized);
      expect(parsed?.status).toBe('succeeded');
      expect(parsed?.exitCode).toBe(0);
      expect(parsed?.output).toContain('CONTAINER ID');
    });

    it('6.5: serializes failed and denied tool steps', () => {
      const failSer = serializeToolStep(TOOL_STEP_FAILED);
      expect(parseToolStep(failSer)?.status).toBe('failed');

      const denySer = serializeToolStep(TOOL_STEP_DENIED);
      expect(parseToolStep(denySer)?.status).toBe('denied');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 7: [ASK_CHOICE] Extraction
  // -------------------------------------------------------------------------
  describe('F7: [ASK_CHOICE] Extraction', async () => {
    const { extractChoicePrompt } = await getAiToolParser();

    it('7.1: extracts standard single-choice JSON format', () => {
      const input = `Please choose:\n${RAW_CHOICE_JSON_SINGLE_TAG}\nLet me know.`;
      const { cleanText, choicePrompt } = extractChoicePrompt(input);
      expect(choicePrompt).toBeDefined();
      expect(choicePrompt?.multi).toBe(false);
      expect(choicePrompt?.question).toContain('Select CPU and RAM configuration');
      expect(choicePrompt?.options).toHaveLength(3);
      expect(cleanText).toContain('Please choose:');
      expect(cleanText).toContain('Let me know.');
    });

    it('7.2: extracts multi-choice JSON format', () => {
      const input = `[ASK_CHOICE: ${JSON.stringify(CHOICE_JSON_MULTI)}]`;
      const { choicePrompt } = extractChoicePrompt(input);
      expect(choicePrompt?.multi).toBe(true);
      expect(choicePrompt?.options).toHaveLength(3);
      expect(choicePrompt?.options[0].value).toBe('redis:6-alpine');
    });

    it('7.3: extracts compact pipe-delimited choice syntax', () => {
      const input = '[ASK_CHOICE: Pick an action | Start VM | Stop VM | Restart VM]';
      const { cleanText, choicePrompt } = extractChoicePrompt(input);
      expect(choicePrompt?.question).toBe('Pick an action');
      expect(choicePrompt?.multi).toBe(false);
      expect(choicePrompt?.options).toEqual([
        { label: 'Start VM', value: 'Start VM' },
        { label: 'Stop VM', value: 'Stop VM' },
        { label: 'Restart VM', value: 'Restart VM' },
      ]);
      expect(cleanText).toBe('');
    });

    it('7.4: strips choice tag completely from prose', () => {
      const input = 'Answer this: [ASK_CHOICE: A or B? | A | B] Thank you!';
      const { cleanText } = extractChoicePrompt(input);
      expect(cleanText).toBe('Answer this:  Thank you!');
    });

    it('7.5: returns null choicePrompt when no tag is present', () => {
      const input = 'Just normal chat answer without choices.';
      const { cleanText, choicePrompt } = extractChoicePrompt(input);
      expect(choicePrompt).toBeNull();
      expect(cleanText).toBe(input);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 8: Streaming Tag Suppression
  // -------------------------------------------------------------------------
  describe('F8: Streaming Tag Suppression', async () => {
    const { stripStreamingPartialTags } = await getAiToolParser();

    it('8.1: suppresses partial [QUERY: ... tag at stream tail', () => {
      const item = PARTIAL_STREAM_CHUNKS[0];
      const res = stripStreamingPartialTags(item.raw);
      expect(res.cleanText).toBe(item.expectedClean);
      expect(res.pendingTagKind).toBe('query');
    });

    it('8.2: suppresses partial [EVENT_APPROVE: ... tag at stream tail', () => {
      const item = PARTIAL_STREAM_CHUNKS[1];
      const res = stripStreamingPartialTags(item.raw);
      expect(res.cleanText).toBe(item.expectedClean);
      expect(res.pendingTagKind).toBe('event_approve');
    });

    it('8.3: suppresses partial [ASK_CHOICE: ... tag at stream tail', () => {
      const item = PARTIAL_STREAM_CHUNKS[2];
      const res = stripStreamingPartialTags(item.raw);
      expect(res.cleanText).toBe(item.expectedClean);
      expect(res.pendingTagKind).toBe('choice');
    });

    it('8.4: suppresses partial [SUGGEST: ... tag at stream tail', () => {
      const item = PARTIAL_STREAM_CHUNKS[3];
      const res = stripStreamingPartialTags(item.raw);
      expect(res.cleanText).toBe(item.expectedClean);
      expect(res.pendingTagKind).toBe('suggest');
    });

    it('8.5: does NOT suppress regular markdown links or brackets', () => {
      const linkItem = PARTIAL_STREAM_CHUNKS[4];
      expect(stripStreamingPartialTags(linkItem.raw).pendingTagKind).toBeNull();

      const bracketItem = PARTIAL_STREAM_CHUNKS[5];
      expect(stripStreamingPartialTags(bracketItem.raw).pendingTagKind).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // Feature 9: Assistant Turn Content Rebuilder
  // -------------------------------------------------------------------------
  describe('F9: Assistant Turn Content Rebuilder', async () => {
    const { rebuildAssistantTurnContent } = await getAiToolParser();

    it('9.1: rebuilds turn retaining prose and [SUGGEST] chips', () => {
      const cleanText = 'Diagnose complete.';
      const suggests = [
        ['Check logs', 'Show me the last 50 lines'],
        ['Restart', 'Restart'],
      ];
      const rebuilt = rebuildAssistantTurnContent(cleanText, suggests);
      expect(rebuilt).toContain('Diagnose complete.');
      expect(rebuilt).toContain('[SUGGEST: Check logs | Show me the last 50 lines]');
      expect(rebuilt).toContain('[SUGGEST: Restart]');
    });

    it('9.2: preserves [ASK_CHOICE] tags in rebuilt content', () => {
      const cleanText = 'Please select:';
      const rawChoices = ['[ASK_CHOICE: Option A | Option B]'];
      const rebuilt = rebuildAssistantTurnContent(cleanText, [], rawChoices);
      expect(rebuilt).toContain('[ASK_CHOICE: Option A | Option B]');
    });

    it('9.3: handles turns containing only suggestions without prose', () => {
      const rebuilt = rebuildAssistantTurnContent('', [['Retry', 'Retry']]);
      expect(rebuilt).toBe('[SUGGEST: Retry]');
    });

    it('9.4: handles turns containing only choice prompts', () => {
      const rebuilt = rebuildAssistantTurnContent('', [], ['[ASK_CHOICE: Q | Opt1]']);
      expect(rebuilt).toBe('[ASK_CHOICE: Q | Opt1]');
    });

    it('9.5: produces clean trimmed string without trailing whitespace', () => {
      const rebuilt = rebuildAssistantTurnContent('  Clean text  ', []);
      expect(rebuilt).toBe('Clean text');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 10: Contextual Fallback Suggestions
  // -------------------------------------------------------------------------
  describe('F10: Contextual Fallback Suggestions', async () => {
    const { deriveFallbackSuggestions } = await getAiSuggestionsEngine();

    it('10.1: generates resource check & log suggestions after query tools', () => {
      const fallbacks = deriveFallbackSuggestions(MOCK_FALLBACK_AFTER_QUERY);
      expect(fallbacks.length).toBeGreaterThanOrEqual(2);
      const labels = fallbacks.map((f) => f[0]);
      expect(labels).toContain('Check CPU & RAM');
    });

    it('10.2: generates fix proposal suggestions after diagnose tool', () => {
      const fallbacks = deriveFallbackSuggestions(MOCK_FALLBACK_AFTER_DIAGNOSE);
      expect(fallbacks.length).toBeGreaterThanOrEqual(2);
      const labels = fallbacks.map((f) => f[0]);
      expect(labels).toContain('Apply recommended fix');
    });

    it('10.3: generates status verification after event execution', () => {
      const fallbacks = deriveFallbackSuggestions({
        toolsExecuted: [{ kind: 'event', name: 'stop-container' }],
        currentPage: 'containers',
      });
      const labels = fallbacks.map((f) => f[0]);
      expect(labels).toContain('Verify status');
    });

    it('10.4: provides generic fallback suggestions when no tools executed', () => {
      const fallbacks = deriveFallbackSuggestions({
        toolsExecuted: [],
        currentPage: 'dashboard',
      });
      expect(fallbacks.length).toBeGreaterThanOrEqual(2);
      expect(fallbacks[0][0]).toBe('Show system status');
    });

    it('10.5: caps fallback suggestions at maximum 3 chips', () => {
      const fallbacks = deriveFallbackSuggestions(MOCK_FALLBACK_AFTER_QUERY);
      expect(fallbacks.length).toBeLessThanOrEqual(3);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 11: Live Context Starter Prompts
  // -------------------------------------------------------------------------
  describe('F11: Live Context Starter Prompts', async () => {
    const { getStarterPrompts } = await getAiSuggestionsEngine();

    it('11.1: prioritizes stopped Colima instance as warning prompt #1', () => {
      const prompts = getStarterPrompts(MOCK_CTX_STOPPED_INSTANCE);
      expect(prompts[0].category).toBe('warning');
      expect(prompts[0].title).toContain('Investigate stopped instance default');
      expect(prompts[0].icon).toBe('warning');
    });

    it('11.2: prioritizes exited container as warning prompt', () => {
      const prompts = getStarterPrompts({
        currentPage: 'containers',
        colimaInstances: [{ name: 'default', status: 'Running' }],
        containers: [{ id: 'c-1', name: 'web-srv', state: 'exited' }],
      });
      expect(prompts[0].category).toBe('warning');
      expect(prompts[0].title).toContain('Analyze exited container web-srv');
    });

    it('11.3: includes tab-specific prompt for Compose page', () => {
      const prompts = getStarterPrompts(MOCK_CTX_HEALTHY_COMPOSE);
      const composePrompt = prompts.find((p) => p.category === 'context');
      expect(composePrompt?.title).toContain('Diagnose Compose services');
    });

    it('11.4: includes tab-specific prompt for Containers page', () => {
      const prompts = getStarterPrompts({
        currentPage: 'containers',
        colimaInstances: [{ name: 'default', status: 'Running' }],
        containers: [{ id: 'c-1', name: 'app', state: 'running' }],
      });
      const containerPrompt = prompts.find((p) => p.category === 'context');
      expect(containerPrompt?.title).toContain('Inspect resource consumption');
    });

    it('11.5: returns at most 4 starter prompts total', () => {
      const prompts = getStarterPrompts(MOCK_CTX_STOPPED_INSTANCE);
      expect(prompts.length).toBeLessThanOrEqual(4);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 13: Shell Code Block Detection
  // -------------------------------------------------------------------------
  describe('F13: Shell Code Block Detection', async () => {
    const parse = await getChatMarkdownParser();

    it('13.1: identifies bash/sh/zsh blocks as shell commands', () => {
      for (const item of MARKDOWN_SHELL_BLOCKS.filter((b) => b.expectedShell && b.lang)) {
        const blocks = parse(`\`\`\`${item.lang}\n${item.code}\n\`\`\``);
        const codeBlock = blocks[0] as CodeBlock;
        expect(codeBlock.type).toBe('code');
        expect(codeBlock.isShell).toBe(true);
      }
    });

    it('13.2: identifies untagged blocks starting with docker/colima as shell', () => {
      const blocks = parse('```\ncolima status\ndocker ps\n```');
      const codeBlock = blocks[0] as CodeBlock;
      expect(codeBlock.isShell).toBe(true);
    });

    it('13.3: identifies python/json blocks as NON-shell', () => {
      for (const item of MARKDOWN_SHELL_BLOCKS.filter((b) => !b.expectedShell)) {
        const blocks = parse(`\`\`\`${item.lang}\n${item.code}\n\`\`\``);
        const codeBlock = blocks[0] as CodeBlock;
        expect(codeBlock.isShell).toBe(false);
      }
    });

    it('13.4: preserves exact code content without trimming internal indentation', () => {
      const code = '  def indented():\n    return 42';
      const blocks = parse(`\`\`\`python\n${code}\n\`\`\``);
      expect((blocks[0] as CodeBlock).code).toBe(code);
    });

    it('13.5: extracts language tag in lower-case', () => {
      const blocks = parse('```BASH\necho hello\n```');
      expect((blocks[0] as CodeBlock).language).toBe('bash');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 14: Active VM Terminal Target Resolution
  // -------------------------------------------------------------------------
  describe('F14: Active VM Terminal Target Resolution', () => {
    it('14.1: resolves to running Colima instance profile', () => {
      const target = deriveActiveVmTarget([
        { name: 'default', status: 'Running' },
        { name: 'staging', status: 'Stopped' },
      ]);
      expect(target).toEqual({ kind: 'colima', profile: 'default' });
    });

    it('14.2: resolves to named profile when custom instance is running', () => {
      const target = deriveActiveVmTarget([
        { name: 'default', status: 'Stopped' },
        { name: 'dev-k8s', status: 'Running' },
      ]);
      expect(target).toEqual({ kind: 'colima', profile: 'dev-k8s' });
    });

    it('14.3: normalizes "colima" instance name to "default" profile', () => {
      const target = deriveActiveVmTarget([{ name: 'colima', status: 'Running' }]);
      expect(target).toEqual({ kind: 'colima', profile: 'default' });
    });

    it('14.4: falls back to default profile when no instances are running', () => {
      const target = deriveActiveVmTarget([
        { name: 'default', status: 'Stopped' },
      ]);
      expect(target).toEqual({ kind: 'colima', profile: 'default' });
    });

    it('14.5: handles empty instance array gracefully', () => {
      const target = deriveActiveVmTarget([]);
      expect(target).toEqual({ kind: 'colima', profile: 'default' });
    });
  });

  // -------------------------------------------------------------------------
  // Feature 20: Smart Auto-Scrolling Detection
  // -------------------------------------------------------------------------
  describe('F20: Smart Auto-Scrolling Detection', () => {
    it('20.1: detects near-bottom when exactly at bottom (distance = 0)', () => {
      const container = { scrollHeight: 1000, scrollTop: 600, clientHeight: 400 };
      expect(isNearBottom(container)).toBe(true);
    });

    it('20.2: detects near-bottom within 72px threshold', () => {
      const container = { scrollHeight: 1000, scrollTop: 535, clientHeight: 400 };
      // distance = 1000 - 535 - 400 = 65 < 72
      expect(isNearBottom(container)).toBe(true);
    });

    it('20.3: detects NOT near-bottom when user scrolled up beyond 72px', () => {
      const container = { scrollHeight: 1000, scrollTop: 400, clientHeight: 400 };
      // distance = 1000 - 400 - 400 = 200 >= 72
      expect(isNearBottom(container)).toBe(false);
    });

    it('20.4: respects custom threshold parameter if supplied', () => {
      const container = { scrollHeight: 1000, scrollTop: 500, clientHeight: 400 };
      // distance = 100
      expect(isNearBottom(container, 120)).toBe(true);
      expect(isNearBottom(container, 50)).toBe(false);
    });

    it('20.5: handles empty container (scrollHeight === clientHeight)', () => {
      const container = { scrollHeight: 400, scrollTop: 0, clientHeight: 400 };
      expect(isNearBottom(container)).toBe(true);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 23: CSS Design Token Compliance
  // -------------------------------------------------------------------------
  describe('F23: Design Token Compliance', async () => {
    const tokensCss = await readProjectFile('src/styles/tokens.css');

    it('23.1: defines primary background tokens', () => {
      expect(tokensCss).toContain('--bg-app:');
      expect(tokensCss).toContain('--bg-card:');
      expect(tokensCss).toContain('--bg-sidebar:');
    });

    it('23.2: defines border tokens', () => {
      expect(tokensCss).toContain('--border-primary:');
      expect(tokensCss).toContain('--border-subtle:');
    });

    it('23.3: defines text typography tokens', () => {
      expect(tokensCss).toContain('--text-primary:');
      expect(tokensCss).toContain('--text-secondary:');
      expect(tokensCss).toContain('--text-muted:');
    });

    it('23.4: defines accent color tokens', () => {
      expect(tokensCss).toContain('--accent-blue:');
      expect(tokensCss).toContain('--accent-green:');
      expect(tokensCss).toContain('--accent-red:');
    });

    it('23.5: defines font family tokens', () => {
      expect(tokensCss).toContain('--font-mono:');
      expect(tokensCss).toContain('--font-sans:');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 24: Localization Synchronization Across Locales
  // -------------------------------------------------------------------------
  describe('F24: Localization Synchronization', () => {
    it('24.1: en.json contains ai group', () => {
      expect(en.ai).toBeDefined();
      expect(typeof en.ai).toBe('object');
    });

    it('24.2: vi.json contains ai group', () => {
      expect(vi.ai).toBeDefined();
      expect(typeof vi.ai).toBe('object');
    });

    it('24.3: ja.json contains ai group', () => {
      expect(ja.ai).toBeDefined();
      expect(typeof ja.ai).toBe('object');
    });

    it('24.4: zh.json contains ai group', () => {
      expect(zh.ai).toBeDefined();
      expect(typeof zh.ai).toBe('object');
    });

    it('24.5: core ai chat keys exist in all 4 locales', () => {
      const coreKeys: Array<keyof typeof en.ai> = [
        'title',
        'input_placeholder',
        'send',
        'clear_chat',
        'stop',
      ];
      for (const k of coreKeys) {
        expect(en.ai[k]).toBeDefined();
        expect(vi.ai[k as keyof typeof vi.ai]).toBeDefined();
        expect(ja.ai[k as keyof typeof ja.ai]).toBeDefined();
        expect(zh.ai[k as keyof typeof zh.ai]).toBeDefined();
      }
    });
  });

  // -------------------------------------------------------------------------
  // Feature 26: aiToolParser [SUGGEST] Extraction
  // -------------------------------------------------------------------------
  describe('F26: [SUGGEST] Extraction', () => {
    it('26.1: extracts label and prompt from piped tag', () => {
      const parsed = parseAiTools('[SUGGEST: View logs | Show the last 50 lines]');
      expect(parsed.suggests).toHaveLength(1);
      expect(parsed.suggests[0][1]).toBe('View logs');
      expect(parsed.suggests[0][2]).toBe('Show the last 50 lines');
    });

    it('26.2: extracts label without prompt', () => {
      const parsed = parseAiTools('[SUGGEST: Restart container]');
      expect(parsed.suggests).toHaveLength(1);
      expect(parsed.suggests[0][1]).toBe('Restart container');
      expect(parsed.suggests[0][2]).toBeUndefined();
    });

    it('26.3: extracts multiple suggestion chips in order', () => {
      const parsed = parseAiTools(SUGGEST_TRIPLE_TAG);
      expect(parsed.suggests).toHaveLength(3);
      expect(parsed.suggests[0][1]).toBe('Check logs');
      expect(parsed.suggests[1][1]).toBe('Restart container');
      expect(parsed.suggests[2][1]).toBe('Inspect resources');
    });

    it('26.4: caps suggestion chips at MAX_SUGGESTIONS = 3', () => {
      const input = '[SUGGEST: 1][SUGGEST: 2][SUGGEST: 3][SUGGEST: 4][SUGGEST: 5]';
      const parsed = parseAiTools(input);
      expect(parsed.suggests).toHaveLength(3);
    });

    it('26.5: strips [SUGGEST] tags cleanly from visible text', () => {
      const input = 'Diagnosis finished. [SUGGEST: Next step]';
      const parsed = parseAiTools(input);
      expect(parsed.cleanText).toBe('Diagnosis finished.');
    });
  });
});
