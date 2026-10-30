/**
 * Shared Test Fixtures & Vectors for E2E Test Suite (Tiers 1-4)
 */

import type {
  ToolStepPayload,
  ParsedChoicePrompt,
  StarterPromptContext,
  FallbackSuggestionContext,
} from './contractAdapter';

// ---------------------------------------------------------------------------
// Markdown Fixtures
// ---------------------------------------------------------------------------
export const MARKDOWN_HAPPY_PATH = `
# Colima Instance Diagnostic

Here is the current operational assessment for your VM environment:

| Service | Status | Port |
| :--- | :---: | ---: |
| Docker Daemon | Running | 2375 |
| Kubernetes API | Active | 6443 |
| Buildx Builder | Ready | N/A |

### Recommended Action

To check your container status, run:

\`\`\`bash
docker ps -a --format "table {{.ID}}\t{{.Names}}\t{{.Status}}"
\`\`\`

> Note: Ensure your Colima socket is active before executing CLI commands.

- Check lima socket
- Restart docker service
- Inspect disk usage
`;

export const MARKDOWN_THINK_CLOSED = `
<think>
Analyzing container failure logs.
Detected exit code 137 (OOMKilled).
Memory allocation limit was 512MB, peak usage reached 514MB.
</think>
The container terminated because it exceeded its allocated memory quota (OOMKilled).
`;

export const MARKDOWN_THINK_STREAMING = `
<think>
Examining Colima VM lima.yaml configuration.
Checking disk allocation...
`;

export const MARKDOWN_SHELL_BLOCKS = [
  {
    lang: 'bash',
    code: 'colima start --cpu 4 --memory 8',
    expectedShell: true,
  },
  {
    lang: 'sh',
    code: 'docker run -d -p 8080:80 nginx:alpine',
    expectedShell: true,
  },
  {
    lang: 'zsh',
    code: 'kubectl get pods -A',
    expectedShell: true,
  },
  {
    lang: '',
    code: 'colima status\ndocker info',
    expectedShell: true, // detected via first line prefix
  },
  {
    lang: 'python',
    code: 'import os\nprint("Hello from python")',
    expectedShell: false,
  },
  {
    lang: 'json',
    code: '{\n  "status": "ok",\n  "count": 42\n}',
    expectedShell: false,
  },
];

export const MARKDOWN_XSS_VECTORS = [
  '<script>alert("xss")</script>',
  '<img src=x onerror=alert(1)>',
  '"><svg onload=alert(document.cookie)>',
  '[Click me](javascript:alert("xss"))',
  '<iframe src="https://attacker.example.com"></iframe>',
];

// ---------------------------------------------------------------------------
// Tool Step Fixtures
// ---------------------------------------------------------------------------
export const TOOL_STEP_PENDING_NORMAL: ToolStepPayload = {
  version: 1,
  kind: 'event',
  name: 'restart-container',
  title: 'Restart Container web-app',
  safety: 'NORMAL',
  status: 'pending_approval',
  rawInput: JSON.stringify({ id: 'cnt-123', name: 'web-app' }),
  timestamp: Date.now(),
};

export const TOOL_STEP_PENDING_DANGEROUS: ToolStepPayload = {
  version: 1,
  kind: 'command',
  name: 'prune-system',
  title: 'Prune All Stopped Containers & Unused Volumes',
  safety: 'DANGEROUS',
  status: 'pending_approval',
  warning: 'This action permanently removes all stopped containers and anonymous volumes.',
  rawInput: 'docker system prune -a --volumes -f',
  timestamp: Date.now(),
};

export const TOOL_STEP_RUNNING: ToolStepPayload = {
  version: 1,
  kind: 'diagnose',
  name: 'diagnose-colima',
  title: 'Diagnosing Colima VM Network Socket',
  safety: 'SAFE',
  status: 'running',
  rawInput: 'colima status --json',
  timestamp: Date.now(),
};

export const TOOL_STEP_SUCCEEDED: ToolStepPayload = {
  version: 1,
  kind: 'query',
  name: 'list-containers',
  title: 'List All Containers',
  safety: 'SAFE',
  status: 'succeeded',
  output: 'CONTAINER ID   IMAGE          COMMAND                  STATUS\n8a7b6c5d4e3f   nginx:alpine   "/docker-entrypoint.…"   Up 2 hours',
  exitCode: 0,
  timestamp: Date.now(),
};

export const TOOL_STEP_FAILED: ToolStepPayload = {
  version: 1,
  kind: 'command',
  name: 'start-instance',
  title: 'Start Colima Instance "k8s-cluster"',
  safety: 'NORMAL',
  status: 'failed',
  output: 'error starting instance: socket connection timeout at ~/.colima/k8s-cluster/daemon.sock',
  exitCode: 1,
  timestamp: Date.now(),
};

export const TOOL_STEP_DENIED: ToolStepPayload = {
  version: 1,
  kind: 'command',
  name: 'delete-instance',
  title: 'Delete Instance "legacy-vm"',
  safety: 'DANGEROUS',
  status: 'denied',
  warning: 'Cancelled by user',
  timestamp: Date.now(),
};

