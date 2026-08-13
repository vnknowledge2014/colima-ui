use serde::{Deserialize, Serialize};










#[derive(Deserialize)]
pub struct AutostartRequest {
    pub enable: bool,
}




#[derive(Deserialize)]
pub struct ToolQuery {
    pub name: String,
}



#[derive(Serialize)]
pub struct ToolStatus {
    pub installed: bool,
    pub version: String,
}



// ===== Host Hardware Specs (HTTP) =====


// ===== Install dependency with method =====

#[derive(Deserialize)]
pub struct InstallDepRequest {
    pub name: String,
    #[serde(default = "default_install_method")]
    pub method: String,
}







// ===== Colima routes =====


#[derive(Deserialize)]
pub struct ProfileQuery {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub force: bool,
}







#[derive(Deserialize)]
pub struct K8sQuery {
    #[serde(default = "default_profile")]
    pub profile: String,
    pub action: String,
}





// ===== Docker routes =====

#[derive(Deserialize)]
pub struct ContainerQuery {
    #[serde(default)]
    pub all: bool,
}




#[derive(Deserialize)]
pub struct ContainerIdQuery {
    #[serde(rename = "containerId")]
    pub container_id: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_lines")]
    pub lines: u32,
}










// ===== Image management routes =====

#[derive(Deserialize)]
pub struct ImageIdQuery {
    #[serde(default, alias = "imageId")]
    pub image_id: String,
    #[serde(default)]
    pub force: Option<bool>,
}



#[derive(Deserialize)]
pub struct ImagePullQuery {
    #[serde(default, alias = "imageName")]
    pub image_name: String,
}



#[derive(Deserialize)]
pub struct ImageTagBody {
    pub source: String,
    pub target: String,
}



// ===== Background transfers =====
//
// `destDir` and `fileName` arrive separately on purpose: containment of the
// written path is checked against the folder the user chose, which a single
// combined path could not express. See `commands::file_transfer`.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSaveBody {
    pub images: Vec<String>,
    pub dest_dir: String,
    pub file_name: String,
    #[serde(default)]
    pub overwrite: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLoadBody {
    pub tar_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyToContainerBody {
    pub container_id: String,
    pub host_path: String,
    pub container_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyFromContainerBody {
    pub container_id: String,
    pub container_path: String,
    pub dest_dir: String,
    pub file_name: String,
    #[serde(default)]
    pub overwrite: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTransferBody {
    pub job_id: String,
}

// ===== Diagnostics =====

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundleBody {
    /// The message the user is reporting, if they started from one. Feeds the
    /// signature only; it is never stored as a section.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub log_lines: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDiagnosticBundleBody {
    pub bundle: crate::commands::diagnostics::DiagnosticBundle,
    /// Section ids the user left checked.
    pub include: Vec<String>,
    pub dest_dir: String,
    pub file_name: String,
    #[serde(default)]
    pub overwrite: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsIntervalBody {
    pub ms: u64,
}

#[derive(Deserialize)]
pub struct PruneQuery {
    #[serde(default)]
    pub all: Option<bool>,
}










// ===== Volume routes =====

#[derive(Deserialize)]
pub struct VolumeNameQuery {
    pub name: String,
    #[serde(default)]
    pub force: Option<bool>,
}



#[derive(Deserialize)]
pub struct CreateVolumeBody {
    pub name: String,
    #[serde(default)]
    pub driver: String,
}








// ===== Network routes =====

#[derive(Deserialize)]
pub struct NetworkNameQuery {
    pub name: String,
}



#[derive(Deserialize)]
pub struct CreateNetworkBody {
    pub name: String,
    #[serde(default)]
    pub driver: String,
    #[serde(default)]
    pub subnet: String,
}








// ===== Container enhancement routes =====

#[derive(Deserialize)]
pub struct ContainerExecBody {
    #[serde(alias = "containerId")]
    pub container_id: String,
    pub command: String,
}



#[derive(Deserialize)]
pub struct RunContainerBody {
    pub image: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default, alias = "envVars")]
    pub env_vars: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default = "default_true")]
    pub detach: bool,
    #[serde(default, alias = "removeOnExit")]
    pub remove_on_exit: bool,
    #[serde(default, alias = "extraArgs")]
    pub extra_args: Vec<String>,
}



#[derive(Deserialize)]
pub struct RenameContainerBody {
    #[serde(alias = "containerId")]
    pub container_id: String,
    #[serde(alias = "newName")]
    pub new_name: String,
}







// ===== Model routes =====

