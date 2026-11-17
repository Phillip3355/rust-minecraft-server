# 빌드 검증 기록 — 2026-11-17

**현재 결론: 재시도도 클론 단계에서 실패 / Cargo 빌드 미실행·미검증.**
2026-11-17T13:12:31Z에 실제 스크립트를 다시 실행했으나 종료 코드 128로
중단됐다. 실행 세션의 네트워크 소켓 생성이 허용되지 않았다. 성공 바이너리는 없다.

## 재개 실행 — 2026-11-17 13:12 UTC (22:12 KST)

- 요청한 원본 저장소: `/home/phillip/rust-minecraft-server-plan`.
- 실제 작업 위치: 해당 저장소의 연결된 워크트리
  `/home/phillip/.local/state/hermes-orchestrator/worktrees/1e498bdeae9a4bb88d82297e8d45b1c8`.
  원본 경로와 Git 공통 메타데이터는 세션의 쓰기 허용 범위 밖이다.
- 기준 HEAD: `9ce5bcac52007d52850da6387ff16b44a798a770` (detached HEAD).
- 명령: `/usr/bin/time -p ./scripts/build.sh`.
- 결과: `exit_code=128`, `phase=prepare`, 경과 시간 **0.01초**
  (GNU time의 소수 둘째 자리 출력; user/sys 각각 0.00초). 컴파일 시간이 아니다.
- 오류: `Could not resolve host: github.com`. `getent hosts github.com`도
  주소를 반환하지 않았고 `curl -I --connect-timeout 10 https://github.com`은
  종료 코드 6으로 실패했다.
- `/etc/resolv.conf`는 로컬 DNS stub `127.0.0.53`을 가리킨다. Python socket으로
  DNS stub(UDP)과 `1.1.1.1:443`(TCP)에 각각 접근을 진단했으며, 두 경우 모두
  `PermissionError(1, 'Operation not permitted')`가 발생했다. 따라서 호스트에서
  네트워크가 복구됐다는 사용자 보고와 별개로, 이 실행 세션에서는 소켓 사용이
  차단됨을 확인했다. DNS 설정이나 프로젝트 소스 변경으로 해결할 문제는 아니다.
- 이 세션은 네트워크 제한 및 승인 불가 정책으로 실행된다. 외부 실행 권한을
  요청하거나 시스템 설정을 수정하지 않았다.
- 재사용 소스: 원본 저장소의 `.work`, `/tmp/phase0-pumpkin-source`가 없었다.
  기본 Cargo registry/git 캐시도 없어서 오프라인 Cargo 빌드도 진행할 수 없었다.
- Rust: `stable`은 `rustc 1.100.0 (0ef1bc386 2026-11-09)`,
  `cargo 1.100.0 (3acc79c0c 2026-11-05)`. 그러나
  `rustup run 1.100.0 rustc -V`는 해당 버전명 toolchain 미설치로 실패했다.
  클론 차단을 해소한 뒤 버전명 toolchain 설치도 필요하다. 이번 실행의 직접
  실패 원인은 toolchain이 아니라 클론 단계의 네트워크 제한이다.
- `npm`, `node`, `cc`는 PATH에 있고 `cmake`, `clang`은 발견되지 않았다.
  Cargo에 도달하지 않았으므로 npm·WASM·시스템 라이브러리 부족에 따른
  컴파일 오류 여부는 진단할 수 없다. 시스템 패키지 설치나 pin 변경은 하지 않았다.
- `bash -n scripts/build.sh`, `./scripts/build.sh --help`: 종료 코드 0.
- 원본 증거: `.work/build-maqyfnSn/` (Git 제외). 보존 사본:
  [실행 로그](validation/build-resume-20261117.log),
  [결과](validation/build-resume-20261117-result.txt),
  [측정 시간](validation/build-resume-20261117-time.txt),
  [환경 진단](validation/build-resume-20261117-diagnostics.txt).
- 생성 바이너리: **없음**. `build-info.txt`, `pumpkin.sha256`도 생성되지 않았다.
  성공 시 예상 경로만 `.work/build-XXXXXXXX/target/x86_64-unknown-linux-gnu/release/pumpkin`이다.

후속 조치는 네트워크 소켓과 Git 메타데이터 쓰기가 가능한 로컬 실행 환경에서
BUILD.md의 버전명 toolchain을 준비한 뒤 `./scripts/build.sh`를 실행하는 것이다.
이번 재시도로 실제 컴파일 성공이나 Phase 0 완료를 주장할 수 없다.

## 재개 실행의 커밋·push 결과

- `git add docs/BUILD.md docs/BUILD-VALIDATION.md docs/PUMPKIN-PIN.md docs/validation/build-resume-20261117*`: 종료 코드 128.
- `git commit -m "docs: record Pumpkin build retry blocked by sandbox networking"`: 종료 코드 128.
- 두 명령 모두 다음 Git 메타데이터 파일 생성이 읽기 전용 파일 시스템으로 거부됐다:
  `/home/phillip/rust-minecraft-server-plan/.git/worktrees/1e498bdeae9a4bb88d82297e8d45b1c8/index.lock`.