// ---------------------------------------------------------------------------
// [ASK_CHOICE] Fixtures
// ---------------------------------------------------------------------------
export const CHOICE_JSON_SINGLE: ParsedChoicePrompt = {
  question: 'Select CPU and RAM configuration for your new Colima VM:',
  multi: false,
  options: [
    { label: 'Standard', description: '4 CPU · 8GB RAM', value: '4 CPU, 8GB RAM' },
    { label: 'High Performance', description: '8 CPU · 16GB RAM', value: '8 CPU, 16GB RAM' },
    { label: 'Minimal', description: '2 CPU · 4GB RAM', value: '2 CPU, 4GB RAM' },
  ],
};

export const CHOICE_JSON_MULTI: ParsedChoicePrompt = {
  question: 'Select unused images to remove:',
  multi: true,
  options: [
    { label: 'redis:6-alpine (104MB)', value: 'redis:6-alpine' },
    { label: 'node:18-alpine (165MB)', value: 'node:18-alpine' },
    { label: 'postgres:14 (376MB)', value: 'postgres:14' },
  ],
};

export const RAW_CHOICE_JSON_SINGLE_TAG = `[ASK_CHOICE: {"question": "Select CPU and RAM configuration for your new Colima VM:", "multi": false, "options": [{"label": "Standard", "description": "4 CPU · 8GB RAM", "value": "4 CPU, 8GB RAM"}, {"label": "High Performance", "description": "8 CPU · 16GB RAM", "value": "8 CPU, 16GB RAM"}, {"label": "Minimal", "description": "2 CPU · 4GB RAM", "value": "2 CPU, 4GB RAM"}]}]`;

export const RAW_CHOICE_PIPE_TAG = `[ASK_CHOICE: Which container action do you want to run? | Inspect logs | Restart service | Open shell | Remove container]`;

export const RAW_CHOICE_MALFORMED_JSON = `[ASK_CHOICE: {"question": "Unfinished JSON string, "options": [{"label": "opt1"}]`;

// ---------------------------------------------------------------------------
// [SUGGEST] & Partial Streaming Tag Fixtures
// ---------------------------------------------------------------------------
export const SUGGEST_TRIPLE_TAG = `[SUGGEST: Check logs | Show the last 50 lines][SUGGEST: Restart container][SUGGEST: Inspect resources]`;

export const SUGGEST_SURPLUS_TAG = `[SUGGEST: Chip 1][SUGGEST: Chip 2][SUGGEST: Chip 3][SUGGEST: Chip 4][SUGGEST: Chip 5]`;

export const SUGGEST_DUPLICATE_CHIPS = `[SUGGEST: Verify status][SUGGEST: Verify status][SUGGEST: View details]`;

export const PARTIAL_STREAM_CHUNKS = [
  {
    raw: 'Processing your request... [QUERY: list-containers',
    expectedClean: 'Processing your request...',
    expectedPending: 'query',
  },
  {
    raw: 'Action initiated. [EVENT_APPROVE: stop-instance | {"name": "default"',
    expectedClean: 'Action initiated.',
    expectedPending: 'event_approve',
  },
  {
    raw: 'I have options for you: [ASK_CHOICE: {"question": "Select VM',
    expectedClean: 'I have options for you:',
    expectedPending: 'choice',
  },
  {
    raw: 'Here is the summary. [SUGGEST: View container',
    expectedClean: 'Here is the summary.',
    expectedPending: 'suggest',
  },
  {
    raw: 'Check docs at [Colima GitHub](https://github.com/abiosoft/colima) for setup.',
    expectedClean: 'Check docs at [Colima GitHub](https://github.com/abiosoft/colima) for setup.',
    expectedPending: null, // genuine link, not suppressed
  },
  {
    raw: 'Matrix array: `[1, 2, 3]` and data list [A, B, C].',
    expectedClean: 'Matrix array: `[1, 2, 3]` and data list [A, B, C].',
    expectedPending: null,
  },
];

// ---------------------------------------------------------------------------
// Store & UI Context Snapshots
// ---------------------------------------------------------------------------
export const MOCK_CTX_STOPPED_INSTANCE: StarterPromptContext = {
  currentPage: 'dashboard',
  colimaInstances: [
    { name: 'default', status: 'Stopped' },
    { name: 'k8s-dev', status: 'Running' },
  ],
  containers: [
    { id: 'cnt-1', name: 'api-server', state: 'running' },
  ],
};

export const MOCK_CTX_EXITED_CONTAINER: StarterPromptContext = {
  currentPage: 'containers',
  colimaInstances: [
    { name: 'default', status: 'Running' },
  ],
  containers: [
    { id: 'c-web-99', name: 'frontend-web', state: 'exited' },
    { id: 'c-db-01', name: 'postgres-db', state: 'running' },
  ],
};

export const MOCK_CTX_HEALTHY_COMPOSE: StarterPromptContext = {
  currentPage: 'compose',
  colimaInstances: [
    { name: 'default', status: 'Running' },
  ],
  containers: [
    { id: 'c-app', name: 'app-service', state: 'running' },
  ],
};

export const MOCK_FALLBACK_AFTER_QUERY: FallbackSuggestionContext = {
  toolsExecuted: [{ kind: 'query', name: 'list-containers' }],
  currentPage: 'containers',
};

export const MOCK_FALLBACK_AFTER_DIAGNOSE: FallbackSuggestionContext = {
  toolsExecuted: [{ kind: 'diagnose', name: 'DIAGNOSE' }],
  currentPage: 'dashboard',
};
