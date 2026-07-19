use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct GitManager {
    pub repo_url: String,
    #[allow(dead_code)]
    pub cache_dir: PathBuf,
    pub repo_dir: PathBuf,
}

#[allow(dead_code)]
impl GitManager {
    pub fn new(repo_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let proj_dirs = ProjectDirs::from("", "", "gitops-bootstrap-tui")
            .ok_or("Could not determine config directory")?;

        let cache_dir = proj_dirs.cache_dir().to_path_buf();
        let repo_name = repo_url
            .split('/')
            .next_back()
            .unwrap_or("templates")
            .trim_end_matches(".git");
        let repo_dir = cache_dir.join(repo_name);

        Ok(Self {
            repo_url: repo_url.to_string(),
            cache_dir,
            repo_dir,
        })
    }

    pub fn sync(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.repo_dir.exists() {
            fs::create_dir_all(&self.repo_dir)?;
        }

        let is_empty = fs::read_dir(&self.repo_dir)?.next().is_none();

        // Ensure the directory has a .git folder or is empty
        let git_dir = self.repo_dir.join(".git");

        if !is_empty && git_dir.exists() {
            // Already cloned, pull latest
            let status = Command::new("git")
                .arg("-C")
                .arg(&self.repo_dir)
                .arg("pull")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;

            if !status.success() {
                return Err("Failed to pull latest changes from git repository".into());
            }
        } else {
            // Clone
            let status = Command::new("git")
                .arg("clone")
                .arg(&self.repo_url)
                .arg(&self.repo_dir)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;

            if !status.success() {
                return Err("Failed to clone git repository".into());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_manager_creation() {
        let mgr = GitManager::new("https://example.com/repo.git").unwrap();
        assert_eq!(mgr.repo_url, "https://example.com/repo.git");
        // Ensure cache directory has the expected suffix
        assert!(
            mgr.cache_dir
                .to_string_lossy()
                .contains("gitops-bootstrap-tui")
        );
        assert!(mgr.repo_dir.ends_with("repo"));
    }
}