- `git push origin HEAD:main`도 시도했지만 종료 코드 128,
  `Could not resolve host: github.com`으로 실패했다. 새 커밋이나 원격 반영은 없다.
- 변경분은 실제 작업 위치의 `docs/BUILD.md`, `docs/BUILD-VALIDATION.md`,
  `docs/PUMPKIN-PIN.md`, `docs/validation/build-resume-20261117*`에 남아 있다.
  원본 체크아웃 파일에 직접 복사하지 않았다(쓰기 허용 범위 밖).

## 이전 실행 기록 (재개 실행과 구분)

## 실제 환경과 실행

- 작업 위치: `/home/phillip/.local/state/hermes-orchestrator/worktrees/2499126e536f4e04b58eb4821497f0b7`
- 원본 Git 저장소: `/home/phillip/rust-minecraft-server-plan`의 연결된 워크트리.
- OS: Linux Mint 22.3 (Ubuntu noble 계열), Linux x86_64.
- 설치된 `stable` 별칭: `rustc 1.100.0 (0ef1bc386 2026-11-09)`,
  `cargo 1.100.0 (3acc79c0c 2026-11-05)`.
- `rustup toolchain list`에는 `stable-x86_64-unknown-linux-gnu`만 표시되었다.
  버전명 `1.100.0` toolchain 설치 및 타깃 준비는 아직 하지 않았다.
- Git, curl, cc, pkg-config는 존재했다. CMake는 PATH에서 발견되지 않았다.

독립적인 직접 클론도 다음 오류로 실패했다(종료 코드 128):

```text
git clone --no-checkout https://github.com/Pumpkin-MC/Pumpkin.git /tmp/phase0-pumpkin-source
fatal: unable to access 'https://github.com/Pumpkin-MC/Pumpkin.git/': Could not resolve host: github.com
```

자동화 실행: `./scripts/build.sh`, 시작 `2026-11-17T12:56:26Z`.
실제 로그는 [build-attempt.log](validation/build-attempt.log), 상태는
[build-result.txt](validation/build-result.txt)에 복사해 보존했다.
원본 로그 디렉터리는 `.work/build-JeIqGcf2/`이며 Git에서 제외된다.
GitHub API에 대한 curl도 `Could not resolve host: api.github.com`으로 실패했다.
웹 도구 조회는 가능해서 릴리스/SHA/루트 manifest 조사는 수행했다.

## 확인한 것과 확인하지 못한 것

- `bash -n scripts/build.sh`: 통과.
- `./scripts/build.sh --help`: 사용법 출력, 종료 코드 0.
- 실제 실행 시 다운로드 실패를 성공으로 처리하지 않음: `exit_code=128`, `phase=prepare`.
- `./scripts/build.sh .`: 계획 저장소를 소스로 잘못 지정하면 SHA 불일치로
  종료 코드 2를 반환했다. 계획 저장소의 HEAD와 파일을 바꾸지 않았다.
- 독립 코드 검토에서 서브모듈 무시 설정에 따른 검사 누락을 발견했다.
  임시 로컬 Git 저장소에서 수정된 서브모듈이 기존 검사에 숨는 것을 재현하고,
  `--ignore-submodules=none` 및 재귀 작업 트리 검사로 보완했다.
  실제 `check_clean` 함수가 수정된 서브모듈을 종료 코드 2로 거부하고,
  복원한 깨끗한 서브모듈은 종료 코드 0으로 통과시키는 것을 확인했다.
  보완 후 독립 재검토에서도 중요한 추가 문제는 발견되지 않았다.
- 소스 체크아웃, 서브모듈, 의존성 다운로드, 컴파일, 링킹: **미검증**.
- 서버 시작, Java/Bedrock 접속, 게임 기능, 저장 무결성, 성능: **미검증**.

네트워크가 가능한 머신에서 BUILD.md의 패키지와 버전명 Rust toolchain을 준비하고
`./scripts/build.sh`를 다시 실행해야 한다. 현재 로그로 빌드 성공이나 운영 안정성을
주장할 수 없다.

## 이전 실행의 커밋 제한

`git add .gitignore docs scripts`를 시도했으나 Git 메타데이터가 읽기 전용이라
실패했다. 오류: `Unable to create .../.git/worktrees/2499126e536f4e04b58eb4821497f0b7/index.lock: Read-only file system`.
따라서 이번 변경은 커밋되지 않았으며 위 작업 위치에 파일로 남아 있다.
쓰기 가능한 환경에서 `git add .gitignore docs scripts` 후
`git commit -m "build: pin Pumpkin baseline and automate Linux builds"`로 커밋할 수 있다.
