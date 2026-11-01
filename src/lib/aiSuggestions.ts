import type { ColimaInstance, DockerContainer } from "./api/types";
import { t } from "./i18n.svelte";

interface ContainerLike {
  Id?: string;
  id?: string;
  Names?: string | string[];
  name?: string;
  State?: string;
  state?: string;
  Status?: string;
  status?: string;
  Image?: string;
  image?: string;
}

export interface StarterPrompt {
  id: string;
  title: string;
  prompt: string;
  icon: "warning" | "container" | "vm" | "compose" | "k8s" | "security" | "general";
  category: "warning" | "context" | "help";
}

export interface StarterPromptContext {
  currentPage?: string;
  instances?: ColimaInstance[];
  colimaInstances?: ColimaInstance[];
  instancesLoaded?: boolean;
  containers?: DockerContainer[];
  dockerContainers?: DockerContainer[];
  k8sStatus?: { connected: boolean; pods?: number; namespaces?: number; kindClusters?: number };
}

export interface FallbackSuggestionContext {
  toolsExecuted?: (string | { kind?: string; name?: string; [key: string]: unknown })[];
  currentPage?: string;
  hasChoiceCard?: boolean;
}

export interface LiveUiContextSnapshot {
  currentPage: string;
  language: string;
  activeInstances: string[];
  runningContainers: number;
  stoppedContainers: number;
  k8sConnected: boolean;
  activeContext: string;
}

export interface LiveUiSnapshotOptions {
  currentPage?: string;
  language?: string;
  instances?: ColimaInstance[];
  colimaInstances?: ColimaInstance[];
  containers?: DockerContainer[];
  dockerContainers?: DockerContainer[];
  k8sStatus?: { connected: boolean; pods?: number; namespaces?: number; kindClusters?: number };
  activeContext?: string;
}

/**
 * Returns prioritized starter prompts for the AI Empty State.
 * Surfaces up to 2 high-priority system alerts (stopped VMs, exited containers),
 * followed by contextual prompts for the currently active tab (max 4 total).
 */
