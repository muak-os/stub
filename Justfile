# Muak UEFI boot stub
#
# Prerequisites: rustup (nightly toolchain), docker/podman
# Run `just --list` for available recipes

set positional-arguments := true
set shell := ["bash", "-euo", "pipefail", "-c"]
set script-interpreter := ["bash", "-euo", "pipefail"]

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

alpine_version := "3.24"
rust_version := "1.98"
registry := env_var_or_default("REGISTRY", "ghcr.io/muak-os")
tag := env_var_or_default("TAG", "latest")
push := env_var_or_default("PUSH", "false")
latest := env_var_or_default("LATEST", "false")
out := `test -f .git && realpath -m "$(git rev-parse --git-common-dir)/../_out" || realpath -m _out`

# Architecture

[private]
_arch := env_var_or_default("ARCH", "")
arch := if _arch == "amd64" { "x86_64" } else if _arch == "arm64" { "aarch64" } else if _arch != "" { _arch } else { "x86_64" }
oci_suffix := if _arch != "" { "-" + _arch } else { "" }
oci_arch := if _arch == "arm64" { "arm64" } else { "amd64" }

# Container runtime

container_runtime := env_var_or_default("CONTAINER_RUNTIME", "podman")

# Colors

bold := '\e[1m'
cyan := '\e[36m'
green := '\e[32m'
red := '\e[31m'
reset := '\e[0m'

# ─────────────────────────────────────────────────────────────────────────────
# Main Recipes
# ─────────────────────────────────────────────────────────────────────────────

# Build the UEFI stub (e.g., just build, just build --release)
[arg("release", long="release", value="--release")]
[script]
build release="":
    printf "{{ cyan }}Building UEFI stub ({{ arch }}-unknown-uefi){{ reset }}\n"
    cargo build {{ release }} --target {{ arch }}-unknown-uefi --features uefi
    printf "{{ green }}Stub built successfully!{{ reset }}\n"

# Build (and optionally push) the stub OCI image
[script]
oci:
    image="{{ registry }}/stub:{{ tag }}{{ oci_suffix }}"
    tags="--tag ${image}"
    if [ "{{ latest }}" = "true" ]; then
        tags="${tags} --tag {{ registry }}/stub:latest{{ oci_suffix }}"
    fi

    if [ "{{ container_runtime }}" = "podman" ]; then
        cmd="podman build"
        push_flags=""
    else
        cmd="docker buildx build --provenance=false"
        if [ "{{ push }}" = "true" ]; then
            push_flags="--push"
        else
            push_flags=""
        fi
    fi

    printf "{{ cyan }}Building stub image: {{ registry }}/stub (push={{ push }}, latest={{ latest }}){{ reset }}\n"
    ${cmd} \
        --platform=linux/{{ oci_arch }} \
        --progress=auto \
        --build-arg ALPINE_VERSION={{ alpine_version }} \
        --build-arg RUST_VERSION={{ rust_version }} \
        --build-arg SOURCE_DATE_EPOCH=0 \
        ${push_flags} \
        $(just _cache-from stub) $(just _cache-to stub) \
        ${tags} \
        --file Dockerfile \
        .

    if [ "{{ container_runtime }}" = "podman" ] && [ "{{ push }}" = "true" ]; then
        {{ container_runtime }} push "${image}"
        if [ "{{ latest }}" = "true" ]; then {{ container_runtime }} push "{{ registry }}/stub:latest{{ oci_suffix }}"; fi
    fi

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
