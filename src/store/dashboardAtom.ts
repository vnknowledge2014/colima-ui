import { atom } from 'jotai';
import { LimaInstance } from '../lib/api';

export interface DashboardCounts {
  containers: number;
  running: number;
  images: number;
  volumes: number;
  networks: number;
  composeProjects: number;
}

export interface K8sStatusSummary {
  connected: boolean;
  pods: number;
  namespaces: number;
  kindClusters: number;
}

export const dashboardCountsAtom = atom<DashboardCounts | null>(null);
export const dashboardK8sAtom = atom<K8sStatusSummary | null>(null);
export const dashboardVMsAtom = atom<LimaInstance[]>([]);
export const dashboardLastFetchAtom = atom<number>(0);
