import { atom } from 'jotai';
import { DockerContainer, DockerImage } from '../lib/api';

export const containersAtom = atom<DockerContainer[]>([]);
export const imagesAtom = atom<DockerImage[]>([]);
export const dockerLoadingAtom = atom<boolean>(true);

/**
 * Global cooldown for event-based state updates.
 * After a manual container action (start/stop/remove), we do a fresh API fetch
 * which returns authoritative data. During a short cooldown window, we skip
 * event-based pushes from the watcher to prevent stale data from overwriting
 * the fresh fetch result.
 *
 * This solves the race: stop → refreshContainers (correct) → watcher event
 * with stale intermediate state (incorrect) overwrites the correct data.
 */
let _eventCooldownUntil = 0;
export function setEventCooldown(durationMs = 1500) {
  _eventCooldownUntil = Date.now() + durationMs;
}
export function isEventCooldownActive(): boolean {
  return Date.now() < _eventCooldownUntil;
}
