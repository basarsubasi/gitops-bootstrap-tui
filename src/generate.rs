
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn finalize_generation(
    cache_root: &Path,
    gitops_dir_path: &str,
    base_dir_path: &str,
    new_cluster_name: &str,
    checked_paths: &std::collections::HashSet<String>,
    customized_paths: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let expanded_gitops_path = if let Some(stripped) = gitops_dir_path.strip_prefix("~/") {
        if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
            home.join(stripped)
        } else {
            PathBuf::from(gitops_dir_path)
        }
    } else {
        PathBuf::from(gitops_dir_path)
    };

    let target_root = expanded_gitops_path;
    let target_bases = target_root.join(base_dir_path);
    let target_cluster = target_root.join(new_cluster_name);

    fs::create_dir_all(&target_root)?;
    fs::create_dir_all(&target_cluster)?;

    // 1. Copy the entire base directory from cache to target
    let source_bases = cache_root.join(base_dir_path);
    if source_bases.exists() {
        copy_dir_recursive(&source_bases, &target_bases)?;
    }

    let mut root_kustomization_resources = Vec::new();

    // 2. Generate cluster overrides for each checked component
    for rel_path_str in checked_paths {
        let rel_path = Path::new(rel_path_str);
        
        let cluster_component_dir = target_cluster.join(rel_path);
        fs::create_dir_all(&cluster_component_dir)?;

        let depth = rel_path.components().count();
        let mut up_path = String::new();
        for _ in 0..(depth + 1) {
            up_path.push_str("../");
        }
        let bases_ref = format!("{}{}/{}", up_path, base_dir_path, rel_path_str);

        let mut kustomization_content = format!(
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - {}\n",
            bases_ref
        );

        if let Some(patch_content) = customized_paths.get(rel_path_str) {
            let patch_file_path = cluster_component_dir.join("patch.yaml");
            let patch_yaml = format!(
                "apiVersion: helm.toolkit.fluxcd.io/v2\nkind: HelmRelease\nmetadata:\n  name: {}\nspec:\n  values:\n{}",
                rel_path.file_name().unwrap_or_default().to_string_lossy(),
                indent_content(patch_content, 4)
            );
            fs::write(&patch_file_path, patch_yaml)?;
            kustomization_content.push_str("patches:\n  - path: patch.yaml\n");
        }

        let kust_file_path = cluster_component_dir.join("kustomization.yaml");
        fs::write(&kust_file_path, kustomization_content)?;

        // Add to root kustomization
        root_kustomization_resources.push(format!("- {}", rel_path_str));
    }

    // Extract unique namespaces and strip local namespace.yaml from target bases
    let mut unique_namespaces = std::collections::HashSet::new();
    for rel_path_str in checked_paths {
        let base_comp_dir = target_bases.join(rel_path_str);
        
        // Read helm-release.yaml to extract namespace
        let hr_path = base_comp_dir.join("helm-release.yaml");
        if hr_path.exists()
            && let Ok(content) = fs::read_to_string(&hr_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("namespace:") {
                        let ns = trimmed.strip_prefix("namespace:").unwrap().trim();
                        // Remove surrounding quotes if any
                        let ns = ns.trim_matches(|c| c == '"' || c == '\'');
                        if ns != "flux-system" && ns != "default" {
                            unique_namespaces.insert(ns.to_string());
                        }
                        break;
                    }
                }
            }
        
        // Remove namespace.yaml from base's kustomization.yaml to prevent ID collisions
        let kust_path = base_comp_dir.join("kustomization.yaml");
        if kust_path.exists()
            && let Ok(content) = fs::read_to_string(&kust_path) {
                let new_content: Vec<&str> = content.lines()
                    .filter(|l| !l.contains("- namespace.yaml"))
                    .collect();
                let _ = fs::write(&kust_path, new_content.join("\n") + "\n");
            }
    }

    // Generate cluster-level namespaces explicitly
    if !unique_namespaces.is_empty() {
        let ns_cluster_dir = target_cluster.join("namespaces");
        fs::create_dir_all(&ns_cluster_dir)?;
        
        let mut ns_resources = Vec::new();
        let mut sorted_ns: Vec<_> = unique_namespaces.into_iter().collect();
        sorted_ns.sort();
        
        for ns in &sorted_ns {
            let ns_file = format!("{}.yaml", ns);
            let ns_path = ns_cluster_dir.join(&ns_file);
            fs::write(ns_path, format!("apiVersion: v1\nkind: Namespace\nmetadata:\n  name: {}\n", ns))?;
            ns_resources.push(format!("  - {}", ns_file));
        }
        
        fs::write(
            ns_cluster_dir.join("kustomization.yaml"),
            format!("apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n{}\n", ns_resources.join("\n"))
        )?;
        
        // Insert namespaces at the top of root resources
        root_kustomization_resources.insert(0, "- namespaces".to_string());
    }

    // 3. Generate repositories layer explicitly
    let repo_cluster_dir = target_cluster.join("repositories");
    fs::create_dir_all(&repo_cluster_dir)?;
    fs::write(
        repo_cluster_dir.join("kustomization.yaml"),
        "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - ../../bases/repositories\n"
    )?;
    root_kustomization_resources.push("- repositories".to_string());

    // 4. Generate root kustomization.yaml
    if !root_kustomization_resources.is_empty() {
        let root_kust_path = target_cluster.join("kustomization.yaml");
        let root_kust_content = format!(
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n{}\n",
            root_kustomization_resources.join("\n")
        );
        fs::write(&root_kust_path, root_kust_content)?;
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn indent_content(content: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    content
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_finalize_generation_monolithic() {
        let cache_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Setup mock bases
        let bases_path = cache_dir.path().join("bases");
        fs::create_dir_all(bases_path.join("infrastructure/networking/cilium")).unwrap();
        fs::create_dir_all(bases_path.join("apps/my-app")).unwrap();

        let mut checked_paths = HashSet::new();
        checked_paths.insert("infrastructure/networking/cilium".to_string());
        checked_paths.insert("apps/my-app".to_string());

        let customized_paths = HashMap::new();

        let res = finalize_generation(
            cache_dir.path(),
            target_dir.path().to_str().unwrap(),
            "bases",
            "test-cluster",
            &checked_paths,
            &customized_paths,
        );
        assert!(res.is_ok());

        let cluster_path = target_dir.path().join("test-cluster");
        
        // Verify root kustomization contains all paths including repositories
        let root_kust = fs::read_to_string(cluster_path.join("kustomization.yaml")).unwrap();
        assert!(root_kust.contains("- infrastructure/networking/cilium"));
        assert!(root_kust.contains("- apps/my-app"));
        assert!(root_kust.contains("- repositories"));
    }
}
