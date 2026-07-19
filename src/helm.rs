use serde_yaml::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn fetch_helm_values(component_dir: &Path) -> Result<String, String> {
    let repo_path = component_dir.join("helm-repo.yaml");
    let release_path = component_dir.join("helm-release.yaml");

    if !repo_path.exists() || !release_path.exists() {
        return Err("Missing helm-repo.yaml or helm-release.yaml".to_string());
    }

    let repo_content = fs::read_to_string(&repo_path).map_err(|e| e.to_string())?;
    let repo_yaml: Value = serde_yaml::from_str(&repo_content)
        .map_err(|e| format!("Failed to parse helm-repo.yaml: {}", e))?;

    let url = repo_yaml
        .get("spec")
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .ok_or("Could not find spec.url in helm-repo.yaml")?;

    let release_content = fs::read_to_string(&release_path).map_err(|e| e.to_string())?;
    let release_yaml: Value = serde_yaml::from_str(&release_content)
        .map_err(|e| format!("Failed to parse helm-release.yaml: {}", e))?;

    let chart_spec = release_yaml
        .get("spec")
        .and_then(|s| s.get("chart"))
        .and_then(|c| c.get("spec"))
        .ok_or("Could not find spec.chart.spec in helm-release.yaml")?;

    let chart_name = chart_spec
        .get("chart")
        .and_then(|c| c.as_str())
        .ok_or("Could not find chart name")?;

    let mut cmd = Command::new("helm");
    cmd.arg("show")
        .arg("values")
        .arg(chart_name)
        .arg("--repo")
        .arg(url);

    if let Some(version) = chart_spec.get("version").and_then(|v| v.as_str()) {
        cmd.arg("--version").arg(version);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute helm: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("helm show values failed: {}", err_msg));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn diff_yaml(base: &Value, new: &Value) -> Option<Value> {
    match (base, new) {
        (Value::Mapping(b), Value::Mapping(n)) => {
            let mut diff = serde_yaml::Mapping::new();
            for (k, v) in n {
                if let Some(bv) = b.get(k) {
                    if let Some(d) = diff_yaml(bv, v) {
                        diff.insert(k.clone(), d);
                    }
                } else {
                    diff.insert(k.clone(), v.clone());
                }
            }
            if diff.is_empty() {
                None
            } else {
                Some(Value::Mapping(diff))
            }
        }
        (b, n) => {
            if b == n {
                None
            } else {
                Some(n.clone())
            }
        }
    }
}

pub fn edit_yaml(initial_content: &str) -> Result<Option<String>, String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let temp_file = tempfile::Builder::new()
        .prefix("gitops-bootstrap-tui-values-")
        .suffix(".yaml")
        .tempfile()
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let temp_path = temp_file.path().to_path_buf();
    fs::write(&temp_path, initial_content).map_err(|e| e.to_string())?;

    loop {
        let status = Command::new(&editor)
            .arg(&temp_path)
            .status()
            .map_err(|e| format!("Failed to start editor: {}", e))?;

        if !status.success() {
            return Err("Editor exited with an error".to_string());
        }

        let edited_content = fs::read_to_string(&temp_path).map_err(|e| e.to_string())?;

        if edited_content.trim().is_empty() {
            return Err("Cancelled by user".to_string());
        }

        // Validate YAML
        match serde_yaml::from_str::<Value>(&edited_content) {
            Ok(edited_val) => {
                let base_val = if initial_content.trim().is_empty() {
                    Value::Mapping(serde_yaml::Mapping::new())
                } else {
                    serde_yaml::from_str::<Value>(initial_content)
                        .unwrap_or(Value::Mapping(serde_yaml::Mapping::new()))
                };

                if let Some(diff) = diff_yaml(&base_val, &edited_val) {
                    let diff_str = serde_yaml::to_string(&diff).map_err(|e| e.to_string())?;
                    return Ok(Some(diff_str));
                } else {
                    // No changes
                    return Ok(None);
                }
            }
            Err(e) => {
                let err_msg = format!(
                    "\n# ERROR: Invalid YAML: {}\n# Please fix the errors and save, or clear the file to cancel.\n",
                    e
                );
                let new_content = format!("{}{}", err_msg, edited_content);
                fs::write(&temp_path, new_content).map_err(|e| e.to_string())?;
            }
        }
    }
}
