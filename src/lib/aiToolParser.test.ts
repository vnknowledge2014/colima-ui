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
