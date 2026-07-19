# GitOps Bootstrap TUI

> [!CAUTION]
> **THIS PROJECT IS MEANT TO BE USED ON EPHEMERAL CLUSTERS FOR EXPERIMENTATION. DO NOT USE IN PRODUCTION.**

  - **DANGER:** The pipeline uses `git push -f` (force push) when pushing generated manifests to your remote branch. This ensures the repo precisely mirrors your local generation but *will overwrite remote history*. 

An interactive Terminal User Interface (TUI) tool written in Rust to seamlessly bootstrap and manage local GitOps repositories and Flux CD deployments, optimal for ephemeral k8s clusters and stack experimentation.


## Default Templates

Default template and the expected directory structure can be found at the [https://github.com/basarsubasi/flux-templates](https://github.com/basarsubasi/flux-templates) repository.

You can also use your own templates, by creating a similar structure in a directory and providing the path to the `gitops-bootstrap-tui`.

### Generated GitOps Output Structure
When you generate a cluster using the TUI, it creates the following standard Kustomize structure:

```text
.
├── bases/                             # Copied from templates
└── <cluster-name>/                    # Your generated GitOps directory
    ├── kustomization.yaml             # Main kustomization combining all components
    ├── repositories/                  # Centralized HelmRepositories
    │   └── kustomization.yaml
    ├── infrastructure/
    │   └── networking/
    │       └── cilium/
    │           ├── kustomization.yaml # Points to bases
    │           └── patch.yaml         # Your custom Helm values
    └── apps/
        └── ...
```

## Features

- **Interactive Wizard**: A guided wizard to gather essential bootstrap configuration (Base Directory, Cluster Name, Git URL, etc.).
- **Component Explorer**: A navigable tree view to explore, enable, and disable Helm releases and GitOps components before generation.
- **Value Customization**: You can customize the default helm chart values before generating the templates.
- **Deduplicated GitOps Generation**: Dynamically constructs isolated Kustomize trees and automatically prevents duplicate HelmRepository declarations.
- **Post-Generation Actions**:
  - Automatically initialize a local Git repository, natively pull existing commits (resolving conflicts by preferring remote versions), and commit the bootstrapped structure.
  - Seamlessly push generated manifests to any standard remote Git provider (GitHub, GitLab, Gitea, Bitbucket) using SSH keys or HTTP tokens.
  - Bootstrap Flux CD securely using `flux bootstrap git` for provider-agnostic compatibility.
- **Live Interactive Execution Engine**: Stream complex CLI executions (like `flux bootstrap`) right into the TUI. We natively strip ANSI escape codes and parse interactive `[y/N]` prompts so you can type answers without breaking the UI context!
- **Persistent Configuration**: User configurations and inputs are saved automatically to `~/.config/gitops-bootstrap-tui/config.json` and restored on the next run.

## Prerequisites

- `git` CLI installed and available in `$PATH`
- `flux` CLI installed and available in `$PATH`
- `kubectl` CLI installed and available in `$PATH`
- An external Git repository (e.g., GitHub, GitLab, Gitea) configured and reachable.
- A valid Kubernetes cluster (e.g., Kind, Minikube, K3s) and `kubeconfig` for Flux bootstrap.

## Installation

Clone the repository and build the project using Cargo:

```bash
git clone https://github.com/basarsubasi/gitops-tui.git
cd gitops-tui
cargo build --release
```

You can run the binary directly from the `target/release` directory or install it to your Cargo bin path:

```bash
cargo install --path .
```

## Prior Setup & Configuration

Before running the TUI, you must prepare your environment so that Git and Flux can securely communicate with your remote Git provider (GitLab, GitHub, etc.):

**1. Generate an SSH Key (If you don't have one)**
```bash
ssh-keygen -t ed25519 -C "your_email@example.com"
```
*(Save it to the default location `~/.ssh/id_ed25519` without a passphrase for automated Flux access).*

**2. Add the Public Key to your Git Provider**
Copy the contents of your public key (`cat ~/.ssh/id_ed25519.pub`) and add it to your GitLab/GitHub account as a new SSH key. This gives you push access.

**3. Create an Empty Repository**
Go to your Git provider's web interface and create a new, blank repository (e.g., `cicd/flux/testing-tool`). Do not initialize it with a README.

**4. Test Your SSH Connection**
Ensure your machine can authenticate without interactive prompts:
```bash
# For GitLab
ssh -T git@gitlab.com
# For GitHub
ssh -T git@github.com
```

**5. Prepare Your Kubernetes Cluster**
Ensure you have an active Kubernetes context (`kubectl config current-context`) pointing to the cluster where you want to bootstrap Flux.
