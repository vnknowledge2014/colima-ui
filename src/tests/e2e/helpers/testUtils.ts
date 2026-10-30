/**
 * UI Simulation & Test Utilities for E2E Test Suite
 */

export interface SimulatedScrollContainer {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
}

export function isNearBottom(
  container: SimulatedScrollContainer,
  threshold = 72,
): boolean {
  const distance =
    container.scrollHeight - container.scrollTop - container.clientHeight;
  return distance < threshold;
}

export interface SimulatedMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export function simulateTruncateAiMessagesFrom(
  messages: SimulatedMessage[],
  targetId: string,
): { remaining: SimulatedMessage[]; truncated: SimulatedMessage[] } {
  const index = messages.findIndex((m) => m.id === targetId);
  if (index === -1) {
    return { remaining: [...messages], truncated: [] };
  }
  return {
    remaining: messages.slice(0, index),
    truncated: messages.slice(index),
  };
}

export function setupClipboardMock(): {
  writtenText: string[];
  restore: () => void;
} {
  const writtenText: string[] = [];
  const originalClipboard = navigator.clipboard;

  const mockClipboard = {
    writeText: async (text: string) => {
      writtenText.push(text);
      return Promise.resolve();
    },
    readText: async () => {
      return Promise.resolve(writtenText[writtenText.length - 1] || '');
    },
  };

  Object.defineProperty(navigator, 'clipboard', {
    value: mockClipboard,
    writable: true,
    configurable: true,
  });

  return {
    writtenText,
    restore: () => {
      Object.defineProperty(navigator, 'clipboard', {
        value: originalClipboard,
        writable: true,
        configurable: true,
      });
    },
  };
}

export function deriveActiveVmTarget(
  instances: Array<{ name: string; status: string }>,
): { kind: 'colima' | 'lima'; profile: string } {
  const runningColima = instances.find(
    (i) => i.status.toLowerCase() === 'running',
  );
  if (runningColima) {
    const profile =
      runningColima.name === 'colima' || runningColima.name === 'default'
        ? 'default'
        : runningColima.name;
    return { kind: 'colima', profile };
  }
  return { kind: 'colima', profile: 'default' };
}

export async function readProjectFile(relPath: string): Promise<string> {
  try {
    const fsMod = 'node:fs';
    const pathMod = 'node:path';
    const fs = await import(/* @vite-ignore */ fsMod);
    const path = await import(/* @vite-ignore */ pathMod);
    const proc = (globalThis as unknown as { process: { cwd: () => string } }).process;
    return fs.readFileSync(path.resolve(proc.cwd(), relPath), 'utf-8');
  } catch {
    return '';
  }
}
