import { describe, it, expect } from 'vitest';
import { parseAiTools } from './aiToolParser';

describe('aiToolParser', () => {
  it('should parse [QUERY: ... ] tags correctly', () => {
    const input = 'Here is the data: [QUERY: getContainers | show all] and another [QUERY: getImages]';
    const parsed = parseAiTools(input);
    
    expect(parsed.hasTools).toBe(true);
    expect(parsed.queries.length).toBe(2);
    expect(parsed.queries[0][1]).toBe('getContainers ');
    expect(parsed.queries[0][2]).toBe('show all');
    expect(parsed.queries[1][1]).toBe('getImages');
    expect(parsed.queries[1][2]).toBeUndefined();
    
    expect(parsed.cleanText).toBe('Here is the data:  and another');
  });

  it('should parse [EVENT_APPROVE: ... ] tags correctly', () => {
    const input = '[EVENT_APPROVE: compose-build | my-project] Please build it.';
    const parsed = parseAiTools(input);
    
    expect(parsed.hasTools).toBe(true);
    expect(parsed.eventApprovals.length).toBe(1);
    expect(parsed.eventApprovals[0][1]).toBe('compose-build ');
    expect(parsed.eventApprovals[0][2]).toBe('my-project');
    expect(parsed.cleanText).toBe('Please build it.');
  });

  it('should parse [NAVIGATE: ... ] tags correctly', () => {
    const input = 'Sure, let me take you there: [NAVIGATE: compose]';
    const parsed = parseAiTools(input);
    
    expect(parsed.hasTools).toBe(true);
    expect(parsed.navigates.length).toBe(1);
    expect(parsed.navigates[0][1]).toBe('compose');
    expect(parsed.cleanText).toBe('Sure, let me take you there:');
  });

  it('should correctly handle absence of tools', () => {
    const input = 'Just a normal chat response without any tools.';
    const parsed = parseAiTools(input);
    
    expect(parsed.hasTools).toBe(false);
    expect(parsed.queries.length).toBe(0);
    expect(parsed.cleanText).toBe(input);
  });
  
  it('should parse simple parameterless commands like [DIAGNOSE]', () => {
    const input = 'Let me check: [DIAGNOSE]';
    const parsed = parseAiTools(input);

    expect(parsed.hasTools).toBe(true);
    expect(parsed.hasDiagnose).toBe(true);
    expect(parsed.cleanText).toBe('Let me check:');
  });
});

describe('[SUGGEST:] follow-up chips', () => {
  it('parses a label with and without its prompt', () => {
    const input = 'That failed. [SUGGEST: Show the logs | Show me the last 100 lines][SUGGEST: Restart it]';
    const parsed = parseAiTools(input);

    expect(parsed.suggests.length).toBe(2);
    expect(parsed.suggests[0][1]).toBe('Show the logs');
    expect(parsed.suggests[0][2]).toBe('Show me the last 100 lines');
    expect(parsed.suggests[1][1]).toBe('Restart it');
    // No prompt given — the panel falls back to the label.
    expect(parsed.suggests[1][2]).toBeUndefined();
    expect(parsed.cleanText).toBe('That failed.');
  });

  it('does not count as a tool, so a suggest-only reply is final', () => {
    // agentCore treats "no tools" as the end of the agent loop. A suggestion is
    // a finished answer, so counting it would spend another round.
    const parsed = parseAiTools('I could not identify it. [SUGGEST: Try again]');

    expect(parsed.hasTools).toBe(false);
    expect(parsed.suggests.length).toBe(1);
  });

  it('still reports tools when a reply mixes suggestions with a real one', () => {
    const parsed = parseAiTools('[QUERY: list-containers][SUGGEST: Restart it]');

    expect(parsed.hasTools).toBe(true);
    expect(parsed.suggests.length).toBe(1);
  });

  it('caps a chatty model at three chips', () => {
    const parsed = parseAiTools(
      '[SUGGEST: One][SUGGEST: Two][SUGGEST: Three][SUGGEST: Four][SUGGEST: Five]',
    );

    expect(parsed.suggests.length).toBe(3);
    // Dropped chips are still stripped from the visible text.
    expect(parsed.cleanText).toBe('');
  });

  it('leaves the other tags untouched', () => {
    const parsed = parseAiTools('[NAVIGATE: compose][SUGGEST: Go back | Take me to the dashboard]');

    expect(parsed.navigates.length).toBe(1);
    expect(parsed.navigates[0][1]).toBe('compose');
    expect(parsed.suggests[0][2]).toBe('Take me to the dashboard');
  });
});
