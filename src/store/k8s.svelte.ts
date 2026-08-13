export interface K8sResource {
  name: string;
  namespace?: string;
  status?: string;
  age?: string;
  ready?: string;
  restarts?: string;
  node?: string;
  replicas?: string;
  available?: string;
  svcType?: string;
  clusterIP?: string;
  ports?: string;
  _ports?: Record<string, unknown>[];
  roles?: string;
  version?: string;
  os?: string;
  schedulable?: boolean;
  completions?: string;
  schedule?: string;
  lastSchedule?: string;
  hosts?: string;
  paths?: string;
  address?: string;
  dataCount?: string;
  secretType?: string;
  capacity?: string;
  accessModes?: string;
  reclaimPolicy?: string;
  storageClass?: string;
  volume?: string;
  type?: string;
  reason?: string;
  message?: string;
  count?: string;
  source?: string;
  object?: string;
  warnings?: string[];
  _raw: Record<string, unknown>;
}

export const k8sState = $state({
  connected: false,
  loading: true,
  dataLoading: false,
  namespaces: [] as { name: string }[],
  items: [] as K8sResource[],
  activeResource: "pods",
  namespace: "default",
  contexts: [] as string[],
  currentCtx: "",
});
