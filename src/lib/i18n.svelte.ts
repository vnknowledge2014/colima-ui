import en from '../locales/en.json';
import vi from '../locales/vi.json';
import zh from '../locales/zh.json';
import ja from '../locales/ja.json';

const dictionaries: Record<string, any> = {
  en,
  vi,
  zh,
  ja
};

// Global reactive state for current language
let currentLang = $state('en');

// Set language function
export function setLanguage(lang: string) {
  if (dictionaries[lang]) {
    currentLang = lang;
  } else {
    console.warn(`Language ${lang} not found, falling back to en`);
    currentLang = 'en';
  }
}

// Get current language
export function getLanguage() {
  return currentLang;
}

// Translation function using reactive $derived or just reading $state
export function t(key: string, params?: Record<string, string | number>): string {
  // To ensure reactivity in Svelte 5, we read the state variable here
  // so any component calling t() will react when currentLang changes.
  const dict = dictionaries[currentLang] || dictionaries['en'];
  
  const keys = key.split('.');
  let value: any = dict;
  
  for (const k of keys) {
    if (value === undefined || value === null) break;
    value = value[k];
  }
  
  if (value === undefined || typeof value !== 'string') {
    // No translation found — use default fallback or the raw key
    let fallback = params && params.default ? String(params.default) : key;
    // Still apply interpolation to the fallback string
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        if (k === 'default') continue; // skip the 'default' meta-param
        fallback = fallback.replace(new RegExp(`{${k}}`, 'g'), String(v));
      }
    }
    return fallback;
  }
  
  let translated = value;
  
  // Interpolation: replace {var} with params[var]
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      if (k === 'default') continue; // skip the 'default' meta-param
      translated = translated.replace(new RegExp(`{${k}}`, 'g'), String(v));
    }
  }
  
  return translated;
}
