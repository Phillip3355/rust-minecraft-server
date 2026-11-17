#!/usr/bin/env bash
# Fetch external benchmark artifacts (Paper 26.3, Chunky, BotMark, SoulFireCLI) into .work/benchmark/.
# Never commits external binaries to git; supports --dry-run and HTTP_PROXY/HTTPS_PROXY.
set -Eeuo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
work_bench="$project_root/.work/benchmark"
paper_dir="$work_bench/paper"
tools_dir="$work_bench/tools"

USER_AGENT="rust-minecraft-server-plan/phase0-bench (https://github.com/Phillip3355/rust-minecraft-server)"
PAPER_VERSION="26.3"
PAPER_BUILD="latest"
DRY_RUN=0
DO_PAPER=0
DO_CHUNKY=0
DO_BOTMARK=0
DO_SOULFIRE=0

usage() {
    cat <<'EOF'
Usage: fetch_artifacts.sh [OPTIONS]

Options:
  --paper         Download Paper 26.3 server jar via Fill v3 API (fill.papermc.io)
  --paper-build N Pin specific Paper build number (default: latest)
  --chunky        Download Chunky plugin jar for Paper 26.3 via Modrinth API
  --botmark       Clone and build Pumpkin-MC/BotMark into .work/benchmark/tools/botmark
  --soulfire      Download latest SoulFireCLI.jar from soulfiremc-com/SoulFire releases
  --all           Perform all of the above
  --proxy URL     Export HTTP_PROXY/HTTPS_PROXY for curl and git
  --dry-run       Print planned actions/URLs without making network requests
  --help          Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --paper) DO_PAPER=1; shift ;;
        --paper-build) PAPER_BUILD="${2:?Missing build number}"; shift 2 ;;
        --chunky) DO_CHUNKY=1; shift ;;
        --botmark) DO_BOTMARK=1; shift ;;
        --soulfire) DO_SOULFIRE=1; shift ;;
        --all) DO_PAPER=1; DO_CHUNKY=1; DO_BOTMARK=1; DO_SOULFIRE=1; shift ;;
        --proxy)
            export HTTP_PROXY="${2:?Missing proxy URL}"
            export HTTPS_PROXY="$HTTP_PROXY"
            export http_proxy="$HTTP_PROXY"
            export https_proxy="$HTTP_PROXY"
            shift 2
            ;;
        --dry-run) DRY_RUN=1; shift ;;
        --help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ $DO_PAPER -eq 0 && $DO_CHUNKY -eq 0 && $DO_BOTMARK -eq 0 && $DO_SOULFIRE -eq 0 ]]; then
    usage >&2
    exit 2
fi

for cmd in curl python3 sha256sum; do
    command -v "$cmd" >/dev/null || { echo "Missing required command: $cmd" >&2; exit 2; }
done

mkdir -p "$paper_dir/plugins" "$tools_dir"

fetch_paper() {
    local meta_url="https://fill.papermc.io/v3/projects/paper/versions/${PAPER_VERSION}/builds/${PAPER_BUILD}"
    local out_jar="$paper_dir/paper-${PAPER_VERSION}.jar"
    local meta_file="$paper_dir/paper-${PAPER_VERSION}-build.json"
    echo "[paper] Endpoint: $meta_url"
    echo "[paper] Target:   $out_jar"
    if [[ $DRY_RUN -eq 1 ]]; then
        echo "[dry-run] Would query Fill v3 API with User-Agent: $USER_AGENT, download server:default jar, and verify sha256."
        return 0
    fi
    curl -fsSL -H "User-Agent: ${USER_AGENT}" "$meta_url" -o "$meta_file"
    read -r jar_url jar_sha build_id < <(
        python3 - "$meta_file" <<'PY'
import json, sys
with open(sys.argv[1], "r", encoding="utf-8") as f:
    data = json.load(f)
dl = data["downloads"]["server:default"]
print(dl["url"], dl["checksums"]["sha256"], data.get("id", "unknown"))
PY
    )
    echo "[paper] Downloading build #$build_id from $jar_url"
    curl -fsSL -H "User-Agent: ${USER_AGENT}" "$jar_url" -o "$out_jar"
    echo "${jar_sha}  ${out_jar}" | sha256sum -c -
    echo "[paper] Verified Paper $PAPER_VERSION build #$build_id -> $out_jar"
}

