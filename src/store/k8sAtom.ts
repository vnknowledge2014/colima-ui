import { atom } from 'jotai';

export interface K8sResource {
  name: string;
  namespace: string;
  [key: string]: any;
}

export const k8sConnectedAtom = atom<boolean | null>(null);
export const k8sLoadingAtom = atom<boolean>(true);
export const k8sDataLoadingAtom = atom<boolean>(false);
export const k8sNamespacesAtom = atom<{ name: string }[]>([]);
export const k8sItemsAtom = atom<K8sResource[]>([]);
export const k8sActiveResourceAtom = atom<string>('pods');
export const k8sNamespaceAtom = atom<string>('all');
export const k8sContextsAtom = atom<string[]>([]);
export const k8sCurrentCtxAtom = atom<string>('');
