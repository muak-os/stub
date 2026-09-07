# Muak UEFI boot stub
#
# Prerequisites: rustup, docker/podman, git
# Run `just --list` for available recipes

set positional-arguments := true
set shell := ["bash", "-euo", "pipefail", "-c"]
set script-interpreter := ["bash", "-euo", "pipefail"]

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

# Global settings

alpine_version := "3.24"
rust_version := `grep -oP 'rust-version\s*=\s*"\K[^"]+' Cargo.toml`
out := `test -f .git && realpath -m "$(git rev-parse --git-common-dir)/../_out" || realpath -m _out`
registry := env_var_or_default("REGISTRY", "ghcr.io/muak-os")
tag := env_var_or_default("TAG", "latest")
tools := env_var_or_default("TOOLS", "ghcr.io/muak-os/tools:latest")
push := env_var_or_default("PUSH", "false")
latest := env_var_or_default("LATEST", "false")

# Architecture

[private]
_arch := env_var_or_default("ARCH", "")
arch := if _arch == "amd64" { "x86_64" } else if _arch == "arm64" { "aarch64" } else if _arch != "" { _arch } else { "x86_64" }
oci_arch := if _arch == "arm64" { "arm64" } else { "amd64" }

# Container runtime

container_runtime := env_var_or_default("CONTAINER_RUNTIME", "podman")
push_arg := if container_runtime == "podman" { "" } else { if push == "true" { "--push" } else { "" } }

# Colors

bold := '\e[1m'
cyan := '\e[36m'
green := '\e[32m'
red := '\e[31m'
reset := '\e[0m'

# ─────────────────────────────────────────────────────────────────────────────
# Main Recipes
# ─────────────────────────────────────────────────────────────────────────────

# Full local development build (build → oci → annotate)
dev: (build "--release") oci annotate

# Build the UEFI stub (e.g., just build, just build --release)
[arg("release", long="release", value="--release")]
[script]
build release="":
    printf "{{ cyan }}Building UEFI stub ({{ arch }}-unknown-uefi){{ reset }}\n"
    CARGO_BUILD_SBOM=true cargo build {{ release }} -Z sbom --target {{ arch }}-unknown-uefi --features uefi
    printf "{{ green }}Stub built successfully!{{ reset }}\n"

# ─────────────────────────────────────────────────────────────────────────────
# OCI Images
# ─────────────────────────────────────────────────────────────────────────────

# Build (and optionally push) the stub OCI image
[script]
oci:
    image="{{ registry }}/stub:{{ tag }}"
    tags="--tag ${image}"
    if [ "{{ latest }}" = "true" ]; then
        tags="${tags} --tag {{ registry }}/stub:latest"
    fi

    if [ "{{ container_runtime }}" = "podman" ]; then
        cmd="podman build"
    else
        cmd="docker buildx build --provenance=false"
    fi

    printf "{{ cyan }}Building stub image: {{ registry }}/stub (push={{ push }}, latest={{ latest }}){{ reset }}\n"
    ${cmd} \
        --platform=linux/{{ oci_arch }} \
        --progress=auto \
        --build-arg ALPINE_VERSION={{ alpine_version }} \
        --build-arg RUST_VERSION={{ rust_version }} \
        --build-arg SOURCE_DATE_EPOCH=0 \
        {{ push_arg }} \
        $(just _cache-from stub) $(just _cache-to stub) \
        ${tags} \
        --file Dockerfile \
        .

    if [ "{{ container_runtime }}" = "podman" ] && [ "{{ push }}" = "true" ]; then
        {{ container_runtime }} push "${image}"
        if [ "{{ latest }}" = "true" ]; then {{ container_runtime }} push "{{ registry }}/stub:latest"; fi
    fi

# Merge per-platform images into a multi-arch OCI index
[script]
merge *sources:
    tags=""
    if [ "{{ latest }}" = "true" ]; then
        tags="--tag latest"
    fi
    {{ container_runtime }} run --rm --network=host \
        -e KOCI_REGISTRY_USERNAME -e KOCI_REGISTRY_PASSWORD \
        {{ tools }} \
        /koci merge \
            --image "{{ registry }}/stub" \
            --tag "{{ tag }}" \
            ${tags} \
            {{ sources }}

# Annotate an OCI image in the registry with per-entry sizes.
[arg("image", long="image")]
annotate image=(registry + "/stub:" + tag):
    @printf "{{ cyan }}Annotating OCI image {{ image }}{{ reset }}\n"
    {{ container_runtime }} run --rm --network=host \
        -e KOCI_REGISTRY_USERNAME -e KOCI_REGISTRY_PASSWORD \
        {{ tools }} \
        /koci annotate \
            --image "{{ image }}" \
            --annotation dev.muak.sizes

# ─────────────────────────────────────────────────────────────────────────────
# Testing
# ─────────────────────────────────────────────────────────────────────────────

# Run formatting
format:
    @printf "{{ cyan }}Running formatting{{ reset }}\n"
    cargo fmt

# Run clippy and rustfmt
[script]
lint: format
    printf "{{ cyan }}Running lints{{ reset }}\n"
    cargo clippy --all-targets --target {{ arch }}-unknown-uefi --features uefi
    cargo clippy --all-targets --target {{ arch }}-unknown-uefi --no-default-features --features uefi

# Run tests
[script]
test:
    printf "{{ cyan }}Running tests{{ reset }}\n"
    cargo nextest run
    cargo nextest run --no-default-features

# Run tests with coverage (e.g., just coverage, just coverage --missing)
[arg("missing", long="missing", value="--show-missing-lines")]
[script]
coverage missing="":
    cargo +stable llvm-cov clean --workspace
    printf "{{ cyan }}Running tests with coverage{{ reset }}\n"
    cargo +stable llvm-cov nextest {{ missing }}

# ─────────────────────────────────────────────────────────────────────────────
# Utilities
# ─────────────────────────────────────────────────────────────────────────────

# Remove all build artifacts
clean:
    @printf "{{ cyan }}Cleaning build artifacts{{ reset }}\n"
    cargo clean
    rm -rf {{ out }}
    @printf "{{ green }}Clean complete{{ reset }}\n"

# ─────────────────────────────────────────────────────────────────────────────
# Private Helpers
# ─────────────────────────────────────────────────────────────────────────────

[private]
_cache-from name:
    @if [ "{{ env_var_or_default("GITHUB_ACTIONS", "false") }}" = "true" ]; then printf '%s' "--cache-from=type=registry,ref={{ registry }}/{{ name }}:buildcache-{{ oci_arch }}"; fi

[private]
_cache-to name:
    @if [ "{{ env_var_or_default("GITHUB_ACTIONS", "false") }}" = "true" ] && [ "{{ push }}" = "true" ]; then printf '%s' "--cache-to=type=registry,ref={{ registry }}/{{ name }}:buildcache-{{ oci_arch }},mode=max"; fi