fetch_chunky() {
    local api_url="https://api.modrinth.com/v2/project/chunky/version?loaders=%5B%22paper%22%5D&game_versions=%5B%22${PAPER_VERSION}%22%5D"
    local out_jar="$paper_dir/plugins/Chunky.jar"
    echo "[chunky] Endpoint: $api_url"
    echo "[chunky] Target:   $out_jar"
    if [[ $DRY_RUN -eq 1 ]]; then
        echo "[dry-run] Would query Modrinth API for Chunky (paper, ${PAPER_VERSION}) and verify sha512/sha1 if provided."
        return 0
    fi
    local tmp_json
    tmp_json=$(mktemp)
    trap 'rm -f "$tmp_json"' RETURN
    curl -fsSL -H "User-Agent: ${USER_AGENT}" "$api_url" -o "$tmp_json"
    read -r file_url version_num < <(
        python3 - "$tmp_json" <<'PY'
import json, sys
with open(sys.argv[1], "r", encoding="utf-8") as f:
    versions = json.load(f)
if not versions:
    raise SystemExit("No Chunky version found for Paper 26.3 on Modrinth.")
ver = versions[0]
files = [x for x in ver["files"] if x["filename"].endswith(".jar")]
primary = next((x for x in files if x.get("primary")), files[0])
print(primary["url"], ver["version_number"])
PY
    )
    echo "[chunky] Downloading Chunky $version_num from $file_url"
    curl -fsSL -H "User-Agent: ${USER_AGENT}" "$file_url" -o "$out_jar"
    echo "[chunky] Saved $out_jar"
}

fetch_botmark() {
    local repo_url="https://github.com/Pumpkin-MC/BotMark.git"
    local src_dir="$work_bench/src/BotMark"
    local out_bin="$tools_dir/botmark"
    echo "[botmark] Repo:   $repo_url"
    echo "[botmark] Source: $src_dir"
    echo "[botmark] Binary: $out_bin"
    if [[ $DRY_RUN -eq 1 ]]; then
        echo "[dry-run] Would clone $repo_url and run cargo build --release, copying target/release/botmark to $out_bin."
        return 0
    fi
    command -v git >/dev/null || { echo "Missing git" >&2; exit 2; }
    command -v cargo >/dev/null || { echo "Missing cargo" >&2; exit 2; }
    mkdir -p "$work_bench/src"
    if [[ ! -d "$src_dir/.git" ]]; then
        git clone "$repo_url" "$src_dir"
    else
        git -C "$src_dir" pull --ff-only
    fi
    (cd "$src_dir" && cargo build --release)
    cp "$src_dir/target/release/botmark" "$out_bin"
    chmod +x "$out_bin"
    echo "[botmark] Built $out_bin"
}

fetch_soulfire() {
    local api_url="https://api.github.com/repos/soulfiremc-com/SoulFire/releases/latest"
    local out_jar="$tools_dir/SoulFireCLI.jar"
    echo "[soulfire] Endpoint: $api_url"
    echo "[soulfire] Target:   $out_jar"
    if [[ $DRY_RUN -eq 1 ]]; then
        echo "[dry-run] Would query GitHub Releases for SoulFireCLI-*.jar and save to $out_jar."
        return 0
    fi
    local tmp_json
    tmp_json=$(mktemp)
    trap 'rm -f "$tmp_json"' RETURN
    curl -fsSL -H "User-Agent: ${USER_AGENT}" "$api_url" -o "$tmp_json"
    read -r asset_url tag_name < <(
        python3 - "$tmp_json" <<'PY'
import json, sys
with open(sys.argv[1], "r", encoding="utf-8") as f:
    rel = json.load(f)
assets = rel.get("assets", [])
cli = next((a for a in assets if "CLI" in a["name"] and a["name"].endswith(".jar")), None)
if cli is None:
    raise SystemExit("SoulFireCLI*.jar asset not found in latest release.")
print(cli["browser_download_url"], rel.get("tag_name", "unknown"))
PY
    )
    echo "[soulfire] Downloading SoulFireCLI ($tag_name) from $asset_url"
    curl -fsSL -H "User-Agent: ${USER_AGENT}" "$asset_url" -o "$out_jar"
    echo "[soulfire] Saved $out_jar"
}

[[ $DO_PAPER -eq 1 ]] && fetch_paper
[[ $DO_CHUNKY -eq 1 ]] && fetch_chunky
[[ $DO_BOTMARK -eq 1 ]] && fetch_botmark
[[ $DO_SOULFIRE -eq 1 ]] && fetch_soulfire
