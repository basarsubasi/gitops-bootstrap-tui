use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TreeItem {
    pub name: String,
    pub path: String,
    pub depth: usize,
    pub is_leaf: bool,
    pub is_helm: bool,
    pub children: Vec<TreeItem>,
}

#[derive(Debug, Clone)]
pub struct FlatItem {
    pub name: String,
    pub path: String,
    pub depth: usize,
    pub is_leaf: bool,
    pub is_helm: bool,
}

#[derive(PartialEq)]
pub enum ExplorerFocus {
    Tree,
    Previous,
    Next,
}

pub struct ExplorerState {
    #[allow(dead_code)]
    pub root_path: PathBuf,
    pub tree: Vec<TreeItem>,
    pub expanded_paths: HashSet<String>,
    pub checked_paths: HashSet<String>,
    pub customized_paths: std::collections::HashMap<String, String>,
    pub flat_list: Vec<FlatItem>,
    pub list_state: ListState,
    pub focus: ExplorerFocus,
    pub preview_content: Option<String>,
    pub error_message: Option<String>,
}

impl ExplorerState {
    pub fn new(root_path: PathBuf) -> Self {
        let tree = Self::build_tree(&root_path, &root_path, 0);
        let mut state = Self {
            root_path,
            tree,
            expanded_paths: HashSet::new(),
            checked_paths: HashSet::new(),
            customized_paths: std::collections::HashMap::new(),
            flat_list: Vec::new(),
            list_state: ListState::default(),
            focus: ExplorerFocus::Tree,
            preview_content: None,
            error_message: None,
        };
        state.update_flat_list();
        if !state.flat_list.is_empty() {
            state.list_state.select(Some(0));
        }
        state
    }

    fn build_tree(root: &Path, current: &Path, depth: usize) -> Vec<TreeItem> {
        let mut items = Vec::new();
        if let Ok(entries) = fs::read_dir(current) {
            let mut dirs = Vec::new();
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_dir()
                {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') {
                        dirs.push(entry);
                    }
                }
            }
            dirs.sort_by_key(|a| a.file_name());

