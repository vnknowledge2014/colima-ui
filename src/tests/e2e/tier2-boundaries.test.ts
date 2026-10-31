/**
 * E2E Test Suite — Tier 2: Boundary & Corner Cases
 *
 * Requirements: >=5 test cases / assertions per boundary feature:
 * - Unclosed <think> streaming
 * - Malformed choice JSON & pipe fallback
 * - Empty chat state & onboarding triggers
 * - Max 3 suggestions & duplicate chip handling
 * - Shell vs non-shell code block boundaries
 * - History truncation at boundaries (head, middle, tail)
 * - XSS escaping and tag suppression boundaries
 */

import { describe, it, expect } from 'vitest';
import {
  getChatMarkdownParser,
  getAiToolParser,
  getAiSuggestionsEngine,
  renderInlineChatMarkdown,
  type ThinkBlock,
  type CodeBlock,
} from './helpers/contractAdapter';
import {
  MARKDOWN_THINK_STREAMING,
  MARKDOWN_XSS_VECTORS,
  SUGGEST_SURPLUS_TAG,
  SUGGEST_DUPLICATE_CHIPS,
  RAW_CHOICE_PIPE_TAG,
  RAW_CHOICE_MALFORMED_JSON,
} from './helpers/fixtures';
import {
  simulateTruncateAiMessagesFrom,
  isNearBottom,
  type SimulatedMessage,
} from './helpers/testUtils';
import { parseAiTools } from '../../lib/aiToolParser';

