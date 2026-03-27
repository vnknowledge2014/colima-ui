import { atom } from 'jotai';
import { DockerContainer, DockerImage } from '../lib/api';

export const containersAtom = atom<DockerContainer[]>([]);
export const imagesAtom = atom<DockerImage[]>([]);
export const dockerLoadingAtom = atom<boolean>(true);
