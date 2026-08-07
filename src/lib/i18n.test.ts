import { describe, it, expect, beforeEach } from 'vitest';
import { t, setLanguage, getLanguage } from './i18n.svelte';

describe('i18n — getLanguage / setLanguage', () => {
  beforeEach(() => {
    setLanguage('en'); // reset to default before each test
  });

  it('defaults to English', () => {
    expect(getLanguage()).toBe('en');
  });

  it('sets language to vi', () => {
    setLanguage('vi');
    expect(getLanguage()).toBe('vi');
  });

  it('falls back to en for unknown language', () => {
    setLanguage('xx'); // non-existent locale
    expect(getLanguage()).toBe('en');
  });
});

describe('i18n — t() translation function', () => {
  beforeEach(() => {
    setLanguage('en');
  });

  it('returns a known key from the en dictionary', () => {
    expect(t('sidebar.dashboard')).toBe('Dashboard');
  });

  it('returns a nested key', () => {
    expect(t('dashboard.running')).toBe('Running');
  });

  it('returns the default fallback for an unknown key', () => {
    const result = t('some.unknown.key', { default: 'My Fallback' });
    expect(result).toBe('My Fallback');
  });

  it('returns the raw key for unknown key with no default', () => {
    const result = t('some.unknown.key');
    expect(result).toBe('some.unknown.key');
  });

  it('performs variable interpolation', () => {
    // This key does not exist, so we get the default with interpolation
    const result = t('fake.greeting', { default: 'Hello {name}!', name: 'World' });
    expect(result).toBe('Hello World!');
  });

  it('handles multiple interpolation variables', () => {
    const result = t('fake.msg', { default: '{a} + {b} = {c}', a: '1', b: '2', c: '3' });
    expect(result).toBe('1 + 2 = 3');
  });

  it('default param is not injected as interpolation variable into translated strings', () => {
    // The 'default' key should not replace a {default} placeholder if a real translation is found
    const result = t('sidebar.dashboard', { default: 'Fallback' });
    expect(result).toBe('Dashboard'); // real translation wins
  });

  it('handles deeply nested missing keys gracefully', () => {
    const result = t('a.b.c.d.e.f', { default: 'deep fallback' });
    expect(result).toBe('deep fallback');
  });
});