            for dir in dirs {
                let path = dir.path();
                let name = dir.file_name().to_string_lossy().to_string();
                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                let mut is_leaf = false;
                let mut is_helm = false;
                if let Ok(dir_entries) = fs::read_dir(&path) {
                    for de in dir_entries.flatten() {
                        let fname = de.file_name();
                        if fname == "helm-release.yaml" {
                            is_leaf = true;
                            is_helm = true;
                        } else if fname == "kustomization.yaml" || fname == "kustomization.yml" {
                            is_leaf = true;
                        }
                    }
                }

                if is_leaf {
                    items.push(TreeItem {
                        name,
                        path: rel_path,
                        depth,
                        is_leaf: true,
                        is_helm,
                        children: Vec::new(),
                    });
                } else {
                    let children = Self::build_tree(root, &path, depth + 1);
                    if !children.is_empty() {
                        items.push(TreeItem {
                            name,
                            path: rel_path,
                            depth,
                            is_leaf: false,
                            is_helm: false,
                            children,
                        });
                    }
                }
            }
        }
        items
    }

    pub fn update_flat_list(&mut self) {
        let mut new_flat = Vec::new();
        for item in &self.tree {
            Self::flatten_to(item, &self.expanded_paths, &mut new_flat);
        }
        self.flat_list = new_flat;

        // Adjust cursor if it's out of bounds
        if let Some(selected) = self.list_state.selected() {
            if self.flat_list.is_empty() {
                self.list_state.select(None);
            } else if selected >= self.flat_list.len() {
                self.list_state.select(Some(self.flat_list.len() - 1));
            }
        }
    }

    fn flatten_to(item: &TreeItem, expanded: &HashSet<String>, out: &mut Vec<FlatItem>) {
        out.push(FlatItem {
            name: item.name.clone(),
            path: item.path.clone(),
            depth: item.depth,
            is_leaf: item.is_leaf,
            is_helm: item.is_helm,
        });

        if expanded.contains(&item.path) {
            for child in &item.children {
                Self::flatten_to(child, expanded, out);
            }
        }
    }

    pub fn handle_up(&mut self) {
        match self.focus {
            ExplorerFocus::Next | ExplorerFocus::Previous => {
                self.focus = ExplorerFocus::Tree;
            }
            ExplorerFocus::Tree => {
                if self.flat_list.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.flat_list.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
        }
    }

    pub fn handle_down(&mut self) {
        if self.focus == ExplorerFocus::Tree {
            if self.flat_list.is_empty() {
                self.focus = ExplorerFocus::Previous;
                return;
            }
            let i = match self.list_state.selected() {
                Some(i) => {
                    if i >= self.flat_list.len() - 1 {
                        self.focus = ExplorerFocus::Previous;
                        i
                    } else {
                        i + 1
                    }
                }
                None => {
                    self.focus = ExplorerFocus::Previous;
                    0
                }
            };
            if self.focus == ExplorerFocus::Tree {
                self.list_state.select(Some(i));
            }
        }
    }

    pub fn handle_left(&mut self) {
        match self.focus {
            ExplorerFocus::Tree => {
                if let Some(idx) = self.list_state.selected()
                    && let Some(item) = self.flat_list.get(idx).cloned()
                    && !item.is_leaf
                    && self.expanded_paths.contains(&item.path)
                {
                    self.expanded_paths.remove(&item.path);
                    self.update_flat_list();
                }
            }
            ExplorerFocus::Next => self.focus = ExplorerFocus::Previous,
            ExplorerFocus::Previous => self.focus = ExplorerFocus::Next,
        }
    }

    pub fn handle_right(&mut self) {
        match self.focus {
            ExplorerFocus::Tree => {
                if let Some(idx) = self.list_state.selected()
                    && let Some(item) = self.flat_list.get(idx).cloned()
                    && !item.is_leaf
                {
                    self.expanded_paths.insert(item.path.clone());
                    self.update_flat_list();
                }
            }
            ExplorerFocus::Previous => self.focus = ExplorerFocus::Next,
            ExplorerFocus::Next => self.focus = ExplorerFocus::Previous,
        }
    }

    pub fn handle_tab(&mut self) {
        match self.focus {
            ExplorerFocus::Tree => {
                if let Some(selected) = self.list_state.selected() {
                    let mut found = false;
                    for i in selected + 1..self.flat_list.len() {
                        if !self.flat_list[i].is_leaf {
                            self.list_state.select(Some(i));
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        self.focus = ExplorerFocus::Next;
                    }
                } else {
                    self.focus = ExplorerFocus::Next;
                }
            }
            ExplorerFocus::Next => self.focus = ExplorerFocus::Previous,
            ExplorerFocus::Previous => self.focus = ExplorerFocus::Tree,
        }
    }

    pub fn handle_backtab(&mut self) {
        match self.focus {
            ExplorerFocus::Tree => {
                if let Some(selected) = self.list_state.selected() {
                    let mut found = false;
                    for i in (0..selected).rev() {
                        if !self.flat_list[i].is_leaf {
                            self.list_state.select(Some(i));
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        self.focus = ExplorerFocus::Previous;
                    }
                } else {
                    self.focus = ExplorerFocus::Previous;
                }
            }
            ExplorerFocus::Next => self.focus = ExplorerFocus::Tree,
            ExplorerFocus::Previous => self.focus = ExplorerFocus::Next,
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.focus != ExplorerFocus::Tree {
            return;
        }
        if let Some(idx) = self.list_state.selected()
            && let Some(item) = self.flat_list.get(idx).cloned()
            && !item.is_leaf
        {
            if self.expanded_paths.contains(&item.path) {
                self.expanded_paths.remove(&item.path);
            } else {
                self.expanded_paths.insert(item.path.clone());
            }
            self.update_flat_list();
        }
    }

    pub fn toggle_current(&mut self) {
        if let Some(idx) = self.list_state.selected()
            && let Some(item) = self.flat_list.get(idx)
            && item.is_leaf
        {
            if self.checked_paths.contains(&item.path) {
                self.checked_paths.remove(&item.path);
            } else {
                self.checked_paths.insert(item.path.clone());
            }
        }
    }

    pub fn undo_current(&mut self) {
        if let Some(idx) = self.list_state.selected()
            && let Some(item) = self.flat_list.get(idx)
            && item.is_leaf
        {
            self.customized_paths.remove(&item.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_explorer_scan() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let apps_dir = root.join("apps");
        let redis_dir = apps_dir.join("redis");
        fs::create_dir_all(&redis_dir).unwrap();
        fs::write(redis_dir.join("kustomization.yaml"), "").unwrap();

        let mut state = ExplorerState::new(root.to_path_buf());
        assert_eq!(state.flat_list.len(), 1); // "apps" should be visible
        assert_eq!(state.flat_list[0].name, "apps");
        assert!(!state.flat_list[0].is_leaf);

        // Expand "apps"
        state.expanded_paths.insert("apps".to_string());
        state.update_flat_list();

        assert_eq!(state.flat_list.len(), 2);
        assert_eq!(state.flat_list[1].name, "redis");
        assert!(state.flat_list[1].is_leaf);
    }
}
