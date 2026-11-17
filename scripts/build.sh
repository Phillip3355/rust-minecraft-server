#!/usr/bin/env bash
# Build the pristine pinned upstream baseline; never reset an existing checkout.
set -Eeuo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
# shellcheck source=pumpkin-pin.env
source "$project_root/scripts/pumpkin-pin.env"
if [[ ${1:-} == --help ]]; then
    echo "Usage: $0 [SOURCE_DIRECTORY]"
    echo 'Default source: PROJECT/.work/pumpkin; logs/artifacts: PROJECT/.work/build-*'
    exit 0
fi
[[ $# -le 1 ]] || { echo 'Expected at most one source directory.' >&2; exit 2; }
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || {
    echo 'Requires native Linux x86-64.' >&2; exit 2;
}
for tool in git rustup cc c++ make pkg-config sha256sum tee; do
    command -v "$tool" >/dev/null || { echo "Missing dependency: $tool" >&2; exit 2; }
done
mkdir -p "$project_root/.work"
run_dir=$(mktemp -d "$project_root/.work/build-XXXXXXXX")
exec > >(tee "$run_dir/build.log") 2>&1
phase=prepare
trap 'result=$?; printf "exit_code=%s\nphase=%s\n" "$result" "$phase" > "$run_dir/result.txt"; echo "Result: exit=$result phase=$phase; evidence: $run_dir"' EXIT
echo "Started: $(date -u +%FT%TZ)"
echo "Pinned source: $PUMPKIN_REPOSITORY @ $PUMPKIN_COMMIT ($PUMPKIN_VERSION)"
source_dir=${1:-"$project_root/.work/pumpkin"}
if [[ ! -e "$source_dir" ]]; then
    git clone --no-checkout "$PUMPKIN_REPOSITORY" "$source_dir"
    git -C "$source_dir" fetch origin "$PUMPKIN_COMMIT"
    git -C "$source_dir" checkout --detach "$PUMPKIN_COMMIT"
fi
source_dir=$(cd -- "$source_dir" && pwd -P)
[[ $(git -C "$source_dir" rev-parse --show-toplevel) == "$source_dir" ]] || {
    echo 'Source must be the root of its own Git checkout.' >&2; exit 2;
}
[[ $(git -C "$source_dir" rev-parse HEAD) == "$PUMPKIN_COMMIT" ]] || {
    echo 'Source SHA mismatch; use a separate pristine checkout at the pinned SHA.' >&2; exit 2;
}
cd -- "$source_dir"
check_clean() {
    local state
    state=$(git status --porcelain --untracked-files=all --ignore-submodules=none)
    [[ -z "$state" ]] || {
        echo 'Source has local changes or untracked files; refusing to alter it.' >&2
        return 2
    }
    git submodule foreach --quiet --recursive '
        state=$(git status --porcelain --untracked-files=all --ignore-submodules=none) || exit
        if test -n "$state"; then
            echo "Dirty submodule: $displaypath" >&2
            exit 2
        fi
    '
}
check_clean
# Reject existing revision changes before update can switch a submodule checkout.
submodule_state=$(git submodule status --recursive)
if grep -qE '^[+U]' <<< "$submodule_state"; then
    echo 'Existing submodule revision mismatch; refusing to alter it.' >&2; exit 2
fi
git submodule update --init --recursive
check_clean
submodule_state=$(git submodule status --recursive)
if grep -qE '^[+U-]' <<< "$submodule_state"; then
    echo 'Submodule revision mismatch.' >&2; exit 2
fi
[[ -f Cargo.lock ]] || { echo 'Pinned Cargo.lock is required.' >&2; exit 2; }
phase=toolchain
rustup run "$PUMPKIN_TOOLCHAIN" rustc --version
rustup run "$PUMPKIN_TOOLCHAIN" cargo --version
# Require the versioned toolchain; never silently use/update the moving stable alias.
rustup run "$PUMPKIN_TOOLCHAIN" rustc --print target-libdir --target "$PUMPKIN_TARGET" >/dev/null
phase=build
export CARGO_TARGET_DIR="$run_dir/target"
export RUSTFLAGS='-C target-cpu=x86-64'
unset CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
{
    date -u +%FT%TZ
    uname -srmo
    git rev-parse HEAD
    git submodule status --recursive
    sha256sum Cargo.lock
    rustup run "$PUMPKIN_TOOLCHAIN" rustc -Vv
    rustup run "$PUMPKIN_TOOLCHAIN" cargo -V
    cc --version
    ldd --version
    echo "RUSTFLAGS=$RUSTFLAGS"
    echo "target=$PUMPKIN_TARGET; profile=release; default features; package=pumpkin"
} > "$run_dir/build-info.txt"
rustup run "$PUMPKIN_TOOLCHAIN" cargo build --locked --release \
    --target "$PUMPKIN_TARGET" --package pumpkin --bin pumpkin
git diff --exit-code -- Cargo.lock
binary="$CARGO_TARGET_DIR/$PUMPKIN_TARGET/release/pumpkin"
[[ -x "$binary" ]] || { echo 'Expected binary missing.' >&2; exit 1; }
sha256sum "$binary" > "$run_dir/pumpkin.sha256"
phase=complete
echo "Build succeeded: $binary"
