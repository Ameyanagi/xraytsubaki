#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "${os}" in
  darwin) os="macos" ;;
  mingw*|msys*|cygwin*) os="windows" ;;
esac

arch="$(uname -m)"
case "${arch}" in
  arm64) arch="aarch64" ;;
  amd64) arch="x86_64" ;;
esac
platform_dir="${os}-${arch}"

resource_dir="${APP_DIR}/src-tauri/resources/feff/${platform_dir}"
mkdir -p "${resource_dir}"

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/feff85exafs.XXXXXX")"
cleanup() {
  rm -rf "${work_dir}"
}
trap cleanup EXIT

echo "[bundle-feff] cloning FEFF85EXAFS into ${work_dir}"
git clone --depth 1 https://github.com/xraypy/feff85exafs.git "${work_dir}/src"

pushd "${work_dir}/src" >/dev/null
  echo "[bundle-feff] building FEFF85EXAFS (make install)"
  make install
popd >/dev/null

bin_dir="${work_dir}/src/local_install/bin"
modules=(
  feff8l_rdinp
  feff8l_pot
  feff8l_xsph
  feff8l_pathfinder
  feff8l_genfmt
  feff8l_ff2x
)

exe_suffix=""
if [[ "${os}" == "windows" ]]; then
  exe_suffix=".exe"
fi

for module in "${modules[@]}"; do
  src_path="${bin_dir}/${module}${exe_suffix}"
  if [[ ! -f "${src_path}" ]]; then
    echo "[bundle-feff] missing expected module: ${src_path}" >&2
    exit 1
  fi
  cp -f "${src_path}" "${resource_dir}/${module}${exe_suffix}"
  chmod +x "${resource_dir}/${module}${exe_suffix}"
done

echo "[bundle-feff] copied FEFF modules to ${resource_dir}"
echo "[bundle-feff] keep THIRD_PARTY_NOTICES.md with redistributions"
