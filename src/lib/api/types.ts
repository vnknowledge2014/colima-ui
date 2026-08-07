// ===== Types =====

export interface ColimaInstance {
  name: string;
  status: string;
  arch: string;
  cpus: number;
  memory: number;
  disk: number;
  runtime: string;
  address: string;
  kubernetes: boolean;
}

export interface InstanceStatus {
  profile: string;
  status: string;
  arch: string;
  runtime: string;
  port_forwarding: string;
  cpu_usage: string;
  memory_usage: string;
  disk_usage: string;
  address: string;
}

export interface StartConfig {
  profile: string;
  runtime: string;
  cpus: number;
  memory: number;
  disk: number;
  vm_type: string;
  kubernetes: boolean;
  kubernetes_version: string;
  arch: string;
  mount_type: string;
  mounts: string[];
  dns: string[];
  network_address: boolean;
}

export interface DockerContainer {
  Id: string;
  Names: string;
  Image: string;
  Status: string;
  State: string;
  Ports: string;
  CreatedAt: string;
  Size: string;
  Command: string;
}

export interface DockerImage {
  Id: string;
  Repository: string;
  Tag: string;
  Size: string;
  CreatedAt: string;
}

export interface SystemInfo {
  colima_installed: boolean;
  colima_version: string;
  docker_installed: boolean;
  docker_version: string;
  lima_installed: boolean;
  lima_version: string;
}

export interface AiModel {
  name: string;
  size: string;
  format: string;
  family: string;
  parameters: string;
  quantization: string;
}

export interface DockerVolume {
  Name: string;
  Driver: string;
  Mountpoint: string;
  Scope: string;
  Labels: string;
}

export interface DockerNetwork {
  Id: string;
  Name: string;
  Driver: string;
  Scope: string;
  Ipv6: string;
  Internal: string;
  Labels: string;
}
