// eslint-disable-next-line @typescript-eslint/no-require-imports -- CommonJS build script; require is the only module mechanism available in .cjs
const fs = require('fs');

const content = fs.readFileSync('src/lib/api.ts', 'utf-8');
const sections = content.split('// ===== ');

const mapping = {
  'Runtime Detection': 'client.ts',
  'Unified call function': 'client.ts',
  'API Token (for SSE/browser mode auth)': 'client.ts',
  'Types': 'types.ts',
  'Colima API': 'colima.ts',
  'Docker API': 'docker.ts',
  'Volumes API': 'volumes.ts',
  'Networks API': 'networks.ts',
  'System API': 'system.ts',
  'System Methods (convenience facade)': 'system.ts',
  'Compose API': 'compose.ts',
  'Models API': 'models.ts',
  'Kubernetes API': 'k8s.ts',
  'Kind API': 'k8s.ts',
  'Lima API': 'lima.ts',
  'AI Chat API': 'ai.ts',
  'Settings API': 'settings.ts',
  'Knowledge Bank API': 'knowledgeBank.ts',
  'Shell Sandbox API': 'sandbox.ts'
};

const output = {
  'client.ts': 'export { getApiToken } from "./client"; // Placeholder\n',
};

// Add standard imports to each file
for (const file of new Set(Object.values(mapping))) {
  if (file !== 'client.ts' && file !== 'types.ts') {
    output[file] = `import { call } from "./client";\nimport type * as Types from "./types";\n\n`;
  } else {
    output[file] = '';
  }
}
// Add explicit type imports to avoid `Types.` prefix rewriting
for (const file of new Set(Object.values(mapping))) {
  if (file !== 'client.ts' && file !== 'types.ts') {
    output[file] = `import { call } from "./client";\nimport type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";\n\n`;
  }
}

let currentIndexContent = '';

for (let i = 1; i < sections.length; i++) {
  const section = sections[i];
  const title = section.split(' =====')[0].trim();
  const body = section.substring(section.indexOf('\n') + 1);
  
  if (mapping[title]) {
    let file = mapping[title];
    output[file] += `// ===== ${title} =====\n${body}\n`;
  } else {
    console.warn('Unknown section:', title);
  }
}

// Special fix for client.ts exports
output['client.ts'] = output['client.ts'].replace(/async function call/g, 'export async function call');
output['client.ts'] = output['client.ts'].replace(/const isTauri = \(\)/g, 'export const isTauri = ()');
output['client.ts'] = output['client.ts'].replace(/const API_BASE/g, 'export const API_BASE');
output['client.ts'] = output['client.ts'].replace(/async function getInvoke/g, 'export async function getInvoke');


for (const [file, content] of Object.entries(output)) {
  fs.writeFileSync(`src/lib/api/${file}`, content.replace(/\n+$/, '\n'));
  
  const moduleName = file.replace('.ts', '');
  currentIndexContent += `export * from "./${moduleName}";\n`;
}

fs.writeFileSync('src/lib/api/index.ts', currentIndexContent);
fs.writeFileSync('src/lib/api.ts', 'export * from "./api/index";\n');
console.log('Done splitting api.ts');
