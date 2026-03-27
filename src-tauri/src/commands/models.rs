use serde::{Deserialize, Serialize};
use std::process::Command;

/// AI Model info from `colima model list`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModel {
    pub name: String,
    pub size: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub parameters: String,
    #[serde(default)]
    pub quantization: String,
}

/// List available AI models
#[tauri::command]
pub async fn list_models(profile: String, runner: Option<String>) -> Result<Vec<AiModel>, String> {
    let mut args = vec!["model", "list"];
    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }
    let runner_val;
    if let Some(ref r) = runner {
        if !r.is_empty() {
            runner_val = r.clone();
            args.push("--runner");
            args.push(&runner_val);
        }
    }

    let output = Command::new("colima")
        .args(&args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "'colima' is not installed. Install with: brew install colima".to_string()
            } else {
                format!("Failed to list models: {}", e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not supported") || stderr.contains("not available") {
            return Ok(vec![]);
        }
        if stderr.contains("krunkit") || stderr.contains("vm-type") {
            return Err("GPU support requires krunkit. Install with: brew tap slp/krunkit && brew install krunkit\nThen restart: colima start --runtime docker --vm-type krunkit".to_string());
        }
        return Err(format!("colima model list failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Try JSON parse first, fallback to text parsing
    if let Ok(models) = serde_json::from_str::<Vec<AiModel>>(&stdout) {
        return Ok(models);
    }

    // Parse text table output
    let models: Vec<AiModel> = stdout
        .lines()
        .skip(1) // Skip header
        .filter(|l| !l.trim().is_empty() && !l.starts_with("---"))
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            AiModel {
                name: parts.first().unwrap_or(&"unknown").to_string(),
                size: parts.get(1).unwrap_or(&"").to_string(),
                format: parts.get(2).unwrap_or(&"").to_string(),
                family: parts.get(3).unwrap_or(&"").to_string(),
                parameters: parts.get(4).unwrap_or(&"").to_string(),
                quantization: parts.get(5).unwrap_or(&"").to_string(),
            }
        })
        .collect();

    Ok(models)
}

/// Pull/download a model
#[tauri::command]
pub async fn pull_model(profile: String, model_name: String, runner: Option<String>) -> Result<String, String> {
    let mut args = vec!["model", "pull", model_name.as_str()];
    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }
    let runner_val;
    if let Some(ref r) = runner {
        if !r.is_empty() {
            runner_val = r.clone();
            args.push("--runner");
            args.push(&runner_val);
        }
    }

    let output = Command::new("colima")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to pull model: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "model pull failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Model '{}' pulled successfully", model_name))
}

/// Serve a model
#[tauri::command]
pub async fn serve_model(profile: String, model_name: String, port: u16, runner: Option<String>) -> Result<String, String> {
    let port_str = port.to_string();
    let mut args = vec!["model", "serve", model_name.as_str(), "--port", &port_str];
    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }
    let runner_val;
    if let Some(ref r) = runner {
        if !r.is_empty() {
            runner_val = r.clone();
            args.push("--runner");
            args.push(&runner_val);
        }
    }

    let output = Command::new("colima")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to serve model: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "model serve failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Model '{}' serving on port {}", model_name, port))
}

/// Delete a model
#[tauri::command]
pub async fn delete_model(profile: String, model_name: String, runner: Option<String>) -> Result<String, String> {
    let mut args = vec!["model", "delete", model_name.as_str()];
    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }
    let runner_val;
    if let Some(ref r) = runner {
        if !r.is_empty() {
            runner_val = r.clone();
            args.push("--runner");
            args.push(&runner_val);
        }
    }

    let output = Command::new("colima")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to delete model: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "model delete failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Model '{}' deleted", model_name))
}
