import { describe, it, expect } from 'vitest';
import { escapeHtml, renderMarkdown } from './markdown';

describe('markdown utilities', () => {
  describe('escapeHtml', () => {
    it('should escape <, >, &, ", and \'', () => {
      const input = '<script>alert("hello & \'world\'")</script>';
      const expected = '&lt;script&gt;alert(&quot;hello &amp; &#039;world&#039;&quot;)&lt;/script&gt;';
      expect(escapeHtml(input)).toBe(expected);
    });

    it('should handle empty or null strings safely', () => {
      expect(escapeHtml('')).toBe('');
      // @ts-ignore testing invalid input
      expect(escapeHtml(null)).toBe('');
    });
  });

  describe('renderMarkdown', () => {
    it('should convert bold **text** to <strong>', () => {
      const input = 'This is **bold** and this is **also bold**';
      const result = renderMarkdown(input);
      expect(result).toContain('<strong>bold</strong>');
      expect(result).toContain('<strong>also bold</strong>');
    });

    it('should convert inline `code` to stylized <code> tags', () => {
      const input = 'Use `console.log()`';
      const result = renderMarkdown(input);
      expect(result).toContain('<code style="background:rgba(255,255,255,0.06);padding:1px 4px;border-radius:3px;font-size:0.72rem">console.log()</code>');
    });

    it('should render <think> blocks as stylized div elements', () => {
      const input = '<think>\nI am thinking about this.\n</think>\nAnd here is the answer.';
      const result = renderMarkdown(input);
      expect(result).toContain('class="ai-think-block"');
      expect(result).toContain('Thinking Process');
      expect(result).toContain('I am thinking about this.');
      expect(result).toContain('<p>And here is the answer.</p>');
    });

    it('should handle multi-line code blocks', () => {
      const input = '```javascript\nconst a = 1;\nconst b = 2;\n```';
      const result = renderMarkdown(input);
      expect(result).toContain('<pre><code>const a = 1;\nconst b = 2;\n</code></pre>');
    });

    it('should leave unclosed code blocks open and render them at the end', () => {
      const input = '```\nconst a = 1;\n';
      const result = renderMarkdown(input);
      expect(result).toContain('<pre><code>const a = 1;\n\n</code></pre>');
    });

    it('should correctly escape HTML inside code blocks and think blocks', () => {
      const input = '```\n<script>alert("test")</script>\n```\n<think>\n<script>\n</think>';
      const result = renderMarkdown(input);
      expect(result).toContain('&lt;script&gt;alert(&quot;test&quot;)&lt;/script&gt;');
      expect(result).toContain('&lt;script&gt;');
    });

    it('should render # and ## headings as heading elements', () => {
      const input = '# Article title\n## Related';
      const result = renderMarkdown(input);
      expect(result).toContain('<h2 class="md-h md-h-title">Article title</h2>');
      expect(result).toContain('<h3 class="md-h">Related</h3>');
    });

    it('should handle ### headers and lists', () => {
      const input = '### Header 3\n- Item 1\n- Item 2';
      const result = renderMarkdown(input);
      expect(result).toContain('<h4 class="md-h">Header 3</h4>');
      expect(result).toContain('<p>• Item 1</p>');
      expect(result).toContain('<p>• Item 2</p>');
    });

    it('should render [text](slug) links with data-slug', () => {
      const input = '- [Start a Colima instance](start-colima)';
      const result = renderMarkdown(input);
      expect(result).toContain('<a class="md-link" data-slug="start-colima">Start a Colima instance</a>');
    });
    
    it('should handle empty lines gracefully by adding <br />', () => {
      const input = 'Line 1\n\nLine 2';
      const result = renderMarkdown(input);
      expect(result).toContain('<p>Line 1</p>');
      expect(result).toContain('<br />');
      expect(result).toContain('<p>Line 2</p>');
    });
  });
});
