use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn finalize_generation(
    cache_root: &Path,     // ~/.cache/gitops-tui (where the git repo is)
    gitops_dir_path: &str, // e.g. /home/user/gitops-repo
    base_dir_path: &str,   // e.g. "bases"
    new_cluster_name: &str,
    checked_paths: &std::collections::HashSet<String>, // relative paths e.g. "infrastructure/networking/cilium"
    customized_paths: &HashMap<String, String>,        // relative path -> patch.yaml content
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

    // 2. Generate cluster overrides for each checked component
    let mut root_kustomization_resources = Vec::new();

    for rel_path_str in checked_paths {
        let rel_path = Path::new(rel_path_str);

        let cluster_component_dir = target_cluster.join(rel_path);
        fs::create_dir_all(&cluster_component_dir)?;

        // Determine the relative path back to bases.
        // If rel_path is "infrastructure/networking/cilium" (3 levels deep),
        // cluster_component_dir is my-cluster/infrastructure/networking/cilium.
        // We need to go up 3 (from cilium to my-cluster) + 1 (from my-cluster to root) = 4 levels up.
        let depth = rel_path.components().count();
        let mut up_path = String::new();
        // The total up path should be `depth` + 1 (for cluster_name)
        for _ in 0..(depth + 1) {
            up_path.push_str("../");
        }
        let bases_ref = format!("{}{}/{}", up_path, base_dir_path, rel_path_str);

        // Generate the component kustomization.yaml
        let mut kustomization_content = format!(
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - {}\n",
            bases_ref
        );

        // Inject patches if any
        if let Some(patch_content) = customized_paths.get(rel_path_str) {
            let patch_file_path = cluster_component_dir.join("patch.yaml");

            // Format the strategic merge patch targeted at the HelmRelease
            let patch_yaml = format!(
                "apiVersion: helm.toolkit.fluxcd.io/v2beta1\nkind: HelmRelease\nmetadata:\n  name: {}\nspec:\n  values:\n{}",
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

    // 3. Generate Root kustomization.yaml
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
            // skip .git if it accidentally got inside bases
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