export function getStarterPrompts(context: StarterPromptContext = {}): StarterPrompt[] {
  const prompts: StarterPrompt[] = [];
  const warnings: StarterPrompt[] = [];
  const ctx = context || {};

  // 1. Inspect Colima VM instances for warnings
  const rawInstances = ctx.colimaInstances || ctx.instances;
  const instances = Array.isArray(rawInstances) ? rawInstances : [];
  if (ctx.instancesLoaded !== false && instances.length > 0) {
    for (const inst of instances) {
      if (warnings.length >= 2) break;
      if (!inst || typeof inst !== "object") continue;
      if (typeof inst.status !== "string" || !inst.status) continue;
      const status = typeof inst.status === "string" ? inst.status.toLowerCase() : "";
      if (status !== "running") {
        const instName = typeof inst.name === "string" && inst.name ? inst.name : "default";
        warnings.push({
          id: `warning-instance-${instName}`,
          title: t("ai.starter.warning_instance_title", { default: "Investigate stopped instance {name}", name: instName }),
          prompt: t("ai.starter.warning_instance_prompt", { default: "Why is Colima instance {name} stopped? Check its status and suggest how to start it. Investigate stopped/unhealthy instance {name}.", name: instName }),
          icon: "warning",
          category: "warning",
        });
      }
    }
  }

  // 2. Inspect containers for warnings (exited or dead)
  const rawContainers = ctx.dockerContainers || ctx.containers;
  const containers = Array.isArray(rawContainers) ? rawContainers : [];
  if (warnings.length < 2 && containers.length > 0) {
    for (const c of containers) {
      if (warnings.length >= 2) break;
      if (!c || typeof c !== "object") continue;
      const cont = c as ContainerLike;
      const hasContainerProps = Boolean(
        (cont.Id || cont.id || cont.Names || cont.name) &&
        (cont.State || cont.state || cont.Status || cont.status)
      );
      if (!hasContainerProps) continue;
      const state = typeof cont.State === "string"
        ? cont.State.toLowerCase()
        : typeof cont.state === "string"
          ? cont.state.toLowerCase()
          : "";
      const status = typeof cont.Status === "string"
        ? cont.Status.toLowerCase()
        : typeof cont.status === "string"
          ? cont.status.toLowerCase()
          : "";
      if (state === "exited" || state === "dead" || status.startsWith("exited")) {
        const rawName = Array.isArray(cont.Names)
          ? cont.Names[0] ?? ""
          : typeof cont.Names === "string"
            ? cont.Names
            : typeof cont.name === "string"
              ? cont.name
              : "";
        const rawId = typeof cont.Id === "string"
          ? cont.Id
          : typeof cont.id === "string"
            ? cont.id
            : "";
        const cleanName = (typeof rawName === "string" ? rawName.replace(/^\//, "") : "") || (rawId ? rawId.slice(0, 12) : "unknown");
        const id = cont.Names !== undefined
          ? `warning-container-${rawId || cleanName}`
          : `warn-container-${rawId || cleanName}`;
        const title = cont.Names !== undefined
          ? t("ai.starter.warning_container_title", { default: 'Container "{name}" exited', name: cleanName })
          : t("ai.starter.warning_container_unnamed_title", { default: "Analyze exited container {name}", name: cleanName });
        const prompt = cont.Names !== undefined
          ? t("ai.starter.warning_container_prompt", { default: "Analyze why container {name} exited", name: cleanName })
          : t("ai.starter.warning_container_unnamed_prompt", { default: "Container {name} ({id}) exited. Analyze why container {name} exited.", name: cleanName, id: rawId || cleanName });
        warnings.push({
          id,
          title,
          prompt,
          icon: "warning",
          category: "warning",
        });
      }
    }
  }

  // Include up to 2 top warnings
  const selectedWarnings = warnings.slice(0, 2);
  prompts.push(...selectedWarnings);

  // 3. Tab-specific contextual starter prompts
  const page = typeof ctx.currentPage === "string" && ctx.currentPage ? ctx.currentPage.toLowerCase() : "dashboard";
  const pagePrompts = getPageContextPrompts(page);

  // If warnings exist, add up to 2 page prompts (max 4 total). Otherwise add up to 4 page prompts.
  const maxPagePrompts = selectedWarnings.length > 0 ? 2 : 4;
  prompts.push(...pagePrompts.slice(0, maxPagePrompts));

  if (prompts.length < 4 && page === "settings") {
    prompts.push({
      id: "starter-general-optimize",
      title: t("ai.starter.general_optimize_title", { default: "Optimize VM resources" }),
      prompt: t("ai.starter.general_optimize_prompt", { default: "What are the recommended CPU, memory, and disk settings for my Colima instances?" }),
      icon: "general",
      category: "help",
    });
  }

  return prompts.slice(0, 4);
}

function getPageContextPrompts(page: string): StarterPrompt[] {
  switch (page) {
    case "instances":
      return [
        {
          id: "starter-instances-config",
          title: t("ai.starter.instances_config_title", { default: "Create or optimize VM" }),
          prompt: t("ai.starter.instances_config_prompt", { default: "Create or optimize a Colima VM configuration" }),
          icon: "vm",
          category: "context",
        },
        {
          id: "starter-instances-mounts",
          title: t("ai.starter.instances_mounts_title", { default: "Explain mount types" }),
          prompt: t("ai.starter.instances_mounts_prompt", { default: "Explain difference between virtiofs, sshfs, and 9p mounts" }),
          icon: "vm",
          category: "context",
        },
        {
          id: "starter-instances-resize",
          title: t("ai.starter.instances_resize_title", { default: "Resize CPU and memory" }),
          prompt: t("ai.starter.instances_resize_prompt", { default: "How to resize CPU and memory for an existing VM" }),
          icon: "vm",
          category: "context",
        },
      ];

    case "containers":
      return [
        {
          id: "starter-containers-resources",
          title: t("ai.starter.containers_resources_title", { default: "Inspect resource consumption" }),
          prompt: t("ai.starter.containers_resources_prompt", { default: "List running containers and inspect their CPU and memory usage" }),
          icon: "container",
          category: "context",
        },
        {
          id: "starter-containers-list",
          title: t("ai.starter.containers_list_title", { default: "List active containers" }),
          prompt: t("ai.starter.containers_list_prompt", { default: "List running containers and inspect resource usage" }),
          icon: "container",
          category: "context",
        },
        {
          id: "starter-containers-mem",
          title: t("ai.starter.containers_mem_title", { default: "Top memory consumers" }),
          prompt: t("ai.starter.containers_mem_prompt", { default: "Which containers are consuming the most memory?" }),
          icon: "container",
          category: "context",
        },
        {
          id: "starter-containers-prune",
          title: t("ai.starter.containers_prune_title", { default: "Clean stopped containers" }),
          prompt: t("ai.starter.containers_prune_prompt", { default: "Clean up all stopped containers and reclaim space" }),
          icon: "container",
          category: "context",
        },
      ];

    case "compose":
      return [
        {
          id: "starter-compose-diagnose",
          title: t("ai.starter.compose_diagnose_title", { default: "Diagnose Compose services" }),
          prompt: t("ai.starter.compose_diagnose_prompt", { default: "Diagnose compose services and check health" }),
          icon: "compose",
          category: "context",
        },
        {
          id: "starter-compose-validate",
          title: t("ai.starter.compose_validate_title", { default: "Validate docker-compose" }),
          prompt: t("ai.starter.compose_validate_prompt", { default: "Validate docker-compose file for syntax or environment errors" }),
          icon: "compose",
          category: "context",
        },
        {
          id: "starter-compose-logs",
          title: t("ai.starter.compose_logs_title", { default: "Inspect compose logs" }),
          prompt: t("ai.starter.compose_logs_prompt", { default: "Check recent error logs across all compose stack services" }),
          icon: "compose",
          category: "context",
        },
      ];

    case "kubernetes":
      return [
        {
          id: "starter-k8s-status",
          title: t("ai.starter.k8s_status_title", { default: "Cluster & pod health" }),
          prompt: t("ai.starter.k8s_status_prompt", { default: "Check K8s cluster status and pod health" }),
          icon: "k8s",
          category: "context",
        },
        {
          id: "starter-k8s-failing",
          title: t("ai.starter.k8s_failing_title", { default: "Find failing pods" }),
          prompt: t("ai.starter.k8s_failing_prompt", { default: "Find pods in CrashLoopBackOff or Pending status" }),
          icon: "k8s",
          category: "context",
        },
        {
          id: "starter-k8s-namespaces",
          title: t("ai.starter.k8s_namespaces_title", { default: "List namespaces" }),
          prompt: t("ai.starter.k8s_namespaces_prompt", { default: "List Kubernetes namespaces and workload distributions" }),
          icon: "k8s",
          category: "context",
        },
      ];

    case "images":
      return [
        {
          id: "starter-images-unused",
          title: t("ai.starter.images_unused_title", { default: "Scan unused images" }),
          prompt: t("ai.starter.images_unused_prompt", { default: "Scan unused container images and clean up disk space" }),
          icon: "container",
          category: "context",
        },
        {
          id: "starter-images-optimize",
          title: t("ai.starter.images_optimize_title", { default: "Optimize image sizes" }),
          prompt: t("ai.starter.images_optimize_prompt", { default: "Analyze large Docker images and recommend size optimizations" }),
          icon: "container",
          category: "context",
        },
      ];

    case "volumes":
      return [
        {
          id: "starter-volumes-inspect",
          title: t("ai.starter.volumes_inspect_title", { default: "Inspect volume sizes" }),
          prompt: t("ai.starter.volumes_inspect_prompt", { default: "Inspect volume sizes and orphaned data" }),
          icon: "container",
          category: "context",
        },
        {
          id: "starter-volumes-prune",
          title: t("ai.starter.volumes_prune_title", { default: "Remove dangling volumes" }),
          prompt: t("ai.starter.volumes_prune_prompt", { default: "Identify and remove dangling volumes safely" }),
          icon: "container",
          category: "context",
        },
      ];

    case "networks":
      return [
        {
          id: "starter-networks-inspect",
          title: t("ai.starter.networks_inspect_title", { default: "Inspect networks & ports" }),
          prompt: t("ai.starter.networks_inspect_prompt", { default: "Inspect container bridge networks and port mappings" }),
          icon: "container",
          category: "context",
        },
        {
          id: "starter-networks-conflicts",
          title: t("ai.starter.networks_conflicts_title", { default: "Fix port conflicts" }),
          prompt: t("ai.starter.networks_conflicts_prompt", { default: "Troubleshoot container port conflicts and host reachability" }),
          icon: "container",
          category: "context",
        },
      ];

    case "security":
      return [
        {
          id: "starter-security-scan",
          title: t("ai.starter.security_scan_title", { default: "Vulnerability scan" }),
          prompt: t("ai.starter.security_scan_prompt", { default: "Run vulnerability scan and evaluate security posture" }),
          icon: "security",
          category: "context",
        },
        {
          id: "starter-security-audit",
          title: t("ai.starter.security_audit_title", { default: "Audit privileged containers" }),
          prompt: t("ai.starter.security_audit_prompt", { default: "Audit privileged containers and host port exposures" }),
          icon: "security",
          category: "context",
        },
      ];

    case "models":
      return [
        {
          id: "starter-models-config",
          title: t("ai.starter.models_config_title", { default: "Check AI models" }),
          prompt: t("ai.starter.models_config_prompt", { default: "Check available AI models and configure endpoints" }),
          icon: "general",
          category: "context",
        },
        {
          id: "starter-models-ollama",
          title: t("ai.starter.models_ollama_title", { default: "Test Ollama connection" }),
          prompt: t("ai.starter.models_ollama_prompt", { default: "Test Ollama connection and list installed local models" }),
          icon: "general",
          category: "context",
        },
      ];

    case "settings":
      return [
        {
          id: "starter-general-optimize",
          title: t("ai.starter.general_optimize_title", { default: "Optimize VM resources" }),
          prompt: t("ai.starter.general_optimize_prompt", { default: "What are the recommended CPU, memory, and disk settings for my Colima instances?" }),
          icon: "general",
          category: "help",
        },
      ];

    case "dashboard":
    default:
      return [
        {
          id: "starter-dashboard-overview",
          title: t("ai.starter.dashboard_overview_title", { default: "System overview" }),
          prompt: t("ai.starter.dashboard_overview_prompt", { default: "Show system overview and active instances" }),
          icon: "general",
          category: "context",
        },
        {
          id: "starter-dashboard-resources",
          title: t("ai.starter.dashboard_resources_title", { default: "Check resource usage" }),
          prompt: t("ai.starter.dashboard_resources_prompt", { default: "Check resource usage across all running containers and VMs" }),
          icon: "general",
          category: "context",
        },
        {
          id: "starter-dashboard-health",
          title: t("ai.starter.dashboard_health_title", { default: "Runtime health check" }),
          prompt: t("ai.starter.dashboard_health_prompt", { default: "Inspect cluster and runtime health" }),
          icon: "general",
          category: "context",
        },
      ];
  }
}

/**
 * Derives 2–3 contextual follow-up chips when the LLM omits [SUGGEST] tags.
 * Returns an empty array if an interactive choice card is present.
 */
export function deriveFallbackSuggestions(context: FallbackSuggestionContext = {}): [string, string][] {
  const ctx = context || {};
  // If a choice card is active, suppress follow-up chips to prevent duplicate actions
  if (ctx.hasChoiceCard) {
    return [];
  }

  const rawTools = ctx.toolsExecuted;
  const toolsList = Array.isArray(rawTools) ? rawTools : [];
  const tools = toolsList.map((t) => {
    if (typeof t === "string") {
      return t.toLowerCase();
    }
    if (t && typeof t === "object") {
      const name = typeof t.name === "string" ? t.name : "";
      const kind = typeof t.kind === "string" ? t.kind : "";
      return `${name} ${kind}`.trim().toLowerCase();
    }
    return "";
  });

  // Check executed tools first
  if (tools.some((t) => t.includes("container") && (t.includes("list") || t.includes("stat")))) {
    return [
      [t("ai.fallback.check_cpu_mem", { default: "Check CPU & memory" }), t("ai.fallback.check_cpu_mem_prompt", { default: "Check CPU and memory usage of running containers" })],
      [t("ai.fallback.view_container_logs", { default: "View container logs" }), t("ai.fallback.view_container_logs_prompt", { default: "Show recent logs for running containers" })],
      [t("ai.fallback.check_cpu_ram", { default: "Check CPU & RAM" }), t("ai.fallback.check_cpu_ram_prompt", { default: "Inspect CPU and memory usage of running containers" })],
    ];
  }

  if (tools.some((t) => t.includes("log"))) {
    return [
      [t("ai.fallback.analyze_errors", { default: "Analyze errors" }), t("ai.fallback.analyze_errors_prompt", { default: "Analyze these container logs and propose troubleshooting fixes" })],
      [t("ai.fallback.check_restart_policy", { default: "Check restart policy" }), t("ai.fallback.check_restart_policy_prompt", { default: "Inspect restart policy and health check config" })],
      [t("ai.fallback.view_environment", { default: "View environment" }), t("ai.fallback.view_environment_prompt", { default: "Inspect container environment variables" })],
    ];
  }

  if (tools.some((t) => t.includes("diagnose") || t.includes("heal") || t.includes("compose"))) {
    return [
      [t("ai.fallback.propose_fixes", { default: "Propose fixes" }), t("ai.fallback.propose_fixes_prompt", { default: "Propose automated fixes for diagnosed issues" })],
      [t("ai.fallback.apply_recommended_fix", { default: "Apply recommended fix" }), t("ai.fallback.apply_recommended_fix_prompt", { default: "Propose and execute automated fix steps for the diagnosed issue" })],
      [t("ai.fallback.validate_compose_file", { default: "Validate compose file" }), t("ai.fallback.validate_compose_file_prompt", { default: "Check docker-compose.yml for configuration mistakes" })],
    ];
  }

  if (tools.some((t) => t.includes("instance") || t.includes("colima"))) {
    return [
      [t("ai.fallback.inspect_vm_resources", { default: "Inspect VM resources" }), t("ai.fallback.inspect_vm_resources_prompt", { default: "Show CPU, memory, and disk usage for Colima VM" })],
      [t("ai.fallback.check_mount_type", { default: "Check mount type" }), t("ai.fallback.check_mount_type_prompt", { default: "Verify active mount type and filesystem performance" })],
      [t("ai.fallback.view_lima_logs", { default: "View Lima VM logs" }), t("ai.fallback.view_lima_logs_prompt", { default: "Check Lima daemon and serial logs for warnings" })],
    ];
  }

  if (tools.some((t) => t.includes("k8s") || t.includes("pod"))) {
    return [
      [t("ai.fallback.check_pod_health", { default: "Check pod health" }), t("ai.fallback.check_pod_health_prompt", { default: "List pods that are restarting or in CrashLoopBackOff" })],
      [t("ai.fallback.inspect_cluster_events", { default: "Inspect cluster events" }), t("ai.fallback.inspect_cluster_events_prompt", { default: "Show recent Kubernetes cluster warning events" })],
      [t("ai.fallback.check_node_capacity", { default: "Check node capacity" }), t("ai.fallback.check_node_capacity_prompt", { default: "Inspect node CPU and memory allocatable capacity" })],
    ];
  }

  if (tools.some((t) => t.includes("sec") || t.includes("triage") || t.includes("vuln"))) {
    return [
      [t("ai.fallback.review_cves", { default: "Review CVEs" }), t("ai.fallback.review_cves_prompt", { default: "Filter and explain critical and high-severity CVEs" })],
      [t("ai.fallback.generate_patch", { default: "Generate patch" }), t("ai.fallback.generate_patch_prompt", { default: "Propose minimal Dockerfile or package updates to fix CVEs" })],
      [t("ai.fallback.audit_capabilities", { default: "Audit capabilities" }), t("ai.fallback.audit_capabilities_prompt", { default: "Check for containers running with root or elevated capabilities" })],
    ];
  }

  if (tools.some((t) => t.includes("event") || t.includes("command"))) {
    return [
      [t("ai.fallback.verify_status", { default: "Verify status" }), t("ai.fallback.verify_status_prompt", { default: "Verify that the operation completed successfully" })],
      [t("ai.fallback.next_steps", { default: "Next steps" }), t("ai.fallback.next_steps_prompt", { default: "What should I do next after this operation?" })],
      [t("ai.fallback.inspect_logs", { default: "Inspect logs" }), t("ai.fallback.inspect_logs_prompt", { default: "Check logs to confirm operation result" })],
    ];
  }

  // Fallback based on current page
  const page = typeof ctx.currentPage === "string" && ctx.currentPage ? ctx.currentPage.toLowerCase() : "dashboard";
  switch (page) {
    case "containers":
      return [
        [t("ai.fallback.list_running_containers", { default: "List running containers" }), t("ai.fallback.list_running_containers_prompt", { default: "List all active containers and their port mappings" })],
        [t("ai.fallback.check_resource_usage", { default: "Check resource usage" }), t("ai.fallback.check_resource_usage_prompt", { default: "Check container resource utilization" })],
        [t("ai.fallback.prune_stopped", { default: "Prune stopped" }), t("ai.fallback.prune_stopped_prompt", { default: "How do I remove all stopped containers?" })],
      ];

    case "compose":
      return [
        [t("ai.fallback.diagnose_active_stack", { default: "Diagnose active stack" }), t("ai.fallback.diagnose_active_stack_prompt", { default: "Diagnose compose services and check health" })],
        [t("ai.fallback.view_compose_logs", { default: "View compose logs" }), t("ai.fallback.view_compose_logs_prompt", { default: "Show recent logs for compose services" })],
        [t("ai.fallback.validate_configuration", { default: "Validate configuration" }), t("ai.fallback.validate_configuration_prompt", { default: "Check docker-compose.yml for errors" })],
      ];

    case "kubernetes":
      return [
        [t("ai.fallback.check_cluster_health", { default: "Check cluster health" }), t("ai.fallback.check_cluster_health_prompt", { default: "Check Kubernetes cluster and pod status" })],
        [t("ai.fallback.list_all_namespaces", { default: "List all namespaces" }), t("ai.fallback.list_all_namespaces_prompt", { default: "List Kubernetes namespaces and pod counts" })],
        [t("ai.fallback.find_failing_pods", { default: "Find failing pods" }), t("ai.fallback.find_failing_pods_prompt", { default: "Find pods not in Running state" })],
      ];

    case "images":
      return [
        [t("ai.fallback.scan_dangling_images", { default: "Scan dangling images" }), t("ai.fallback.scan_dangling_images_prompt", { default: "Find unused and dangling Docker images" })],
        [t("ai.fallback.analyze_image_sizes", { default: "Analyze image sizes" }), t("ai.fallback.analyze_image_sizes_prompt", { default: "Which images are using the most disk space?" })],
      ];

    case "volumes":
      return [
        [t("ai.fallback.list_dangling_volumes", { default: "List dangling volumes" }), t("ai.fallback.list_dangling_volumes_prompt", { default: "Find dangling volumes not attached to any container" })],
        [t("ai.fallback.inspect_volume_usage", { default: "Inspect volume usage" }), t("ai.fallback.inspect_volume_usage_prompt", { default: "Check volume sizes on host disk" })],
      ];

    case "networks":
      return [
        [t("ai.fallback.list_container_networks", { default: "List container networks" }), t("ai.fallback.list_container_networks_prompt", { default: "Inspect custom Docker bridge networks" })],
        [t("ai.fallback.inspect_port_bindings", { default: "Inspect port bindings" }), t("ai.fallback.inspect_port_bindings_prompt", { default: "List all exposed host ports and conflicting bindings" })],
      ];

    case "instances":
      return [
        [t("ai.fallback.list_colima_profiles", { default: "List Colima profiles" }), t("ai.fallback.list_colima_profiles_prompt", { default: "List all Colima profiles and their statuses" })],
        [t("ai.fallback.optimize_vm_settings", { default: "Optimize VM settings" }), t("ai.fallback.optimize_vm_settings_prompt", { default: "Recommend optimal CPU and memory settings for my Mac" })],
      ];

    case "security":
      return [
        [t("ai.fallback.run_security_audit", { default: "Run security audit" }), t("ai.fallback.run_security_audit_prompt", { default: "Audit Docker security posture and configuration" })],
        [t("ai.fallback.check_image_vulnerabilities", { default: "Check image vulnerabilities" }), t("ai.fallback.check_image_vulnerabilities_prompt", { default: "Scan local container images for vulnerabilities" })],
      ];

    case "dashboard":
    default:
      return [
        [t("ai.fallback.show_system_status", { default: "Show system status" }), t("ai.fallback.show_system_status_prompt", { default: "Show current Colima and Docker status" })],
        [t("ai.fallback.inspect_active_vm", { default: "Inspect active VM" }), t("ai.fallback.inspect_active_vm_prompt", { default: "Inspect resource allocation and disk usage of active VM" })],
        [t("ai.fallback.system_overview", { default: "System overview" }), t("ai.fallback.system_overview_prompt", { default: "Summarize system status, active instances, and containers" })],
      ];
  }
}

/**
 * Creates a structured LiveUiContextSnapshot object.
 */
export function createLiveUiContextSnapshot(options: LiveUiSnapshotOptions = {}): LiveUiContextSnapshot {
  const opts = options || {};
  const rawInstances = opts.colimaInstances || opts.instances;
  const instances = Array.isArray(rawInstances) ? rawInstances : [];
  const activeInstances = instances
    .filter((i): i is ColimaInstance => Boolean(i && typeof i === "object" && typeof i.status === "string" && i.status.toLowerCase() === "running"))
    .map((i) => (typeof i.name === "string" && i.name ? i.name : "default"))
    .filter(Boolean);

  let runningContainers = 0;
  let stoppedContainers = 0;
  const rawContainers = opts.dockerContainers || opts.containers;
  const containers = Array.isArray(rawContainers) ? rawContainers : [];
  for (const c of containers) {
    if (!c || typeof c !== "object") continue;
    const cont = c as ContainerLike;
    const state = typeof cont.State === "string"
      ? cont.State.toLowerCase()
      : typeof cont.state === "string"
        ? cont.state.toLowerCase()
        : "";
    const status = typeof cont.Status === "string"
      ? cont.Status.toLowerCase()
      : typeof cont.status === "string"
        ? cont.status.toLowerCase()
        : "";
    if (!state && !status) continue;
    if (state === "running" || status.startsWith("up")) {
      runningContainers++;
    } else {
      stoppedContainers++;
    }
  }

  return {
    currentPage: typeof opts.currentPage === "string" && opts.currentPage ? opts.currentPage : "dashboard",
    language: typeof opts.language === "string" && opts.language ? opts.language : "en",
    activeInstances,
    runningContainers,
    stoppedContainers,
    k8sConnected: Boolean(opts.k8sStatus?.connected),
    activeContext: typeof opts.activeContext === "string" && opts.activeContext
      ? opts.activeContext
      : (activeInstances[0] ?? "colima"),
  };
}

/**
 * Formats a concise (< 250 tokens) text snapshot of the live UI state
 * suitable for inclusion in agent prompt context.
 */
export function buildLiveUiContextSnapshot(options: LiveUiSnapshotOptions = {}): string {
  const opts = options || {};
  const page = typeof opts.currentPage === "string" && opts.currentPage ? opts.currentPage : "dashboard";
  const lang = typeof opts.language === "string" && opts.language ? opts.language : "en";

  const rawInstances = opts.colimaInstances || opts.instances;
  const instances = Array.isArray(rawInstances) ? rawInstances : [];
  const validInstances = instances.filter(
    (i): i is ColimaInstance => Boolean(i && typeof i === "object" && (typeof i.name === "string" || typeof i.status === "string"))
  );
  // VM summary
  let vmSummary = "none";
  if (validInstances.length > 0) {
    vmSummary = validInstances
      .map((i) => `${i.name || "unknown"}: ${i.status || "Unknown"} (${i.cpus || "?"} CPU, ${i.memory || "?"}GiB)`)
      .join("; ");
  }

  // Container counts
  let running = 0;
  let stopped = 0;
  const rawContainers = opts.dockerContainers || opts.containers;
  const containers = Array.isArray(rawContainers) ? rawContainers : [];
  if (containers.length > 0) {
    for (const c of containers) {
      if (!c || typeof c !== "object") continue;
      const cont = c as ContainerLike;
      const state = typeof cont.State === "string"
        ? cont.State.toLowerCase()
        : typeof cont.state === "string"
          ? cont.state.toLowerCase()
          : "";
      const status = typeof cont.Status === "string"
        ? cont.Status.toLowerCase()
        : typeof cont.status === "string"
          ? cont.status.toLowerCase()
          : "";
      if (!state && !status) continue;
      if (state === "running" || status.startsWith("up")) {
        running++;
      } else {
        stopped++;
      }
    }
  }

  // Kubernetes
  let k8sSummary = "disconnected";
  if (opts.k8sStatus?.connected) {
    k8sSummary = `connected (${opts.k8sStatus.pods ?? 0} pods, ${opts.k8sStatus.namespaces ?? 0} namespaces)`;
  }

  const activeContext = typeof opts.activeContext === "string" && opts.activeContext
    ? opts.activeContext
    : (validInstances[0]?.name || "colima");

  const lines = [
    `[LIVE_UI_CONTEXT]`,
    `Current Page: ${page}`,
    `Language: ${lang}`,
    `Colima VMs: ${vmSummary}`,
    `Containers: ${running} running, ${stopped} stopped`,
    `Kubernetes: ${k8sSummary}`,
    `Active Context: ${activeContext}`,
  ];

  const groundTruthRule = "Ground Truth Rule: Prioritize [LIVE_UI_CONTEXT] and fresh [Tool Results] over older chat turns.";

  if (containers.length > 0) {
    const detailLines: string[] = [];
    const maxContainers = Math.min(containers.length, 6);
    for (let i = 0; i < maxContainers; i++) {
      const c = containers[i];
      if (!c || typeof c !== "object") continue;
      const cont = c as ContainerLike;

      const rawName = Array.isArray(cont.Names)
        ? cont.Names[0]
        : typeof cont.Names === "string"
          ? cont.Names
          : typeof cont.name === "string"
            ? cont.name
            : typeof cont.Id === "string"
              ? cont.Id
              : typeof cont.id === "string"
                ? cont.id
                : "unknown";
      let cleanName = (typeof rawName === "string" ? rawName.replace(/^\//, "") : "") || (cont.Id ? cont.Id.slice(0, 12) : "unknown");
      cleanName = cleanName.trim();
      if (cleanName.length > 30) {
        cleanName = cleanName.slice(0, 27) + "...";
      }

      const rawStatus = typeof cont.Status === "string" && cont.Status
        ? cont.Status
        : typeof cont.status === "string" && cont.status
          ? cont.status
          : typeof cont.State === "string" && cont.State
            ? cont.State
            : typeof cont.state === "string" && cont.state
              ? cont.state
              : "unknown";
      let cleanStatus = (typeof rawStatus === "string" ? rawStatus : "") || "unknown";
      cleanStatus = cleanStatus.trim();
      if (cleanStatus.length > 15) {
        cleanStatus = cleanStatus.slice(0, 12) + "...";
      }

      const rawImage = typeof cont.Image === "string" && cont.Image
        ? cont.Image
        : typeof cont.image === "string" && cont.image
          ? cont.image
          : "";
      let cleanImage = (typeof rawImage === "string" ? rawImage : "").replace(/@sha256:[a-f0-9]+/i, "").trim();
      if (cleanImage.length > 25) {
        cleanImage = cleanImage.slice(0, 22) + "...";
      }

      const desc = cleanImage ? `${cleanName} [${cleanStatus}, ${cleanImage}]` : `${cleanName} [${cleanStatus}]`;
      const line = `- ${desc}`;

      // Check accumulation budget <= 800 chars total for lines + detailLines + line + groundTruthRule
      const candidateTotal = [...lines, "Container Details:", ...detailLines, line, groundTruthRule].join("\n").length;
      if (candidateTotal > 800) {
        break;
      }
      detailLines.push(line);
    }

    if (detailLines.length > 0) {
      lines.push("Container Details:", ...detailLines);
    }
  }

  lines.push(groundTruthRule);
  let result = lines.join("\n");
  if (result.length >= 900) {
    result = result.slice(0, 895) + "...";
  }
  return result;
}
