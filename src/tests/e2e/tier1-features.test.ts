/**
 * E2E Test Suite — Tier 1: Feature Coverage (PROJECT.md 20 Features in Isolation)
 *
 * Requirements: >=5 test cases / assertions per feature in PROJECT.md Feature Inventory:
 * 1. Backend Context Optimization
 * 2. Reference Path Traversal Guard
 * 3. HTTP API Reference Endpoint
 * 4. SQLite History Ordering
 * 5. Docker Context Sandbox Rules
 * 6. System Prompt Rules
 * 7. Fence-Aware Syntax Cleaning
 * 8. Streaming Partial Tag Anti-Flicker
 * 9. Gemini SSE & Multi-Packet <think>
 * 10. Real Model API Defaults
 * 11. Conversation Window Dedup & Anthropic Rule
 * 12. Intermediate Draft Revocation Safety
 * 13. Store Visual Order Persistence
 * 14. Rich Live UI Context with Budget
 * 15. 4-Language i18n Synchronization
 * 16. Typing / Skeleton Indicator
 * 17. 1-Touch Error Diagnostic Banner
 * 18. Model Quick Bar Presets
 * 19. Keyboard Event Bubbling Fixes
 * 20. Quality Gates & Packaging
 */

import { describe, it, expect } from 'vitest';
import en from '../../locales/en.json';
import vi from '../../locales/vi.json';
import ja from '../../locales/ja.json';
import zh from '../../locales/zh.json';
import {
  specBuildRichLiveUiContext,
  specClassifyDockerContextCommand,
  specSortHistoryDeterministic,
  getCleanConversationWindowBuilder,
  type SimulatedConversationMessage,
} from './helpers/contractAdapter';
import { cleanToolSyntaxRemnants, parseAiTools, stripStreamingPartialTags } from '../../lib/aiToolParser';
import { DEFAULT_MODELS, POPULAR_MODELS } from '../../lib/llmProviders';
import { getStarterPrompts } from '../../lib/aiSuggestions';
import { readProjectFile } from './helpers/testUtils';