#[derive(Deserialize)]
pub struct ModelQuery {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    #[serde(rename = "modelName")]
    pub model_name: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub runner: String,
}







// ===== Terminal session routes (browser mode) =====

#[derive(Deserialize)]
pub struct TerminalCreateParams {
    pub session_id: String,
    pub profile: String,
    #[serde(default = "default_vm_type")]
    pub vm_type: String,
}



#[derive(Deserialize)]
pub struct TerminalWriteParams {
    pub session_id: String,
    pub data: String,
}



#[derive(Deserialize)]
pub struct TerminalSessionParams {
    pub session_id: String,
}








// ===== AI Chat route =====



// ===== AI Diagnostics routes (SearXNG + HTML→MD) =====



// ===== AI Context route (browser mode) =====


// ===== Docker System routes =====


#[derive(Deserialize)]
pub struct DockerPruneQuery {
    #[serde(default)]
    pub confirm: bool,
}




// ===== Lima routes =====

#[derive(Deserialize)]
pub struct LimaNameBody {
    pub name: String,
}



#[derive(Deserialize)]
pub struct LimaDeleteBody {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}



#[derive(Deserialize)]
pub struct LimaShellBody {
    pub name: String,
    pub command: String,
}










#[derive(Deserialize)]
pub struct LimaCreateBody {
    pub name: String,
    #[serde(default = "default_cpus")]
    pub cpus: u32,
    #[serde(default = "default_memory")]
    pub memory: u32,
    #[serde(default = "default_disk")]
    pub disk: u32,
    #[serde(default)]
    pub template: String,
}




// ===== Kubernetes routes =====

#[derive(Deserialize)]
pub struct K8sNsQuery {
    #[serde(default)]
    pub namespace: String,
}



#[derive(Deserialize)]
pub struct K8sPodLogQuery {
    pub namespace: String,
    pub pod: String,
    #[serde(default = "default_log_lines")]
    pub lines: u32,
}



#[derive(Deserialize)]
pub struct K8sDeletePodBody {
    pub namespace: String,
    pub pod: String,
}



#[derive(Deserialize)]
pub struct K8sDescribeQuery {
    pub namespace: String,
    #[serde(alias = "resourceType")]
    pub resource_type: String,
    pub name: String,
}



#[derive(Deserialize)]
pub struct K8sScaleBody {
    pub namespace: String,
    pub deployment: String,
    pub replicas: u32,
}














// Generic K8s resource list endpoint — handles configmaps, secrets, statefulsets, etc.
#[derive(Deserialize)]
pub struct K8sResourceQuery {
    pub resource: String,
    #[serde(default)]
    pub namespace: String,
}




// Generic K8s resource delete
#[derive(Deserialize)]
pub struct K8sDeleteBody {
    #[serde(alias = "resourceType")]
    pub resource_type: String,
    pub namespace: String,
    pub name: String,
}




// Rollout restart (for deployments, statefulsets, daemonsets)
#[derive(Deserialize)]
pub struct K8sRestartBody {
    #[serde(alias = "resourceType")]
    pub resource_type: String,
    pub namespace: String,
    pub name: String,
}




// Get resource YAML
#[derive(Deserialize)]
pub struct K8sYamlQuery {
    #[serde(alias = "resourceType")]
    pub resource_type: String,
    pub namespace: String,
    pub name: String,
}








#[derive(Deserialize)]
pub struct K8sContextBody {
    pub context: String,
}




// ===== Phase 2: YAML Apply =====

#[derive(Deserialize)]
pub struct K8sApplyBody {
    pub yaml: String,
    #[serde(default)]
    pub namespace: String,
}



#[derive(Deserialize)]
pub struct K8sPortForwardBody {
    pub namespace: String,
    #[serde(alias = "resourceType", default = "default_pod_type")]
    pub resource_type: String,
    pub name: String,
    #[serde(alias = "localPort")]
    pub local_port: u16,
    #[serde(alias = "remotePort")]
    pub remote_port: u16,
}




#[derive(Deserialize)]
pub struct K8sPortForwardStopBody {
    #[serde(alias = "localPort")]
    pub local_port: u16,
}





// ===== Phase 2: Exec Shell =====

#[derive(Deserialize)]
pub struct K8sExecBody {
    pub namespace: String,
    pub pod: String,
    #[serde(default)]
    pub container: String,
}




// ===== Phase 2: Container-level logs =====

#[derive(Deserialize)]
pub struct K8sContainerLogQuery {
    pub namespace: String,
    pub pod: String,
    #[serde(default)]
    pub container: String,
    #[serde(default = "default_log_lines")]
    pub lines: u32,
    #[serde(default)]
    pub previous: bool,
}





