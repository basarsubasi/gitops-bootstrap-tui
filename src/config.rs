use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub template_repo_url: String,
    pub base_dir_path: String,
    pub gitops_dir_path: String,
    pub new_cluster_name: String,

    // Post-generation actions defaults
    #[serde(default)]
    pub init_git: bool,
    #[serde(default)]
    pub git_daemon: bool,
    #[serde(default)]
    pub bootstrap_flux: bool,

    // Git Daemon defaults
    #[serde(default)]
    pub git_daemon_address: String,
    #[serde(default)]
    pub git_branch: String,
    #[serde(default)]
    pub git_http_server: bool,
    #[serde(default)]
    pub git_http_server_port: u16,

    // Flux specific defaults
    #[serde(default)]
    pub flux_git_url: String,
    #[serde(default)]
    pub flux_git_branch: String,
    #[serde(default)]
    pub flux_kubeconfig: String,
    #[serde(default)]
    pub flux_ssh_key_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            template_repo_url: "https://github.com/basarsubasi/flux-templates.git".to_string(),
            base_dir_path: "bases".to_string(),
            gitops_dir_path: "~/my-gitops-repo".to_string(),
            new_cluster_name: "my-cluster".to_string(),
            init_git: true,
            git_daemon: false,
            bootstrap_flux: true,
            git_daemon_address: "127.0.0.1".to_string(),
            git_branch: "main".to_string(),
            git_http_server: true,
            git_http_server_port: 8080,
            flux_git_url: "http://127.0.0.1:8080".to_string(),
            flux_git_branch: "main".to_string(),
            flux_kubeconfig: "~/.kube/config".to_string(),
            flux_ssh_key_path: "".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let home_dir = directories::UserDirs::new()
            .ok_or("Could not determine user home directory")?
            .home_dir()
            .to_path_buf();

        let config_dir = home_dir.join(".config").join("gitops-bootstrap-tui");
        let config_file = config_dir.join("config.json");

        if !config_file.exists() {
            // Create default config if it doesn't exist
            fs::create_dir_all(config_dir)?;
            let default_config = AppConfig {
                template_repo_url: "https://github.com/basarsubasi/flux-templates.git".to_string(),
                base_dir_path: "bases".to_string(),
                gitops_dir_path: "~/my-gitops-repo".to_string(),
                new_cluster_name: "my-cluster-1".to_string(),
                ..Default::default()
            };
            let json = serde_json::to_string_pretty(&default_config)?;
            fs::write(&config_file, json)?;
            return Ok(default_config);
        }

        let json = fs::read_to_string(config_file)?;
        let mut config: AppConfig = serde_json::from_str(&json)?;

        // Handle migration from old configs
        if config.gitops_dir_path.is_empty() {
            config.gitops_dir_path = "~/my-gitops-repo".to_string();
        }
        if config.new_cluster_name.is_empty() {
            config.new_cluster_name = "my-cluster".to_string();
        }
        if config.flux_git_url.is_empty() {
            config.flux_git_url = "git://127.0.0.1/".to_string();
        }
        if config.flux_git_branch.is_empty() {
            config.flux_git_branch = "main".to_string();
        }
        if config.flux_kubeconfig.is_empty() {
            config.flux_kubeconfig = "~/.kube/config".to_string();
        }
        if config.git_daemon_address.is_empty() {
            config.git_daemon_address = "127.0.0.1".to_string();
        }
        if config.git_branch.is_empty() {
            config.git_branch = "main".to_string();
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let home_dir = directories::UserDirs::new()
            .ok_or("Could not determine user home directory")?
            .home_dir()
            .to_path_buf();

        let config_dir = home_dir.join(".config").join("gitops-bootstrap-tui");
        let config_file = config_dir.join("config.json");

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&config_file, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "template_repo_url": "https://github.com/fluxcd/flux2.git",
            "base_dir_path": "test_bases",
            "gitops_dir_path": "/tmp/test",
            "new_cluster_name": "c1"
        }"#;

        let config: AppConfig = serde_json::from_str(json).expect("Failed to deserialize JSON");

        assert_eq!(
            config.template_repo_url,
            "https://github.com/fluxcd/flux2.git"
        );
        assert_eq!(config.base_dir_path, "test_bases");
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig {
            template_repo_url: "local/repo".to_string(),
            base_dir_path: "b".to_string(),
            gitops_dir_path: "d".to_string(),
            new_cluster_name: "e".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""template_repo_url":"local/repo""#));
        assert!(json.contains(r#""gitops_dir_path":"d""#));
        assert!(json.contains(r#""new_cluster_name":"e""#));
    }
}
