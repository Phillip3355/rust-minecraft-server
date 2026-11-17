# 빌드 검증 기록 — 2026-11-17

**결론: 빌드 미실행 / 미검증.** 스크립트 실행은 했으나 소스 클론 단계에서
실패하여 Cargo 컴파일에 도달하지 못했다. 성공 산출물은 없다.

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

## 커밋 제한

`git add .gitignore docs scripts`를 시도했으나 Git 메타데이터가 읽기 전용이라
실패했다. 오류: `Unable to create .../.git/worktrees/2499126e536f4e04b58eb4821497f0b7/index.lock: Read-only file system`.
따라서 이번 변경은 커밋되지 않았으며 위 작업 위치에 파일로 남아 있다.
쓰기 가능한 환경에서 `git add .gitignore docs scripts` 후
`git commit -m "build: pin Pumpkin baseline and automate Linux builds"`로 커밋할 수 있다.
