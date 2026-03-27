import { atom } from 'jotai';
import { DockerVolume, DockerNetwork } from '../lib/api';

export const volumesAtom = atom<DockerVolume[]>([]);
export const volumesLoadingAtom = atom<boolean>(true);
export const networksAtom = atom<DockerNetwork[]>([]);
export const networksLoadingAtom = atom<boolean>(true);
