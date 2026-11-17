# Linux x86-64 빌드 및 로컬 포크 준비

범위: 계획서 v1.2 Phase 0 중 upstream 고정, 포크 준비, 빌드 자동화.
기준선과 조사 출처는 [PUMPKIN-PIN.md](PUMPKIN-PIN.md), 실제 검증 결과는
[BUILD-VALIDATION.md](BUILD-VALIDATION.md)에 있다.

## 빌드 환경

네이티브 Linux x86-64 + glibc 환경을 사용한다. Debian/Ubuntu 계열 준비 예시:

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends \
  git curl ca-certificates build-essential pkg-config cmake clang libclang-dev
```

Git, Bash, GNU coreutils, C/C++ 컴파일러, 링커와 make가 필요하다. CMake와
Clang/libclang은 네이티브 의존성 빌드를 대비한 보수적 준비 목록이며,
고정 소스에서 모두 필수인지 또는 이 목록만으로 충분한지는 아직 미검증이다.
이 작업 환경에서는 패키지를 설치하지 않았다. 소스 및 의존성 다운로드를 위해
GitHub, crates.io와 Rust 배포 서버에 HTTPS/DNS 접근이 필요하다.

Rustup이 없다면 [공식 설치 안내](https://www.rust-lang.org/tools/install)에 따라
설치한다. 이후 프로젝트가 선택한 정확한 버전을 준비한다:

```bash
rustup toolchain install 1.100.0 --profile minimal
rustup target add --toolchain 1.100.0 x86_64-unknown-linux-gnu
rustup run 1.100.0 rustc -Vv
rustup run 1.100.0 cargo -V
```

`stable` 별칭은 시간이 지나면 달라져 사용하지 않는다. `1.100.0`은 이 환경에서
확인한 Rust 버전이며 upstream MSRV `1.97` 이상이다. 이 조합의 컴파일 성공을
입증한 것은 아니다. 코어 빌드에는 Java 서버 실행을 포함하지 않는다.
PatchBukkit/JVM 연동은 별도 작업이다. 메모리·디스크 최소 요구량도 아직 실측하지
않았으므로 대형 release/LTO 빌드가 OOM이면 로그와 머신 자원을 기록한다.
`CARGO_BUILD_JOBS=2 ./scripts/build.sh` 등으로 병렬도를 줄일 수 있다.

## 자동 준비와 빌드

이 계획 저장소 루트에서:

```bash
./scripts/build.sh
```

기본적으로 `.work/pumpkin`에 원본 저장소를 복제하고 고정 SHA를 detached HEAD로
체크아웃한다. 서브모듈도 상위 커밋에 기록된 revision으로 초기화한다.
이미 디렉터리가 있으면 루트 경로·SHA·깨끗한 상태를 검사하고, 불일치 시 중단한다.
로컬 변경을 reset/clean하거나 다른 브랜치로 강제 전환하지 않는다.

실제 빌드 명령은 다음과 같다. CPU 호환성을 위해 `native`나 `x86-64-v3`를 쓰지 않는다.
프로파일과 기본 feature는 고정된 upstream 설정을 사용한다.

```bash
RUSTFLAGS='-C target-cpu=x86-64' rustup run 1.100.0 cargo build \
  --locked --release --target x86_64-unknown-linux-gnu --package pumpkin --bin pumpkin
```

실행마다 `.work/build-XXXXXXXX/`가 생긴다. `build.log`, `result.txt`는 실패 시에도
남는다. 빌드 단계에 도달하면 `build-info.txt`에 SHA·서브모듈·lock 해시·컴파일러·
OS·플래그를 기록한다. 성공 시 바이너리는
`target/x86_64-unknown-linux-gnu/release/pumpkin`, 해시는 `pumpkin.sha256`에 있다.
성공 조건은 Cargo 종료 코드 0, lock 불변, 실행 가능한 산출물 존재다.
이 스크립트는 서버를 시작하거나 운영 월드를 열지 않는다.

`--locked`는 의존성 버전 재해석을 막지만 다운로드 자체는 허용한다. 완전 오프라인
빌드를 위해서는 Cargo registry/git 캐시와 서브모듈, toolchain을 별도로 확보해야 한다.
소스·도구 버전·타깃을 고정해 재실행 가능한 절차를 제공하며, OS 패키지/링커 및
Cargo 설정·환경까지 격리한 bit-for-bit 동일 바이너리는 보장하지 않는다.
비교 빌드는 같은 OS/패키지/환경을 사용하고 해시를 직접 비교해야 한다.

## 수동 클론과 로컬 포크

자동 스크립트와 동일한 기준선에서 작업 브랜치를 만드는 절차다. `PUMPKIN_DIR`은
존재하지 않는 새 경로로 선택한다. 원본 계획 저장소의 remote는 변경하지 않는다.

```bash
# 계획 저장소 루트에서 실행
source scripts/pumpkin-pin.env
PUMPKIN_DIR="$PWD/.work/pumpkin-fork"
git clone --no-checkout "$PUMPKIN_REPOSITORY" "$PUMPKIN_DIR"
git -C "$PUMPKIN_DIR" fetch origin "$PUMPKIN_COMMIT"
git -C "$PUMPKIN_DIR" checkout --detach "$PUMPKIN_COMMIT"
git -C "$PUMPKIN_DIR" submodule update --init --recursive
git -C "$PUMPKIN_DIR" switch -c project/phase0
git -C "$PUMPKIN_DIR" remote rename origin upstream
git -C "$PUMPKIN_DIR" rev-parse HEAD
git -C "$PUMPKIN_DIR" submodule status --recursive
./scripts/build.sh "$PUMPKIN_DIR"
```

HEAD 출력이 pin 파일과 같고 서브모듈 상태에 `-`, `+`, `U`가 없어야 한다.
확장 코드는 이 로컬 브랜치에서 개발하되 이 빌드 스크립트는 **수정 없는 upstream
기준선 전용**이므로, 커밋이 달라진 포크의 검증은 별도로 구성한다.

GitHub 원격 포크가 필요하면 사용자 계정에서
[원본 저장소](https://github.com/Pumpkin-MC/Pumpkin)를 Fork한 뒤, 실제 소유자와 URL을
확인하고 로컬 Pumpkin 저장소에서 다음 명령을 실행한다:

```bash
git remote add origin https://github.com/YOUR_ACCOUNT/Pumpkin.git
git push -u origin project/phase0
```

위 두 명령은 원격 포크를 준비한 이후의 선택 절차다. 이번 작업에서는 GitHub
원격 포크를 생성하거나 push하지 않았으며, 로컬 클론도 DNS 오류로 생성되지 않았다.
코드 재배포 시 고정 소스의 GPL-3.0 라이선스와 고지 파일을 유지한다.

## 이 변경의 실행 순서와 후속 검증

1. 공식 릴리스와 SHA를 확인하고 pin 파일에 기록한다.
2. 클론·검사·빌드·로그 절차를 자동화한다.
3. 셸 문법, 도움말, 오류 시 종료/로그를 확인한다.
4. 실제 스크립트를 실행하고 결과를 정직하게 기록한 뒤 커밋한다.

네트워크가 가능한 환경에서 이 절차를 재실행하고 성공 로그를 확보하는 것이 다음
작업이다. 이후 Java/Bedrock 접속·Anvil 저장/재로드·Paper 기준 성능 비교를 별도로
수행해야 Phase 0 전체 완료 여부를 판단할 수 있다.
