
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

    let mut layer_resources: HashMap<String, Vec<String>> = HashMap::new();

    // 2. Generate cluster overrides for each checked component
    for rel_path_str in checked_paths {
        let rel_path = Path::new(rel_path_str);
        
        let components: Vec<_> = rel_path.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
        if components.is_empty() { continue; }
        
        let top_layer = components[0].clone();
        let sub_path = components[1..].join("/");
        layer_resources.entry(top_layer).or_default().push(sub_path);

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
    }

    // 3. Manually add repositories layer
    let repo_cluster_dir = target_cluster.join("repositories");
    fs::create_dir_all(&repo_cluster_dir)?;
    fs::write(
        repo_cluster_dir.join("kustomization.yaml"),
        "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - ../../bases/repositories\n"
    )?;

    // 4. Generate Layer kustomization.yamls
    for (layer, resources) in &layer_resources {
        let layer_dir = target_cluster.join(layer);
        let kust_path = layer_dir.join("kustomization.yaml");
        let formatted_resources = resources.iter().map(|r| format!("  - {}", r)).collect::<Vec<_>>().join("\n");
        let content = format!("apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n{}\n", formatted_resources);
        fs::write(&kust_path, content)?;
    }

    // 5. Generate flux-system sync manifests with priorities
    let flux_system_dir = target_cluster.join("flux-system");
    fs::create_dir_all(&flux_system_dir)?;

    let mut sync_resources = Vec::new();

    // Priority 1: repositories
    sync_resources.push("sync-repositories.yaml".to_string());
    fs::write(
        flux_system_dir.join("sync-repositories.yaml"),
        format!("apiVersion: kustomize.toolkit.fluxcd.io/v1\nkind: Kustomization\nmetadata:\n  name: cluster-repositories\n  namespace: flux-system\nspec:\n  interval: 10m\n  path: ./{}/{}/repositories\n  prune: true\n  sourceRef:\n    kind: GitRepository\n    name: flux-system\n", base_dir_path, new_cluster_name).replace("bases/", "clusters/")
    )?;

    // Make sure we define layers properly
    let order = ["infrastructure", "databases", "apps"];
    let mut existing_layers = Vec::new();
    for l in order.iter() {
        if layer_resources.contains_key(*l) {
            existing_layers.push(l.to_string());
        }
    }
    // Add any other dynamic layers the user selected
    for l in layer_resources.keys() {
        if !order.contains(&l.as_str()) {
            existing_layers.push(l.to_string());
        }
    }

    // Priority 2+: ordered layers
    let mut prev_dependency = "cluster-repositories".to_string();
    
    for layer in existing_layers {
        let name = format!("cluster-{}", layer);
        sync_resources.push(format!("sync-{}.yaml", layer));
        
        let content = format!(
            "apiVersion: kustomize.toolkit.fluxcd.io/v1\nkind: Kustomization\nmetadata:\n  name: {}\n  namespace: flux-system\nspec:\n  dependsOn:\n    - name: {}\n  interval: 10m\n  path: ./{}/{}/{}\n  prune: true\n  sourceRef:\n    kind: GitRepository\n    name: flux-system\n",
            name, prev_dependency, "clusters", new_cluster_name, layer
        );
        fs::write(flux_system_dir.join(format!("sync-{}.yaml", layer)), content)?;
        
        // Next layer depends on this one
        prev_dependency = name;
    }

    // Create flux-system/kustomization.yaml
    let flux_kust_content = format!(
        "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n{}\n",
        sync_resources.iter().map(|r| format!("  - {}", r)).collect::<Vec<_>>().join("\n")
    );
    fs::write(flux_system_dir.join("kustomization.yaml"), flux_kust_content)?;

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
    fn test_finalize_generation_layers_and_priorities() {
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
        
        // Verify infrastructure layer kustomization exists and contains cilium
        let infra_kust = fs::read_to_string(cluster_path.join("infrastructure/kustomization.yaml")).unwrap();
        assert!(infra_kust.contains("- networking/cilium"));

        // Verify apps layer kustomization exists
        let apps_kust = fs::read_to_string(cluster_path.join("apps/kustomization.yaml")).unwrap();
        assert!(apps_kust.contains("- my-app"));

        // Verify flux-system sync priority manifests
        let sync_repos = fs::read_to_string(cluster_path.join("flux-system/sync-repositories.yaml")).unwrap();
        assert!(sync_repos.contains("name: cluster-repositories"));

        let sync_infra = fs::read_to_string(cluster_path.join("flux-system/sync-infrastructure.yaml")).unwrap();
        assert!(sync_infra.contains("dependsOn:\n    - name: cluster-repositories"));

        let sync_apps = fs::read_to_string(cluster_path.join("flux-system/sync-apps.yaml")).unwrap();
        assert!(sync_apps.contains("dependsOn:\n    - name: cluster-infrastructure"));

        // Verify root kustomization contains all sync files
        let root_kust = fs::read_to_string(cluster_path.join("flux-system/kustomization.yaml")).unwrap();
        assert!(root_kust.contains("- sync-repositories.yaml"));
        assert!(root_kust.contains("- sync-infrastructure.yaml"));
        assert!(root_kust.contains("- sync-apps.yaml"));
    }
}
