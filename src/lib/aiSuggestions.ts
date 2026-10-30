import type { ColimaInstance, DockerContainer } from "./api/types";

interface ContainerLike {
  Id?: string;
  id?: string;
  Names?: string | string[];
  name?: string;
  State?: string;
  state?: string;
  Status?: string;
  status?: string;
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
          title: `Investigate stopped instance ${instName}`,
          prompt: `Why is Colima instance ${instName} stopped? Check its status and suggest how to start it. Investigate stopped/unhealthy instance ${instName}.`,
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
          ? `Container "${cleanName}" exited`
          : `Analyze exited container ${cleanName}`;
        const prompt = cont.Names !== undefined
          ? `Analyze why container ${cleanName} exited`
          : `Container ${cleanName} (${rawId || cleanName}) exited. Analyze why container ${cleanName} exited.`;
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
      title: "Optimize VM resources",
      prompt: "What are the recommended CPU, memory, and disk settings for my Colima instances?",
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
          title: "Create or optimize VM",
          prompt: "Create or optimize a Colima VM configuration",
          icon: "vm",
          category: "context",
        },
        {
          id: "starter-instances-mounts",
          title: "Explain mount types",
          prompt: "Explain difference between virtiofs, sshfs, and 9p mounts",
          icon: "vm",
          category: "context",
        },
        {
          id: "starter-instances-resize",
          title: "Resize CPU and memory",
          prompt: "How to resize CPU and memory for an existing VM",
          icon: "vm",
          category: "context",
        },
      ];

    case "containers":
      return [
        {
          id: "starter-containers-resources",
          title: "Inspect resource consumption",
          prompt: "List running containers and inspect their CPU and memory usage",
          icon: "container",
          category: "context",
        },
        {
          id: "starter-containers-list",
          title: "List active containers",
          prompt: "List running containers and inspect resource usage",
          icon: "container",
          category: "context",
        },
        {
          id: "starter-containers-mem",
          title: "Top memory consumers",
          prompt: "Which containers are consuming the most memory?",
          icon: "container",
          category: "context",
        },
        {
          id: "starter-containers-prune",
          title: "Clean stopped containers",
          prompt: "Clean up all stopped containers and reclaim space",
          icon: "container",
          category: "context",
        },
      ];

    case "compose":
      return [
        {
          id: "starter-compose-diagnose",
          title: "Diagnose Compose services",
          prompt: "Diagnose compose services and check health",
          icon: "compose",
          category: "context",
        },
        {
          id: "starter-compose-validate",
          title: "Validate docker-compose",
          prompt: "Validate docker-compose file for syntax or environment errors",
          icon: "compose",
          category: "context",
        },
        {
          id: "starter-compose-logs",
          title: "Inspect compose logs",
          prompt: "Check recent error logs across all compose stack services",
          icon: "compose",
          category: "context",
        },
      ];

    case "kubernetes":
      return [
        {
          id: "starter-k8s-status",
          title: "Cluster & pod health",
          prompt: "Check K8s cluster status and pod health",
          icon: "k8s",
          category: "context",
        },
        {
          id: "starter-k8s-failing",
          title: "Find failing pods",
          prompt: "Find pods in CrashLoopBackOff or Pending status",
          icon: "k8s",
          category: "context",
        },
        {
          id: "starter-k8s-namespaces",
          title: "List namespaces",
          prompt: "List Kubernetes namespaces and workload distributions",
          icon: "k8s",
          category: "context",
        },
      ];

    case "images":
      return [
        {
          id: "starter-images-unused",
          title: "Scan unused images",
          prompt: "Scan unused container images and clean up disk space",
          icon: "container",
          category: "context",
        },
        {
          id: "starter-images-optimize",
          title: "Optimize image sizes",
          prompt: "Analyze large Docker images and recommend size optimizations",
          icon: "container",
          category: "context",
        },
      ];

    case "volumes":
      return [
        {
          id: "starter-volumes-inspect",
          title: "Inspect volume sizes",
          prompt: "Inspect volume sizes and orphaned data",
          icon: "container",
          category: "context",
        },
        {
          id: "starter-volumes-prune",
          title: "Remove dangling volumes",
          prompt: "Identify and remove dangling volumes safely",
          icon: "container",
          category: "context",
        },
      ];

    case "networks":
      return [
        {
          id: "starter-networks-inspect",
          title: "Inspect networks & ports",
          prompt: "Inspect container bridge networks and port mappings",
          icon: "container",
          category: "context",
        },
        {
          id: "starter-networks-conflicts",
          title: "Fix port conflicts",
          prompt: "Troubleshoot container port conflicts and host reachability",
          icon: "container",
          category: "context",
        },
      ];

    case "security":
      return [
        {
          id: "starter-security-scan",
          title: "Vulnerability scan",
          prompt: "Run vulnerability scan and evaluate security posture",
          icon: "security",
          category: "context",
        },
        {
          id: "starter-security-audit",
          title: "Audit privileged containers",
          prompt: "Audit privileged containers and host port exposures",
          icon: "security",
          category: "context",
        },
      ];

    case "models":
      return [
        {
          id: "starter-models-config",
          title: "Check AI models",
          prompt: "Check available AI models and configure endpoints",
          icon: "general",
          category: "context",
        },
        {
          id: "starter-models-ollama",
          title: "Test Ollama connection",
          prompt: "Test Ollama connection and list installed local models",
          icon: "general",
          category: "context",
        },
      ];

    case "settings":
      return [
        {
          id: "starter-general-optimize",
          title: "Optimize VM resources",
          prompt: "What are the recommended CPU, memory, and disk settings for my Colima instances?",
          icon: "general",
          category: "help",
        },
      ];

    case "dashboard":
    default:
      return [
        {
          id: "starter-dashboard-overview",
          title: "System overview",
          prompt: "Show system overview and active instances",
          icon: "general",
          category: "context",
        },
        {
          id: "starter-dashboard-resources",
          title: "Check resource usage",
          prompt: "Check resource usage across all running containers and VMs",
          icon: "general",
          category: "context",
        },
        {
          id: "starter-dashboard-health",
          title: "Runtime health check",
          prompt: "Inspect cluster and runtime health",
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
      ["Check CPU & memory", "Check CPU and memory usage of running containers"],
      ["View container logs", "Show recent logs for running containers"],
      ["Check CPU & RAM", "Inspect CPU and memory usage of running containers"],
    ];
  }

  if (tools.some((t) => t.includes("log"))) {
    return [
      ["Analyze errors", "Analyze these container logs and propose troubleshooting fixes"],
      ["Check restart policy", "Inspect restart policy and health check config"],
      ["View environment", "Inspect container environment variables"],
    ];
  }

  if (tools.some((t) => t.includes("diagnose") || t.includes("heal") || t.includes("compose"))) {
    return [
      ["Propose fixes", "Propose automated fixes for diagnosed issues"],
      ["Apply recommended fix", "Propose and execute automated fix steps for the diagnosed issue"],
      ["Validate compose file", "Check docker-compose.yml for configuration mistakes"],
    ];
  }

  if (tools.some((t) => t.includes("instance") || t.includes("colima"))) {
    return [
      ["Inspect VM resources", "Show CPU, memory, and disk usage for Colima VM"],
      ["Check mount type", "Verify active mount type and filesystem performance"],
      ["View Lima VM logs", "Check Lima daemon and serial logs for warnings"],
    ];
  }

  if (tools.some((t) => t.includes("k8s") || t.includes("pod"))) {
    return [
      ["Check pod health", "List pods that are restarting or in CrashLoopBackOff"],
      ["Inspect cluster events", "Show recent Kubernetes cluster warning events"],
      ["Check node capacity", "Inspect node CPU and memory allocatable capacity"],
    ];
  }

  if (tools.some((t) => t.includes("sec") || t.includes("triage") || t.includes("vuln"))) {
    return [
      ["Review CVEs", "Filter and explain critical and high-severity CVEs"],
      ["Generate patch", "Propose minimal Dockerfile or package updates to fix CVEs"],
      ["Audit capabilities", "Check for containers running with root or elevated capabilities"],
    ];
  }

  if (tools.some((t) => t.includes("event") || t.includes("command"))) {
    return [
      ["Verify status", "Verify that the operation completed successfully"],
      ["Next steps", "What should I do next after this operation?"],
      ["Inspect logs", "Check logs to confirm operation result"],
    ];
  }

  // Fallback based on current page
  const page = typeof ctx.currentPage === "string" && ctx.currentPage ? ctx.currentPage.toLowerCase() : "dashboard";
  switch (page) {
    case "containers":
      return [
        ["List running containers", "List all active containers and their port mappings"],
        ["Check resource usage", "Check container resource utilization"],
        ["Prune stopped", "How do I remove all stopped containers?"],
      ];

    case "compose":
      return [
        ["Diagnose active stack", "Diagnose compose services and check health"],
        ["View compose logs", "Show recent logs for compose services"],
        ["Validate configuration", "Check docker-compose.yml for errors"],
      ];

    case "kubernetes":
      return [
        ["Check cluster health", "Check Kubernetes cluster and pod status"],
        ["List all namespaces", "List Kubernetes namespaces and pod counts"],
        ["Find failing pods", "Find pods not in Running state"],
      ];

    case "images":
      return [
        ["Scan dangling images", "Find unused and dangling Docker images"],
        ["Analyze image sizes", "Which images are using the most disk space?"],
      ];

    case "volumes":
      return [
        ["List dangling volumes", "Find dangling volumes not attached to any container"],
        ["Inspect volume usage", "Check volume sizes on host disk"],
      ];

    case "networks":
      return [
        ["List container networks", "Inspect custom Docker bridge networks"],
        ["Inspect port bindings", "List all exposed host ports and conflicting bindings"],
      ];

    case "instances":
      return [
        ["List Colima profiles", "List all Colima profiles and their statuses"],
        ["Optimize VM settings", "Recommend optimal CPU and memory settings for my Mac"],
      ];

    case "security":
      return [
        ["Run security audit", "Audit Docker security posture and configuration"],
        ["Check image vulnerabilities", "Scan local container images for vulnerabilities"],
      ];

    case "dashboard":
    default:
      return [
        ["Show system status", "Show current Colima and Docker status"],
        ["Inspect active VM", "Inspect resource allocation and disk usage of active VM"],
        ["System overview", "Summarize system status, active instances, and containers"],
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

  return [
    `[LIVE_UI_CONTEXT]`,
    `Current Page: ${page}`,
    `Language: ${lang}`,
    `Colima VMs: ${vmSummary}`,
    `Containers: ${running} running, ${stopped} stopped`,
    `Kubernetes: ${k8sSummary}`,
    `Active Context: ${activeContext}`,
  ].join("\n");
}