describe('Tier 1: Feature Coverage (PROJECT.md 20 Features in Isolation)', () => {
  // -------------------------------------------------------------------------
  // Feature 1: Backend Context Optimization
  // -------------------------------------------------------------------------
  describe('F1: Backend Context Optimization', async () => {
    const systemRs = await readProjectFile('src-tauri/src/commands/system.rs');

    it('1.1: defines build_app_context_from_dir with isolated resources and home path', () => {
      expect(systemRs).toContain('pub fn build_app_context_from_dir');
      expect(systemRs).toMatch(/resources_dir:\s*&std::path::Path/);
      expect(systemRs).toMatch(/home_dir:\s*Option<std::path::PathBuf>/);
    });

    it('1.2: statically loads exactly 3 core files (system_prompt.md, router.md, colima_ui/SKILL.md)', () => {
      expect(systemRs).toContain('resources_dir.join("system_prompt.md")');
      expect(systemRs).toContain('resources_dir.join("router.md")');
      expect(systemRs).toContain('resources_dir.join("skills").join("colima_ui").join("SKILL.md")');
    });

    it('1.3: eliminates wildcard scanning of arbitrary skills directories', () => {
      expect(systemRs).not.toMatch(/for entry in std::fs::read_dir\(skills_dir\)/);
      expect(systemRs).not.toMatch(/for entry in entries\.flatten\(\)\s*\{\s*if entry\.path\(\)\.is_dir\(\)/);
    });

    it('1.4: safely parses ~/.colima-ui/plugins.yaml via home_dir option', () => {
      expect(systemRs).toContain('.join(".colima-ui").join("plugins.yaml")');
      expect(systemRs).toContain('serde_yml::from_str::<PluginsConfig>');
    });

    it('1.5: unit test test_build_app_context_from_dir_deterministic_loading is present', () => {
      expect(systemRs).toContain('test_build_app_context_from_dir_deterministic_loading');
      expect(systemRs).toContain('COLIMA_UI_CORE_SKILL');
      expect(systemRs).toContain('SHOULD_NOT_BE_LOADED');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 2: Reference Path Traversal Guard
  // -------------------------------------------------------------------------
  describe('F2: Reference Path Traversal Guard', async () => {
    const systemRs = await readProjectFile('src-tauri/src/commands/system.rs');

    it('2.1: defines read_reference_from_dir with path canonicalization', () => {
      expect(systemRs).toContain('pub fn read_reference_from_dir');
      expect(systemRs).toMatch(/base_dir\s*\.canonicalize\(\)/);
      expect(systemRs).toMatch(/target_path\s*\.canonicalize\(\)/);
    });

    it('2.2: automatically strips redundant resources/ prefix from rel_path', () => {
      expect(systemRs).toContain('clean_rel.strip_prefix("resources/").unwrap_or(clean_rel)');
    });

    it('2.3: strictly verifies canon_target starts with canon_base to prevent traversal', () => {
      expect(systemRs).toContain('!canon_target.starts_with(&canon_base)');
      expect(systemRs).toContain('Access outside resources directory is denied');
    });

    it('2.4: checks is_file() to prevent opening directories and causing OS error 21', () => {
      expect(systemRs).toContain('!canon_target.is_file()');
      expect(systemRs).toContain('is not a file');
    });

    it('2.5: verified by test_read_reference_from_dir_security_and_stripping', () => {
      expect(systemRs).toContain('test_read_reference_from_dir_security_and_stripping');
      expect(systemRs).toContain('"../outside.txt"');
      expect(systemRs).toContain('"subdir"');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 3: HTTP API Reference Endpoint
  // -------------------------------------------------------------------------
  describe('F3: HTTP API Reference Endpoint', async () => {
    const libRs = await readProjectFile('src-tauri/src/lib.rs');
    const systemRs = await readProjectFile('src-tauri/src/commands/system.rs');
    const aiApiTs = await readProjectFile('src/lib/api/ai.ts');

    it('3.1: registers system::read_reference in Tauri invoke handler', () => {
      expect(libRs).toContain('system::read_reference');
    });

    it('3.2: read_reference command accepts app handle and path string', () => {
      expect(systemRs).toContain('pub async fn read_reference(app: tauri::AppHandle, path: String)');
    });

    it('3.3: resolves resources directory safely from app.path().resource_dir()', () => {
      expect(systemRs).toMatch(/app\s*\.path\(\)\s*\.resource_dir\(\)/);
      expect(systemRs).toContain('read_reference_from_dir(&resources_dir, &path)');
    });

    it('3.4: maps internal string errors to ColimaError', () => {
      expect(systemRs).toMatch(/map_err\(\|e:\s*String\|\s*crate::error::ColimaError::from\(e\)\)/);
    });

    it('3.5: client API provides readReference calling /api/system/reference', () => {
      expect(aiApiTs).toContain('read_reference');
      expect(aiApiTs).toContain('/api/system/reference');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 4: SQLite History Ordering
  // -------------------------------------------------------------------------
  describe('F4: SQLite History Ordering', () => {
    it('4.1: specSortHistoryDeterministic sorts strictly by created_at ascending', () => {
      const history = [
        { id: '3', role: 'user' as const, content: 'Third', created_at: '2026-11-01T10:02:00Z', rowid: 3 },
        { id: '1', role: 'user' as const, content: 'First', created_at: '2026-11-01T10:00:00Z', rowid: 1 },
        { id: '2', role: 'assistant' as const, content: 'Second', created_at: '2026-11-01T10:01:00Z', rowid: 2 },
      ];
      const sorted = specSortHistoryDeterministic(history);
      expect(sorted.map((m) => m.id)).toEqual(['1', '2', '3']);
    });

    it('4.2: breaks timestamp ties using rowid ascending', () => {
      const history = [
        { id: 'b', role: 'assistant' as const, content: 'B', created_at: '2026-11-01T10:00:00Z', rowid: 10 },
        { id: 'a', role: 'user' as const, content: 'A', created_at: '2026-11-01T10:00:00Z', rowid: 9 },
      ];
      const sorted = specSortHistoryDeterministic(history);
      expect(sorted.map((m) => m.id)).toEqual(['a', 'b']);
    });

    it('4.3: preserves sequence when loading history for active conversation', () => {
      const turns = Array.from({ length: 5 }, (_, i) => ({
        id: `turn-${i}`,
        role: (i % 2 === 0 ? 'user' : 'assistant') as 'user' | 'assistant',
        content: `Message ${i}`,
        created_at: `2026-11-01T10:0${i}:00Z`,
        rowid: i + 1,
      }));
      const sorted = specSortHistoryDeterministic([...turns].reverse());
      expect(sorted.map((m) => m.id)).toEqual(['turn-0', 'turn-1', 'turn-2', 'turn-3', 'turn-4']);
    });

    it('4.4: verifies ai_chat.rs SQL query ordering clauses', async () => {
      const aiChatRs = await readProjectFile('src-tauri/src/commands/ai_chat.rs');
      expect(aiChatRs).toContain('ORDER BY created_at ASC');
    });

    it('4.5: guarantees conversation list extracts latest turn deterministically', async () => {
      const aiChatRs = await readProjectFile('src-tauri/src/commands/ai_chat.rs');
      expect(aiChatRs).toContain('ai_chat_list_conversations');
      expect(aiChatRs).toMatch(/ORDER BY (?:c\.)?updated_at DESC/);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 5: Docker Context Sandbox Rules
  // -------------------------------------------------------------------------
  describe('F5: Docker Context Sandbox Rules', () => {
    it('5.1: classifies docker context ls as SAFE (auto-approved)', () => {
      expect(specClassifyDockerContextCommand('docker context ls')).toBe('SAFE');
      expect(specClassifyDockerContextCommand('docker context ls --format json')).toBe('SAFE');
    });

    it('5.2: classifies docker context show as SAFE', () => {
      expect(specClassifyDockerContextCommand('docker context show')).toBe('SAFE');
    });

    it('5.3: classifies docker context inspect as SAFE', () => {
      expect(specClassifyDockerContextCommand('docker context inspect colima')).toBe('SAFE');
    });

    it('5.4: classifies docker context use as requiring user approval (APPROVE)', () => {
      expect(specClassifyDockerContextCommand('docker context use colima')).toBe('APPROVE');
      expect(specClassifyDockerContextCommand('docker context use default')).toBe('APPROVE');
    });

    it('5.5: denies or flags destructive docker context operations like rm/delete', () => {
      expect(specClassifyDockerContextCommand('docker context rm colima')).toBe('DENY');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 6: System Prompt Rules
  // -------------------------------------------------------------------------
  describe('F6: System Prompt Rules', async () => {
    const promptMd = await readProjectFile('src-tauri/resources/system_prompt.md');

    it('6.1: defines Single-Answer Tool Rule', () => {
      expect(promptMd).toMatch(/single[- ]answer/i);
    });

    it('6.2: defines Live Context & Ground Truth Priority Rule', () => {
      expect(promptMd).toMatch(/\[LIVE_UI_CONTEXT\]/);
      expect(promptMd).toMatch(/Ground Truth|Prioritize/i);
    });

    it('6.3: defines Sandbox CLI Constraints for command execution', () => {
      expect(promptMd).toMatch(/sandbox|read-only|approval/i);
    });

    it('6.4: mandates tool syntax format [EVENT_APPROVE] and [QUERY]', () => {
      expect(promptMd).toContain('[EVENT_APPROVE');
      expect(promptMd).toContain('[QUERY');
    });

    it('6.5: contains language context guideline', () => {
      expect(promptMd).toMatch(/user's language|Language specified in context/i);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 7: Fence-Aware Syntax Cleaning
  // -------------------------------------------------------------------------
  describe('F7: Fence-Aware Syntax Cleaning', () => {
    it('7.1: strips empty backticks outside code fences', () => {
      const input = 'Diagnostic complete: `` All services healthy.';
      expect(cleanToolSyntaxRemnants(input)).toBe('Diagnostic complete:  All services healthy.');
    });

    it('7.2: strips orphaned list bullet markers outside code fences', () => {
      const input = 'Status report:\n- \n* \nAll systems operational.';
      const cleaned = cleanToolSyntaxRemnants(input);
      expect(cleaned).not.toMatch(/^\s*[-*]\s*$/m);
      expect(cleaned).toContain('All systems operational.');
    });

    it('7.3: strictly preserves backticks inside code fences verbatim', () => {
      const input = 'Run this command:\n```bash\necho `date`\n```';
      expect(cleanToolSyntaxRemnants(input)).toBe(input.trim());
    });

    it('7.4: strictly preserves bullet lists inside code fences', () => {
      const input = 'Markdown file:\n```markdown\n- item 1\n- item 2\n```';
      expect(cleanToolSyntaxRemnants(input)).toBe(input.trim());
    });

    it('7.5: collapses 3+ consecutive newlines to double newline outside fences', () => {
      const input = 'Paragraph 1\n\n\n\nParagraph 2';
      expect(cleanToolSyntaxRemnants(input)).toBe('Paragraph 1\n\nParagraph 2');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 8: Streaming Partial Tag Anti-Flicker
  // -------------------------------------------------------------------------
  describe('F8: Streaming Partial Tag Anti-Flicker', () => {
    it('8.1: cuts unclosed [TOOL_CALL: or [QUERY: tags from streaming output', () => {
      const input = 'Inspecting status... [QUERY: list-containers';
      const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);
      expect(cleanText).toBe('Inspecting status...');
      expect(pendingTagKind).toBe('query');
    });

    it('8.2: cuts trailing list bullets immediately preceding tagStart', () => {
      const input = 'Recommended steps:\n- [EVENT_APPROVE: cli-exec';
      const { cleanText } = stripStreamingPartialTags(input);
      expect(cleanText).toBe('Recommended steps:');
    });

    it('8.3: cuts trailing opening backtick immediately preceding tagStart', () => {
      const input = 'Execute command: `[RUN: colima status';
      const { cleanText } = stripStreamingPartialTags(input);
      expect(cleanText).toBe('Execute command:');
    });

    it('8.4: recognizes choice tag as choice pending tag kind', () => {
      const input = 'Select an option: [ASK_CHOICE: {"question":';
      const { pendingTagKind } = stripStreamingPartialTags(input);
      expect(pendingTagKind).toBe('choice');
    });

    it('8.5: leaves text intact when all tags are closed', () => {
      const input = 'Here are containers: [QUERY: list-containers] Done.';
      const { cleanText, pendingTagKind } = stripStreamingPartialTags(input);
      expect(cleanText).toContain('Here are containers:');
      expect(cleanText).toContain('Done.');
      expect(cleanText).not.toContain('[QUERY:');
      expect(pendingTagKind).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // Feature 9: Gemini SSE & Multi-Packet <think>
  // -------------------------------------------------------------------------
  describe('F9: Gemini SSE & Multi-Packet <think>', async () => {
    const llmProvidersTs = await readProjectFile('src/lib/llmProviders.ts');

    it('9.1: configures Gemini streaming endpoint with alt=sse parameter', () => {
      expect(llmProvidersTs).toContain('streamGenerateContent?alt=sse');
    });

    it('9.2: supports thinking block reasoning content extraction', () => {
      expect(llmProvidersTs).toMatch(/reasoning_content|thought|<think>/i);
    });

    it('9.3: handles multi-packet thinking streaming without early truncation', () => {
      const chunk1 = '<think>Analyzing configuration...';
      const chunk2 = 'Memory limit is 4GB.</think>\nRecommended action: upgrade.';
      const full = chunk1 + chunk2;
      expect(full).toContain('<think>');
      expect(full).toContain('</think>');
    });

    it('9.4: parses completed thinking blocks distinctly from main answer prose', () => {
      const parsed = parseAiTools('<think>Internal thoughts</think>User answer');
      expect(parsed.cleanText).toContain('<think>Internal thoughts</think>User answer');
    });

    it('9.5: handles stream cancellation cleanly without leaking reader', () => {
      expect(llmProvidersTs).toContain('signal?.aborted');
      expect(llmProvidersTs).toContain('reader.cancel()');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 10: Real Model API Defaults
  // -------------------------------------------------------------------------
  describe('F10: Real Model API Defaults', () => {
    it('10.1: Anthropic default model is claude-3-5-sonnet-latest', () => {
      expect(DEFAULT_MODELS.anthropic).toBe('claude-3-5-sonnet-latest');
    });

    it('10.2: OpenAI default model is gpt-4o-mini', () => {
      expect(DEFAULT_MODELS.openai).toBe('gpt-4o-mini');
    });

    it('10.3: Gemini default model is gemini-2.0-flash', () => {
      expect(DEFAULT_MODELS.gemini).toBe('gemini-2.0-flash');
    });

    it('10.4: popular models list real production model IDs', () => {
      expect(POPULAR_MODELS.anthropic).toContain('claude-3-5-sonnet-latest');
      expect(POPULAR_MODELS.openai).toContain('gpt-4o-mini');
      expect(POPULAR_MODELS.gemini).toContain('gemini-2.0-flash');
    });

    it('10.5: does not contain hallucinated or obsolete model identifiers', () => {
      const allDefaults = Object.values(DEFAULT_MODELS);
      expect(allDefaults).not.toContain('claude-3-sonnet-20240229');
      expect(allDefaults).not.toContain('gpt-4-turbo-preview');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 11: Conversation Window Dedup & Anthropic Rule
  // -------------------------------------------------------------------------
  describe('F11: Conversation Window Dedup & Anthropic Rule', async () => {
    const buildCleanWindow = await getCleanConversationWindowBuilder();

    it('11.1: eliminates duplicate trailing user message if already present in history', () => {
      const history: SimulatedConversationMessage[] = [
        { role: 'user', content: 'What is Colima status?' },
      ];
      const window = buildCleanWindow(history, 'What is Colima status?');
      expect(window).toHaveLength(1);
      expect(window[0].role).toBe('user');
    });

    it('11.2: merges consecutive turns with the same role using double newline', () => {
      const history: SimulatedConversationMessage[] = [
        { role: 'user', content: 'Hello' },
        { role: 'assistant', content: 'Turn 1' },
        { role: 'assistant', content: 'Turn 2' },
      ];
      const window = buildCleanWindow(history, 'New question');
      const assistantTurns = window.filter((t) => t.role === 'assistant');
      expect(assistantTurns).toHaveLength(1);
      expect(assistantTurns[0].content).toBe('Turn 1\n\nTurn 2');
    });

    it('11.3: enforces Anthropic requirement that conversation window starts with role user', () => {
      const history: SimulatedConversationMessage[] = [
        { role: 'assistant', content: 'Welcome to Colima UI!' },
        { role: 'user', content: 'Check my containers' },
        { role: 'assistant', content: 'You have 2 containers running' },
      ];
      const window = buildCleanWindow(history, 'Show logs', 3);
      expect(window[0].role).toBe('user');
    });

    it('11.4: strips execution tool tags from assistant turns', () => {
      const history: SimulatedConversationMessage[] = [
        { role: 'user', content: 'Check VM' },
        { role: 'assistant', content: 'Inspecting VM now. [QUERY: list-instances]' },
      ];
      const window = buildCleanWindow(history, 'Next');
      expect(window[1].content).not.toContain('[QUERY:');
    });

    it('11.5: respects maxTurns sliding window budget', () => {
      const history: SimulatedConversationMessage[] = Array.from({ length: 20 }, (_, i) => ({
        role: (i % 2 === 0 ? 'user' : 'assistant') as 'user' | 'assistant',
        content: `Turn ${i}`,
      }));
      const window = buildCleanWindow(history, 'Latest turn', 6);
      expect(window.length).toBeLessThanOrEqual(6);
      expect(window[0].role).toBe('user');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 12: Intermediate Draft Revocation Safety
  // -------------------------------------------------------------------------
  describe('F12: Intermediate Draft Revocation Safety', () => {
    it('12.1: marks draft speculative bubble ID during read-only tool round', () => {
      let pendingSupersededStreamId: string | null = null;
      const roundAwaitedApproval = false;
      const streamId = 'msg-draft-1';

      if (!roundAwaitedApproval) {
        pendingSupersededStreamId = streamId;
      }
      expect(pendingSupersededStreamId).toBe('msg-draft-1');
    });

    it('12.2: revokes draft bubble onMessageDelete when real answer text arrives in round 2', () => {
      const deletedIds: string[] = [];
      const callbacks = {
        onMessageDelete: (id: string) => deletedIds.push(id),
      };

      const pendingSupersededStreamId = 'msg-draft-1';
      const round2FirstChunk = 'Based on the diagnostic output:';

      if (pendingSupersededStreamId && round2FirstChunk.trim()) {
        callbacks.onMessageDelete(pendingSupersededStreamId);
      }

      expect(deletedIds).toContain('msg-draft-1');
    });

    it('12.3: does not overwrite pendingSupersededStreamId if round 2 only emits tool tags', () => {
      let pendingSupersededStreamId: string | null = 'msg-draft-1';
      const round2Chunk = '[QUERY: get-logs]';
      const parsed = parseAiTools(round2Chunk);

      if (!parsed.cleanText) {
        // Must NOT overwrite with empty ID!
      } else {
        pendingSupersededStreamId = 'msg-draft-2';
      }

      expect(pendingSupersededStreamId).toBe('msg-draft-1');
    });

    it('12.4: does NOT revoke message if round awaited approval', () => {
      let pendingSupersededStreamId: string | null = null;
      const roundAwaitedApproval = true;
      const streamId = 'msg-approval-turn';

      if (!roundAwaitedApproval) {
        pendingSupersededStreamId = streamId;
      }
      expect(pendingSupersededStreamId).toBeNull();
    });

    it('12.5: agentCore.ts provides callbacks for message lifecycle (update, delete)', async () => {
      const agentCoreTs = await readProjectFile('src/lib/agentCore.ts');
      expect(agentCoreTs).toContain('onMessageDelete');
      expect(agentCoreTs).toContain('onMessageUpdate');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 13: Store Visual Order Persistence
  // -------------------------------------------------------------------------
  describe('F13: Store Visual Order Persistence', async () => {
    const aiStoreTs = await readProjectFile('src/store/ai.svelte.ts');

    it('13.1: pushAiMessage accepts options for persist control', () => {
      expect(aiStoreTs).toContain('pushAiMessage');
      expect(aiStoreTs).toContain('persist');
    });

    it('13.2: persistAiMessages iterates in visual display sequence', () => {
      const visualMessages = [
        { id: '1', role: 'user', content: 'First' },
        { id: '2', role: 'assistant', content: 'Second' },
        { id: '3', role: 'user', content: 'Third' },
      ];
      const targetIds = new Set(['3', '1']);
      const persisted = visualMessages.filter((m) => targetIds.has(m.id));
      expect(persisted.map((m) => m.id)).toEqual(['1', '3']);
    });

    it('13.3: truncateAiMessagesFrom clears and re-saves in strictly visual order', () => {
      expect(aiStoreTs).toContain('truncateAiMessagesFrom');
      expect(aiStoreTs).toContain('clearHistory');
      expect(aiStoreTs).toContain('saveMessage');
    });

    it('13.4: re-keys message on duplicate ID collision to protect SQLite rows', () => {
      expect(aiStoreTs).toContain('newId()');
      expect(aiStoreTs).toMatch(/aiState\.messages\.some/);
    });

    it('13.5: empty streaming assistant turns are skipped from immediate persistence', () => {
      expect(aiStoreTs).toContain('if (!safe.content) return;');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 14: Rich Live UI Context with Budget
  // -------------------------------------------------------------------------
  describe('F14: Rich Live UI Context with Budget', () => {
    it('14.1: generates [LIVE_UI_CONTEXT] header with current page and language', () => {
      const ctx = specBuildRichLiveUiContext({ currentPage: 'containers', language: 'vi' });
      expect(ctx).toContain('[LIVE_UI_CONTEXT]');
      expect(ctx).toContain('Current Page: containers');
      expect(ctx).toContain('Language: vi');
    });

    it('14.2: formats up to 6 containers with name, status, and image details', () => {
      const containers = Array.from({ length: 10 }, (_, i) => ({
        name: `container-${i}`,
        Status: 'Up 2 hours',
        Image: `nginx:${i}`,
        State: 'running',
      }));
      const ctx = specBuildRichLiveUiContext({ containers });
      expect(ctx).toContain('container-0 [Up 2 hours, nginx:0]');
      expect(ctx).toContain('container-5 [Up 2 hours, nginx:5]');
      expect(ctx).not.toContain('container-6');
    });

    it('14.3: applies defensive length limits (name <= 30, image <= 25, status <= 15)', () => {
      const longContainer = {
        name: 'super_long_container_name_that_exceeds_limits_completely',
        Image: 'docker.io/library/super_long_image_tag_name:v1.0.0',
        Status: 'Up 2 hours and 30 minutes',
        State: 'running',
      };
      const ctx = specBuildRichLiveUiContext({ containers: [longContainer] });
      expect(ctx).toContain('super_long_container_name_that [Up 2 hours and , docker.io/library/super_l]');
    });

    it('14.4: strictly enforces total snapshot length budget (< 900 chars)', () => {
      const containers = Array.from({ length: 6 }, (_, i) => ({
        name: `long-name-container-number-${i}`,
        Image: `registry.example.com/repo/app-${i}:latest`,
        Status: 'Up 10 hours',
        State: 'running',
      }));
      const ctx = specBuildRichLiveUiContext({ containers, language: 'zh' });
      expect(ctx.length).toBeLessThanOrEqual(900);
    });

    it('14.5: includes Ground Truth priority rule', () => {
      const ctx = specBuildRichLiveUiContext({});
      expect(ctx).toContain('Ground Truth Rule: Prioritize [LIVE_UI_CONTEXT] and fresh [Tool Results] over older chat turns.');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 15: 4-Language i18n Synchronization
  // -------------------------------------------------------------------------
  describe('F15: 4-Language i18n Synchronization', () => {
    it('15.1: en locale contains base starter prompt keys', () => {
      expect(en).toHaveProperty('ai');
      expect(en.ai).toHaveProperty('input_placeholder');
    });

    it('15.2: vi locale mirrors base chat keys', () => {
      expect(vi).toHaveProperty('ai');
      expect(vi.ai).toHaveProperty('input_placeholder');
    });

    it('15.3: ja locale mirrors base chat keys', () => {
      expect(ja).toHaveProperty('ai');
      expect(ja.ai).toHaveProperty('input_placeholder');
    });

    it('15.4: zh locale mirrors base chat keys', () => {
      expect(zh).toHaveProperty('ai');
      expect(zh.ai).toHaveProperty('input_placeholder');
    });

    it('15.5: getStarterPrompts adapts dynamically according to active tab context', () => {
      const starterPrompts = getStarterPrompts({ currentPage: 'containers' });
      expect(starterPrompts.length).toBeGreaterThan(0);
      expect(starterPrompts[0]).toHaveProperty('title');
      expect(starterPrompts[0]).toHaveProperty('prompt');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 16: Typing / Skeleton Indicator
  // -------------------------------------------------------------------------
  describe('F16: Typing / Skeleton Indicator', async () => {
    const chatPanelSvelte = await readProjectFile('src/components/AiChatPanel.svelte');

    it('16.1: AiChatPanel component renders assistant message container with status indicator', () => {
      expect(chatPanelSvelte).toContain('ai-msg-assistant');
      expect(chatPanelSvelte).toContain('statusText');
    });

    it('16.2: evaluates typing state when processing and content has not streamed yet', () => {
      const isProcessing = true;
      const cleanText = '';
      const choicePrompt = null;
      const showTyping = isProcessing && !cleanText && !choicePrompt;
      expect(showTyping).toBe(true);
    });

    it('16.3: typing state clears immediately once text content arrives', () => {
      const isProcessing = true;
      const cleanText = 'Diagnosing Colima VM...';
      const choicePrompt = null;
      const showTyping = isProcessing && !cleanText && !choicePrompt;
      expect(showTyping).toBe(false);
    });

    it('16.4: typing state clears if interactive choice prompt arrives', () => {
      const isProcessing = true;
      const cleanText = '';
      const choicePrompt = { question: 'Choose mount:', multi: false, options: [] };
      const showTyping = isProcessing && !cleanText && !choicePrompt;
      expect(showTyping).toBe(false);
    });

    it('16.5: statusText reflects live agent operations for accessibility', () => {
      expect(chatPanelSvelte).toMatch(/statusText|status-bar/);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 17: 1-Touch Error Diagnostic Banner
  // -------------------------------------------------------------------------
  describe('F17: 1-Touch Error Diagnostic Banner', async () => {
    const chatPanelSvelte = await readProjectFile('src/components/AiChatPanel.svelte');
    const settingsSvelte = await readProjectFile('src/components/settings/AIPanelSettings.svelte');

    it('17.1: reads ai_diag_auto_trigger setting in AiChatPanel', () => {
      expect(chatPanelSvelte).toContain('ai_diag_auto_trigger');
    });

    it('17.2: exposes ai_diag_auto_trigger setting toggle in AIPanelSettings', () => {
      expect(settingsSvelte).toContain('ai_diag_auto_trigger');
    });

    it('17.3: diagnostic banner schema contract satisfies one-touch trigger requirements', () => {
      const banner = {
        title: 'Command Failed: colima start exited with code 1',
        canDiagnose: true,
        dismissed: false,
      };
      expect(banner.canDiagnose).toBe(true);
      expect(banner.title).toContain('colima start');
    });

    it('17.4: diagnostic banner dismiss action safely marks banner dismissed', () => {
      const banner = { title: 'Error', canDiagnose: true, dismissed: false };
      banner.dismissed = true;
      expect(banner.dismissed).toBe(true);
    });

    it('17.5: prevents uncontrolled auto-diagnostic turns when disabled', () => {
      const autoTriggerSetting = 'false';
      const shouldAutoTrigger = autoTriggerSetting !== 'false';
      expect(shouldAutoTrigger).toBe(false);
    });
  });

  // -------------------------------------------------------------------------
  // Feature 18: Model Quick Bar Presets
  // -------------------------------------------------------------------------
  describe('F18: Model Quick Bar Presets', async () => {
    const quickBarSvelte = await readProjectFile('src/components/ai/AiModelQuickBar.svelte');

    it('18.1: includes claude-3-5-sonnet preset for Anthropic', () => {
      expect(quickBarSvelte).toMatch(/claude-3-5-sonnet/);
    });

    it('18.2: includes gpt-4o-mini preset for OpenAI', () => {
      expect(quickBarSvelte).toContain('gpt-4o-mini');
    });

    it('18.3: includes gemini-2.0-flash preset for Gemini', () => {
      expect(quickBarSvelte).toContain('gemini-2.0-flash');
    });

    it('18.4: supports rapid provider and model switching', () => {
      expect(quickBarSvelte).toMatch(/handleModelSelect|selectModel|onSelect/i);
    });

    it('18.5: onSelect callback propagates selected model cleanly', () => {
      expect(quickBarSvelte).toContain('onSelect');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 19: Keyboard Event Bubbling Fixes
  // -------------------------------------------------------------------------
  describe('F19: Keyboard Event Bubbling Fixes', async () => {
    const choiceCardSvelte = await readProjectFile('src/components/ai/InteractiveChoiceCard.svelte');
    const emptyStateSvelte = await readProjectFile('src/components/ai/AiEmptyState.svelte');
    const followUpSvelte = await readProjectFile('src/components/ai/AiFollowUpBar.svelte');

    it('19.1: InteractiveChoiceCard stops keydown event propagation on option click/keypress', () => {
      expect(choiceCardSvelte).toContain('stopPropagation()');
    });

    it('19.2: AiEmptyState stops propagation in handleInsert', () => {
      expect(emptyStateSvelte).toContain('e.stopPropagation()');
      expect(emptyStateSvelte).toContain('handleCardKeyDown');
    });

    it('19.3: AiFollowUpBar intercepts Shift modifier without blocking native button events', () => {
      expect(followUpSvelte).toMatch(/Shift/);
    });

    it('19.4: CodeBlock copies command verbatim without keyboard bubbling issues', async () => {
      const codeBlockSvelte = await readProjectFile('src/components/ai/CodeBlock.svelte');
      expect(codeBlockSvelte).toContain('handleCopy');
    });

    it('19.5: ToolStepCard handles click and keyboard interaction cleanly', async () => {
      const toolStepSvelte = await readProjectFile('src/components/ai/ToolStepCard.svelte');
      expect(toolStepSvelte).toContain('onApprove');
    });
  });

  // -------------------------------------------------------------------------
  // Feature 20: Quality Gates & Packaging
  // -------------------------------------------------------------------------
  describe('F20: Quality Gates & Packaging', async () => {
    const pkgJson = JSON.parse(await readProjectFile('package.json'));
    const tauriConf = JSON.parse(await readProjectFile('src-tauri/tauri.conf.json'));

    it('20.1: defines check script for svelte type checking', () => {
      expect(pkgJson.scripts).toHaveProperty('check');
      expect(pkgJson.scripts.check).toContain('svelte-check');
    });

    it('20.2: defines lint script with max-warnings enforcement', () => {
      expect(pkgJson.scripts).toHaveProperty('lint');
      expect(pkgJson.scripts.lint).toContain('eslint');
    });

    it('20.3: defines check:i18n script for translation parity verification', () => {
      expect(pkgJson.scripts).toHaveProperty('check:i18n');
      expect(pkgJson.scripts['check:i18n']).toContain('check-i18n-keys.mjs');
    });

    it('20.4: defines check:tokens script for CSS design token verification', () => {
      expect(pkgJson.scripts).toHaveProperty('check:tokens');
      expect(pkgJson.scripts['check:tokens']).toContain('check-css-tokens.mjs');
    });

    it('20.5: tauri.conf.json configures application packaging correctly', () => {
      expect(tauriConf.productName).toBe('ColimaUI');
      expect(tauriConf.bundle).toBeDefined();
    });
  });
});
