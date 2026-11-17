# Pumpkin 기준선 고정 정보

조사일: 2026-11-17. 기계가 읽는 값은 [`scripts/pumpkin-pin.env`](../scripts/pumpkin-pin.env)에 둔다.

| 항목 | 고정 값 |
|---|---|
| 원본 저장소 | https://github.com/Pumpkin-MC/Pumpkin.git |
| 릴리스/태그 | `0.2.2+26.3-26.60` |
| 전체 커밋 SHA | `eda69440e6369c5f8a05c3f69fc809c40dab93a6` |
| Rust | 프로젝트 선택 `1.100.0` (고정 소스 MSRV `1.97`) |
| 타깃 | `x86_64-unknown-linux-gnu`, CPU `x86-64` |
| Cargo | 해당 Rust 배포판에 포함된 Cargo, upstream `Cargo.lock` + `--locked` |

## 선택 근거와 한계

[공식 릴리스 목록](https://github.com/Pumpkin-MC/Pumpkin/releases)에서
`0.2.2+26.3-26.60`이 Latest 정식 릴리스로 표시되었다(11월 8일).
Nightly와 Canary는 Pre-release여서 제외했다.
[릴리스 페이지](https://github.com/Pumpkin-MC/Pumpkin/releases/tag/0.2.2%2B26.3-26.60)에서
연결한 [커밋 페이지](https://github.com/Pumpkin-MC/Pumpkin/commit/eda69440e6369c5f8a05c3f69fc809c40dab93a6)의
전체 SHA를 고정했다. 재실행 시 이동 가능한 태그를 빌드 기준으로 사용하지 않는다.

이는 **상대적으로 안정된 릴리스 기준선 선택**이다. 자체 빌드·장시간 운영으로
안정성을 입증한 커밋이라는 의미는 아니다. 공식 사이트도 미완성 메커니즘과
버전 간 월드 파손 가능성을 안내한다([현재 상태](https://pumpkinmc.org/)).
릴리스가 명시하는 Java 26.3 / Bedrock 1.26.60 지원은 upstream의 주장으로 기록한다.
실접속, 최신 Bedrock과의 일치 여부, Paper 비교, 데이터팩 및 Anvil 저장은 미검증이다.
계획서 전체 Phase 0 완료 조건을 충족했다고 보고하지 않는다.

## 소스 구조 확인

[해당 태그의 루트 Cargo.toml](https://raw.githubusercontent.com/Pumpkin-MC/Pumpkin/0.2.2%2B26.3-26.60/Cargo.toml)을
웹 도구로 열어 Rust 2024 edition, MSRV, 릴리스 프로파일과 workspace 구성을 확인했다.
`crates/`의 멤버는 다음 23개이며 `tools/`의 3개와 합쳐 총 26개다.

```text
pumpkin, pumpkin-core, pumpkin-api-macros, pumpkin-auth,
pumpkin-codecs, pumpkin-command, pumpkin-config, pumpkin-data,
pumpkin-scheduler, pumpkin-gametest, pumpkin-inventory, pumpkin-macros,
pumpkin-nbt, pumpkin-plugin-api, pumpkin-plugin-runtime, pumpkin-plugin-utils,
pumpkin-protocol, pumpkin-util, pumpkin-wasm-host, pumpkin-wasm-host-common,
pumpkin-wasm-host-v0_1, pumpkin-wasm-host-v0_2, pumpkin-world
tools/pumpkin-codegen, tools/pumpkin-fuzzer, tools/pumpkin-tester
```

빌드 대상은 서버 패키지 `pumpkin`으로 한정한다. 개별 패키지 manifest와
`.cargo/config.toml`은 웹 조회에서도 가져오지 못했다. 따라서 바이너리 이름,
기본 feature 조합 및 네이티브 의존성의 충분성은 실제 빌드로 확인해야 한다.
소스 수정으로 우회하지 말고 실패 로그를 남겨 기준선 변경 여부를 판단한다.
공식 [기여 가이드](https://docs.pumpkinmc.org/developer/contributing)는 서브모듈의
재귀 초기화와 `cargo build --release`를 안내한다.

## 갱신 정책

새 릴리스 선택 시 릴리스 URL·전체 SHA·MSRV·workspace·Cargo.lock을 다시 확인하고
pin 파일과 이 문서를 함께 변경한다. 별도 체크아웃에서 빌드하고 기존 기준선의
로그를 보존한다. 호환성 검증과 실측 결과를 확보한 뒤 채택한다.