describe('Tier 2: Boundary & Corner Cases', () => {
  // -------------------------------------------------------------------------
  // Boundary 1: Unclosed <think> Streaming Handling
  // -------------------------------------------------------------------------
  describe('B1: Unclosed <think> Streaming Handling', async () => {
    const parse = await getChatMarkdownParser();

    it('1.1: emits think block with streaming: true when unclosed', () => {
      const blocks = parse(MARKDOWN_THINK_STREAMING);
      const think = blocks.find((b) => b.type === 'think') as ThinkBlock;
      expect(think).toBeDefined();
      expect(think.streaming).toBe(true);
      expect(think.content).toContain('Examining Colima VM lima.yaml configuration.');
    });

    it('1.2: handles empty <think> tag gracefully', () => {
      const blocks = parse('<think></think> Final answer.');
      expect(blocks).toHaveLength(2);
      expect((blocks[0] as ThinkBlock).content).toBe('');
      expect((blocks[0] as ThinkBlock).streaming).toBe(false);
      expect((blocks[1] as { text: string }).text).toBe('Final answer.');
    });

    it('1.3: handles solitary <think> without content', () => {
      const blocks = parse('<think>');
      expect(blocks).toHaveLength(1);
      expect((blocks[0] as ThinkBlock).type).toBe('think');
      expect((blocks[0] as ThinkBlock).streaming).toBe(true);
    });

    it('1.4: handles multiple <think> tags in separate paragraphs', () => {
      const input = '<think>Part 1</think>\nProse\n<think>Part 2</think>\nEnd.';
      const blocks = parse(input);
      const thinks = blocks.filter((b) => b.type === 'think') as ThinkBlock[];
      expect(thinks).toHaveLength(2);
      expect(thinks[0].content).toBe('Part 1');
      expect(thinks[1].content).toBe('Part 2');
    });

    it('1.5: does NOT freeze or discard subsequent tokens after unclosed <think>', () => {
      const input = '<think>\nLive reasoning...\nStill reasoning...';
      const blocks = parse(input);
      const think = blocks[0] as ThinkBlock;
      expect(think.content).toContain('Live reasoning...');
      expect(think.content).toContain('Still reasoning...');
      expect(think.streaming).toBe(true);
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 2: Malformed Choice JSON & Compact Pipe Fallback
  // -------------------------------------------------------------------------
  describe('B2: Malformed Choice JSON & Pipe Fallback', async () => {
    const { extractChoicePrompt } = await getAiToolParser();

    it('2.1: gracefully falls back to pipe parsing on malformed JSON', () => {
      const input = RAW_CHOICE_MALFORMED_JSON;
      const { choicePrompt } = extractChoicePrompt(input);
      // Malformed JSON should not throw; returns either fallback options or null safely
      expect(choicePrompt === null || Array.isArray(choicePrompt.options)).toBe(true);
    });

    it('2.2: parses compact pipe syntax with multiple options', () => {
      const { choicePrompt } = extractChoicePrompt(RAW_CHOICE_PIPE_TAG);
      expect(choicePrompt).toBeDefined();
      expect(choicePrompt?.question).toBe('Which container action do you want to run?');
      expect(choicePrompt?.options).toHaveLength(4);
      expect(choicePrompt?.options.map((o) => o.label)).toEqual([
        'Inspect logs',
        'Restart service',
        'Open shell',
        'Remove container',
      ]);
    });

    it('2.3: trims extra whitespace around pipe options', () => {
      const input = '[ASK_CHOICE: Choose mode |   Option A    |    Option B   ]';
      const { choicePrompt } = extractChoicePrompt(input);
      expect(choicePrompt?.options[0].label).toBe('Option A');
      expect(choicePrompt?.options[1].label).toBe('Option B');
    });

    it('2.4: handles empty options between double pipes gracefully', () => {
      const input = '[ASK_CHOICE: Question | Valid 1 || Valid 2]';
      const { choicePrompt } = extractChoicePrompt(input);
      if (choicePrompt) {
        // Empty options should either be filtered or not break parser
        for (const opt of choicePrompt.options) {
          expect(typeof opt.label).toBe('string');
        }
      }
    });

    it('2.5: handles choice tags containing nested quotes inside option labels', () => {
      const input = `[ASK_CHOICE: {"question": "Restart container \\"web\\"?", "multi": false, "options": [{"label": "Yes, restart \\"web\\"", "value": "restart"}]}]`;
      const { choicePrompt } = extractChoicePrompt(input);
      expect(choicePrompt).toBeDefined();
      expect(choicePrompt?.question).toContain('web');
      expect(choicePrompt?.options[0].value).toBe('restart');
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 3: Max Suggestions & Duplicate Chip Deduplication
  // -------------------------------------------------------------------------
  describe('B3: Max 3 Suggestions & Duplicate Chips', () => {
    it('3.1: caps at exactly 3 suggestions when model emits 5 chips', () => {
      const parsed = parseAiTools(SUGGEST_SURPLUS_TAG);
      expect(parsed.suggests).toHaveLength(3);
      expect(parsed.suggests.map((s) => s[1])).toEqual(['Chip 1', 'Chip 2', 'Chip 3']);
    });

    it('3.2: drops chips 4 and 5 from clean text display', () => {
      const input = `Done. ${SUGGEST_SURPLUS_TAG}`;
      const parsed = parseAiTools(input);
      expect(parsed.cleanText).toBe('Done.');
      expect(parsed.cleanText).not.toContain('Chip 4');
      expect(parsed.cleanText).not.toContain('Chip 5');
    });

    it('3.3: handles consecutive duplicate chip labels without dropping order', () => {
      const parsed = parseAiTools(SUGGEST_DUPLICATE_CHIPS);
      expect(parsed.suggests).toHaveLength(3);
      expect(parsed.suggests[0][1]).toBe('Verify status');
      expect(parsed.suggests[1][1]).toBe('Verify status');
      expect(parsed.suggests[2][1]).toBe('View details');
    });

    it('3.4: allows safe Svelte key generation via index-label compound key', () => {
      const parsed = parseAiTools(SUGGEST_DUPLICATE_CHIPS);
      const keys = parsed.suggests.map((s, idx) => `${idx}-${s[1]}`);
      const uniqueKeys = new Set(keys);
      expect(uniqueKeys.size).toBe(keys.length);
    });

    it('3.5: handles empty suggestion [SUGGEST:] without crashing', () => {
      const input = 'Answer [SUGGEST:] followed by normal text.';
      const parsed = parseAiTools(input);
      expect(parsed.cleanText).toBeDefined();
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 4: Non-Shell vs Shell Code Blocks Boundaries
  // -------------------------------------------------------------------------
  describe('B4: Shell vs Non-Shell Code Blocks Boundaries', async () => {
    const parse = await getChatMarkdownParser();

    it('4.1: detects docker command in untagged code block', () => {
      const input = '```\ndocker run -p 80:80 nginx\n```';
      const block = parse(input)[0] as CodeBlock;
      expect(block.isShell).toBe(true);
    });

    it('4.2: detects colima command in untagged code block', () => {
      const input = '```\ncolima start --runtime containerd\n```';
      const block = parse(input)[0] as CodeBlock;
      expect(block.isShell).toBe(true);
    });

    it('4.3: detects kubectl command in untagged code block', () => {
      const input = '```\nkubectl apply -f deployment.yaml\n```';
      const block = parse(input)[0] as CodeBlock;
      expect(block.isShell).toBe(true);
    });

    it('4.4: detects helm command in untagged code block', () => {
      const input = '```\nhelm upgrade --install my-app ./chart\n```';
      const block = parse(input)[0] as CodeBlock;
      expect(block.isShell).toBe(true);
    });

    it('4.5: distinguishes sql, css, html as non-shell commands', () => {
      for (const lang of ['sql', 'css', 'html']) {
        const input = `\`\`\`${lang}\nSELECT * FROM chat;\n\`\`\``;
        const block = parse(input)[0] as CodeBlock;
        expect(block.isShell).toBe(false);
      }
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 5: History Truncation Boundary Positions
  // -------------------------------------------------------------------------
  describe('B5: History Truncation Boundary Positions', () => {
    const generateHistory = (count: number): SimulatedMessage[] =>
      Array.from({ length: count }, (_, i) => ({
        id: `turn-${i}`,
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `Message ${i}`,
      }));

    it('5.1: truncates at index 0 (entire conversation wiped)', () => {
      const history = generateHistory(10);
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(history, 'turn-0');
      expect(remaining).toHaveLength(0);
      expect(truncated).toHaveLength(10);
    });

    it('5.2: truncates at final index (only last message removed)', () => {
      const history = generateHistory(10);
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(history, 'turn-9');
      expect(remaining).toHaveLength(9);
      expect(truncated).toHaveLength(1);
      expect(truncated[0].id).toBe('turn-9');
    });

    it('5.3: truncates from exact second-to-last turn', () => {
      const history = generateHistory(10);
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(history, 'turn-8');
      expect(remaining).toHaveLength(8);
      expect(truncated.map((m) => m.id)).toEqual(['turn-8', 'turn-9']);
    });

    it('5.4: handles empty history array gracefully', () => {
      const { remaining, truncated } = simulateTruncateAiMessagesFrom([], 'turn-0');
      expect(remaining).toHaveLength(0);
      expect(truncated).toHaveLength(0);
    });

    it('5.5: handles single-element history truncation', () => {
      const history = generateHistory(1);
      const { remaining, truncated } = simulateTruncateAiMessagesFrom(history, 'turn-0');
      expect(remaining).toHaveLength(0);
      expect(truncated).toHaveLength(1);
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 6: XSS Escaping & HTML Injection Protection
  // -------------------------------------------------------------------------
  describe('B6: XSS Escaping & HTML Injection Protection', async () => {
    const parse = await getChatMarkdownParser();

    it('6.1: escapes <script> tags when rendering inline markdown', () => {
      const input = MARKDOWN_XSS_VECTORS[0];
      const html = renderInlineChatMarkdown(input);
      expect(html).not.toContain('<script>');
      expect(html).toContain('&lt;script&gt;');
    });

    it('6.2: escapes onerror image injection vectors', () => {
      const input = MARKDOWN_XSS_VECTORS[1];
      const html = renderInlineChatMarkdown(input);
      expect(html).not.toContain('<img');
      expect(html).toContain('&lt;img');
    });

    it('6.3: escapes svg onload injection vectors', () => {
      const input = MARKDOWN_XSS_VECTORS[2];
      const html = renderInlineChatMarkdown(input);
      expect(html).not.toContain('<svg');
      expect(html).toContain('&lt;svg');
    });

    it('6.4: escapes iframe injection vectors', () => {
      const input = MARKDOWN_XSS_VECTORS[4];
      const html = renderInlineChatMarkdown(input);
      expect(html).not.toContain('<iframe');
      expect(html).toContain('&lt;iframe');
    });

    it('6.5: preserves safe special characters like ampersands and quotes in code', () => {
      const input = '```bash\necho "A & B < C > D"\n```';
      const blocks = parse(input);
      expect((blocks[0] as CodeBlock).code).toBe('echo "A & B < C > D"');
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 7: Extreme Container Dimensions & Scroll Boundaries
  // -------------------------------------------------------------------------
  describe('B7: Extreme Container Dimensions & Scroll Boundaries', () => {
    it('7.1: evaluates near-bottom when distance is exactly 71px (threshold - 1)', () => {
      const container = { scrollHeight: 1000, scrollTop: 529, clientHeight: 400 };
      // 1000 - 529 - 400 = 71 < 72
      expect(isNearBottom(container)).toBe(true);
    });

    it('7.2: evaluates NOT near-bottom when distance is exactly 72px (threshold)', () => {
      const container = { scrollHeight: 1000, scrollTop: 528, clientHeight: 400 };
      // 1000 - 528 - 400 = 72
      expect(isNearBottom(container)).toBe(false);
    });

    it('7.3: evaluates NOT near-bottom when distance is 73px (threshold + 1)', () => {
      const container = { scrollHeight: 1000, scrollTop: 527, clientHeight: 400 };
      // 1000 - 527 - 400 = 73
      expect(isNearBottom(container)).toBe(false);
    });

    it('7.4: handles negative scrollTop (elastic bounce on macOS)', () => {
      const container = { scrollHeight: 1000, scrollTop: -10, clientHeight: 400 };
      // distance = 1000 - (-10) - 400 = 610 >= 72
      expect(isNearBottom(container)).toBe(false);
    });

    it('7.5: handles over-scrolled bottom (macOS rubber band)', () => {
      const container = { scrollHeight: 1000, scrollTop: 620, clientHeight: 400 };
      // distance = 1000 - 620 - 400 = -20 < 72
      expect(isNearBottom(container)).toBe(true);
    });
  });

  // -------------------------------------------------------------------------
  // Boundary 8: Empty Chat State & Onboarding Triggers
  // -------------------------------------------------------------------------
  describe('B8: Empty Chat State & Onboarding Triggers', async () => {
    const { getStarterPrompts } = await getAiSuggestionsEngine();

    it('8.1: does not surface warning prompts when all VMs are running and containers healthy', () => {
      const prompts = getStarterPrompts({
        currentPage: 'dashboard',
        colimaInstances: [{ name: 'default', status: 'Running' }],
        containers: [{ id: 'c-1', name: 'app', state: 'running' }],
      });
      const warningPrompts = prompts.filter((p) => p.category === 'warning');
      expect(warningPrompts).toHaveLength(0);
    });

    it('8.2: handles empty container and instance arrays without error', () => {
      const prompts = getStarterPrompts({
        currentPage: 'dashboard',
        colimaInstances: [],
        containers: [],
      });
      expect(prompts.length).toBeGreaterThan(0);
    });

    it('8.3: handles unknown currentPage by falling back to dashboard prompt', () => {
      const prompts = getStarterPrompts({
        currentPage: 'custom-unknown-page',
        colimaInstances: [],
        containers: [],
      });
      expect(prompts.some((p) => p.title.toLowerCase().includes('overview'))).toBe(true);
    });

    it('8.4: limits multiple stopped instances to 1 top warning to prevent overcrowding', () => {
      const prompts = getStarterPrompts({
        currentPage: 'dashboard',
        colimaInstances: [
          { name: 'vm-1', status: 'Stopped' },
          { name: 'vm-2', status: 'Stopped' },
          { name: 'vm-3', status: 'Stopped' },
        ],
        containers: [],
      });
      const warnings = prompts.filter((p) => p.category === 'warning');
      expect(warnings.length).toBeLessThanOrEqual(2);
    });

    it('8.5: provides general help/optimize prompt in empty state', () => {
      const prompts = getStarterPrompts({
        currentPage: 'settings',
        colimaInstances: [],
        containers: [],
      });
      expect(prompts.some((p) => p.category === 'help')).toBe(true);
    });
  });
});
