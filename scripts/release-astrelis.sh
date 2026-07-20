#!/usr/bin/env bash
set -euo pipefail

version="0.3.0-rc.1"
mode="${1:-package}"

layers=(
  "astrelis-core astrelis-profiling"
  "astrelis-gpu astrelis-platform astrelis-text"
  "astrelis-app astrelis-gpu-wgpu astrelis-platform-test astrelis-platform-winit astrelis-paint astrelis-render astrelis-text-gpu"
  "astrelis-paint-gpu astrelis-render-2d astrelis-render-3d astrelis-ui-core"
  "astrelis-compositor astrelis-ui astrelis-ui-testing astrelis-ui-widgets"
  "astrelis-ui-docking astrelis-ui-host"
  "astrelis"
)

usage() {
  echo "usage: $0 [package|status|publish]" >&2
  exit 2
}

visible() {
  cargo info "$1@$version" >/dev/null 2>&1
}

wait_until_visible() {
  local package="$1"
  local attempt
  for attempt in {1..40}; do
    if visible "$package"; then
      echo "$package@$version is visible"
      return 0
    fi
    echo "waiting for $package@$version to reach the registry index ($attempt/40)"
    sleep 15
  done
  echo "$package@$version was uploaded but is not visible yet; rerun later" >&2
  return 1
}

case "$mode" in
  package)
    cargo package --workspace --allow-dirty --no-verify
    ;;
  status)
    for layer in "${layers[@]}"; do
      for package in $layer; do
        if visible "$package"; then
          echo "published $package@$version"
        else
          echo "pending   $package@$version"
        fi
      done
    done
    ;;
  publish)
    layer_number=0
    for layer in "${layers[@]}"; do
      layer_number=$((layer_number + 1))
      if [[ -t 0 ]]; then
        read -r -p "Publish layer $layer_number: $layer? [y/N] " answer
        [[ "$answer" == "y" || "$answer" == "Y" ]] || exit 0
      else
        echo "publish requires an interactive terminal for layer confirmation" >&2
        exit 2
      fi
      for package in $layer; do
        if visible "$package"; then
          echo "skipping existing $package@$version"
          continue
        fi
        cargo publish --package "$package"
        wait_until_visible "$package"
      done
    done
    ;;
  *) usage ;;
esac