// ===== Phase 2: Node operations =====

#[derive(Deserialize)]
pub struct K8sNodeBody {
    pub name: String,
    pub action: String, // cordon, uncordon, drain
}




// ===== Phase 2: Kind cluster management =====


#[derive(Deserialize)]
pub struct KindCreateBody {
    pub name: String,
    #[serde(default)]
    pub image: String,
}




#[derive(Deserialize)]
pub struct KindDeleteBody {
    pub name: String,
}




// ===== Phase 3: Generic Scale =====

#[derive(Deserialize)]
pub struct K8sGenericScaleBody {
    #[serde(alias = "resourceType", default = "default_deployment_type")]
    pub resource_type: String,
    pub namespace: String,
    pub name: String,
    pub replicas: u32,
}




// ===== Phase 3: Cluster Health Analysis =====


// ===== CRD Support =====


#[derive(Deserialize)]
pub struct K8sCrdQuery {
    pub resource: String, // e.g. "kustomizations.kustomize.toolkit.fluxcd.io"
    #[serde(default)]
    pub namespace: String,
}




// ===== Real-time Log Streaming via SSE =====

#[derive(Deserialize)]
pub struct K8sLogStreamQuery {
    pub namespace: String,
    pub pod: String,
    #[serde(default)]
    pub container: String,
    #[serde(default = "default_tail_lines")]
    pub tail: u32,
}




// ===== HTTP Benchmark =====

#[derive(Deserialize)]
pub struct BenchmarkBody {
    pub url: String,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_requests")]
    pub requests: u32,
    #[serde(default)]
    pub method: String, // GET, POST, PUT, DELETE
}




// ===== Compose routes =====

#[derive(Deserialize)]
pub struct ComposeUpBody {
    #[serde(alias = "projectDir", default)]
    pub project_dir: String,
    #[serde(default = "default_true")]
    pub detach: bool,
}



#[derive(Deserialize)]
pub struct ComposeProjectBody {
    #[serde(alias = "projectName")]
    pub project_name: String,
}



#[derive(Deserialize)]
pub struct ComposeLogsQuery {
    #[serde(alias = "projectName")]
    pub project_name: String,
    #[serde(default = "default_log_lines")]
    pub lines: u32,
}



#[derive(Deserialize)]
pub struct KbFeedbackRequest {
    #[serde(alias = "solutionId")]
    pub solution_id: i64,
    #[serde(alias = "isLike")]
    pub is_like: bool,
}



#[derive(Deserialize)]
pub struct ComposeDiagnoseBody {
    #[serde(alias = "filePath")]
    pub file_path: String,
}


#[derive(Deserialize)]
pub struct ComposePsQuery {
    #[serde(alias = "projectName")]
    pub project_name: String,
}



// ===== Settings Handlers =====


#[derive(Deserialize)]
pub struct SetSettingRequest {
    pub key: String,
    pub value: String,
}




// ===== Knowledge Bank Handlers =====

#[derive(Deserialize)]
pub struct KbQueryRequest {
    #[serde(alias = "errorText")]
    pub error_text: String,
}




#[derive(Deserialize)]
pub struct KbSearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}





#[derive(Deserialize)]
pub struct UpdateMemoryRequest {
    pub id: String,
    pub content: String,
}




#[derive(Deserialize)]
pub struct DeleteMemoryRequest {
    pub id: String,
}




// ===== Shell Sandbox Handlers =====

#[derive(Deserialize)]
pub struct SandboxRequest {
    pub command: String,
}




pub fn default_install_method() -> String {
    "brew".to_string()
}
pub fn default_profile() -> String {
    "default".to_string()
}
pub fn default_lines() -> u32 {
    200
}
pub fn default_true() -> bool {
    true
}
pub fn default_port() -> u16 {
    11434
}
pub fn default_vm_type() -> String {
    "colima".to_string()
}
pub fn default_cpus() -> u32 {
    2
}
pub fn default_memory() -> u32 {
    2
}
pub fn default_disk() -> u32 {
    60
}
pub fn default_pod_type() -> String {
    "pod".to_string()
}
pub fn default_deployment_type() -> String {
    "deployment".to_string()
}
pub fn default_tail_lines() -> u32 {
    50
}
pub fn default_concurrency() -> u32 {
    5
}
pub fn default_requests() -> u32 {
    50
}
pub fn default_log_lines() -> u32 {
    200
}



pub fn default_limit() -> u32 { 5 }
