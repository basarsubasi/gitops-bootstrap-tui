# GitOps Bootstrap TUI

An interactive Terminal User Interface (TUI) tool written in Rust to seamlessly bootstrap and manage local GitOps repositories and Flux CD deployments, optimal for ephemeral k8s clusters and stack experimentation.


## Default Templates

Default template and the expected directory structure can be found at the [https://github.com/basarsubasi/flux-templates](https://github.com/basarsubasi/flux-templates) repository.

You can also use your own templates, by creating a similar structure in a directory and providing the path to the `gitops-bootstrap-tui`.

## Features

- **Interactive Wizard**: A guided wizard to gather essential bootstrap configuration (Base Directory, Cluster Name, Git URL, etc.).
- **Component Explorer**: A navigable tree view to explore, enable, and disable Helm releases and GitOps components before generation.
- **Value Customization**: You can customize the default helm chart values before generating the templates.
- **Post-Generation Actions**:
  - Automatically initialize a local Git repository and commit the bootstrapped structure.
  - Spawn a local `git daemon` to serve your repository securely.
  - Bootstrap Flux CD directly to your Kubernetes cluster seamlessly from the UI.
- **Persistent Configuration**: User configurations and inputs are saved automatically to `~/.config/gitops-bootstrap-tui/config.json` and restored on the next run.

## Prerequisites

- `git` CLI installed and available in `$PATH`
- `flux` CLI installed and available in `$PATH`
- A valid Kubernetes cluster (e.g., Kind, Minikube, K3s) and `kubeconfig` for Flux bootstrap

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

## Usage

Start the TUI by running:

```bash
gitops-bootstrap-tui
```

