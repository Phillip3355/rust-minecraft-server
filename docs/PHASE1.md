# Phase 1 — 기본 생존 엔진 안정화 (진행 기록)

- 기준 문서: `Minecraft-Rust-Server-Plan.md` v1.2 §7 Phase 1, §3 P0, §6(스토리지), §10(바닐라 파리티 리스크)
- 대상: Pumpkin `0.2.2+26.3-26.60` @ `eda69440e6369c5f8a05c3f69fc809c40dab93a6` (`scripts/pumpkin-pin.env`)
- 작성일: 2026-11-17 · 환경: Linux 7.0.0-28-generic x86_64, rustc/cargo 1.100.0, Python 3.14.9

## 1. 상태 요약 (정직 표기)

| 항목 | 상태 | 근거 |
|---|---|---|
| Pumpkin 소스 로컬 포크(고정 커밋) | **미수행 — 블로커** | 네트워크 차단(Phase 0 `docs/validation/build-resume-20261117*.log`: clone 단계 `Operation not permitted`, exit 128). 이번 세션에서도 외부 접근(`git ls-remote`)은 승인 불가로 실행하지 못함. 작업 디렉터리(worktree) 안에 Pumpkin 체크아웃·`.work/` 없음(그 밖의 경로는 검색 권한 밖이라 확인하지 못함). |
| Pumpkin 바이너리 기동 / 실플레이 | **미검증** | 바이너리 없음(`.work/pumpkin-bin/pumpkin` 부재) |
| 월드 무결성 검증기 `tools/world-verify` | **구현 + 실행 검증 완료** | `cargo test --offline`: 단위 24 + 회귀 26 = **50/50 통과** (`docs/validation/phase1-world-verify-tests.log`) |
| 저장/재실행·크래시 하네스 `scripts/phase1/save_reload_check.py` | **작성 완료, 미실행** | 이 세션에서 Python 실행이 승인되지 않아 `--self-test`·문법 검사도 실행하지 못함. 실 Pumpkin 모드는 바이너리 부재로 실행 시 `NOT_RUN`(exit 3) 기록 예정 |
| 생존 시나리오(채집→네더→엔드) 연속 플레이 | **NOT TESTED** | 서버/클라이언트 부재 |
| 바닐라 미완성 메커니즘 목록 | **미작성 (추측 금지)** | 고정 커밋 소스를 열람할 수 없음. 목록 작성 절차는 §6 참조 |

**Phase 1 완료 조건은 충족되지 않았다.** 이번 단계 산출물은 "Pumpkin이 쓴 월드가 손상·중복·유실 없이 저장/재로드되는지"를 판정하는 **검증 도구와 회귀 테스트 기반**이며, Pumpkin 자체에 대한 판정 결과는 아직 없다.

## 2. 산출물

### 2.1 `tools/world-verify` (Rust, 외부 의존성 0)

오프라인 샌드박스에서 `cargo build --offline`으로 빌드되도록 crates.io 의존성 없이 작성. zlib/gzip inflate, lz4-java `LZ4Block` 스트림, CRC32/Adler-32/XXH32, NBT 리더/라이터를 내장 구현.

| 명령 | 검사 | Phase 1 목표 대응 |
|---|---|---|
| `verify <world>` | 리전 헤더/섹터 경계·중첩, 압축 체크섬(1 gzip/2 zlib/3 none/4 LZ4, 외부 `.mcc`), NBT 무결성(절단·중복 키·깊이·길이 상한), 청크 `xPos/zPos`·`Status`·섹션 Y 중복, 블록/바이옴 팔레트·packed index 범위, 블록 엔티티 좌표, `level.dat`, playerdata(UUID 파일명, Pos 유한/경계, Health, foodLevel, 인벤토리·엔더상자 Slot 중복/범위), 잔여 `.tmp` | 청크 손상, 크래시 후 절단, 플레이어 데이터 손상 |
| `diff <before> <after> [--strict]` | 청크 유실, 플레이어 파일 유실, 시드 변경; `--strict`(중간에 플레이 없음): 블록·블록엔티티 변경, 인벤토리/엔더/XP/허기/차원/위치 변경 | 저장/재실행 무결성, 플레이어 데이터 유실 |
| `items` / `conserve <before> <after>` | 플레이어·컨테이너(셜커/번들 중첩 포함)·아이템 엔티티 전체 아이템 합계; 증가=중복(error), 감소=유실(warning, `--strict-loss` 시 error), `--allow`로 시나리오상 정상 소모 제외; 스택 수량 0 이하/99 초과 | 중복 아이템, 아이템 유실 |
| `fixture <dir>` | 합성 월드 생성(검증기 자체 테스트 전용 — Pumpkin 산출물 아님) | — |

종료 코드: 0 오류 없음 / 1 오류 발견 / 2 사용법·IO 오류. `--json FILE`로 기계 판독 보고서.

주의(확인되지 않은 가정, 실 월드로 확정 필요):
- 청크 레이아웃은 바닐라 1.18+ 형식(`sections`, `block_states`, `block_entities`) 기준. Pumpkin 고정 커밋이 실제로 어떤 키/레이아웃을 쓰는지는 미확인 → 첫 실측에서 `verify` 결과로 확인.
- 26.3 DataVersion 값은 **가정하지 않는다**. 합성 fixture는 `DataVersion=0`. 실측 시 바닐라 26.3 월드에서 읽은 값을 `--expect-data-version`에 전달.
- 플레이어 인벤토리 Slot 허용 범위는 pre-1.21.5 레이아웃(0..35, 100..103, -106) 기준이며 범위 밖은 warning만 낸다. 1.21.5+ `equipment` 맵은 아이템 집계에 포함.
- MAX_STACK=99 (바닐라 `max_stack_size` 컴포넌트 상한).

### 2.2 회귀 테스트 (`tools/world-verify/tests/regression.rs`, 26개)

각 테스트는 합성 월드에 결함 하나를 주입하고 기대 카테고리로 탐지되는지 확인한다. **탐지기의 정확성 테스트이지 Pumpkin 동작 테스트가 아니다.**

| 영역 | 테스트 |
|---|---|
| 정상 기준선 | 4개 압축 방식 모두 오류 0 · 동일 사본 strict diff 0 · zlib↔LZ4 재압축은 변경 아님 · 아이템 합계 정확(상자/아이템엔티티/인벤토리/엔더/방어구) |
| 크래시·청크 손상 | 리전 본문 절단 · 헤더 절단 · gzip/zlib/LZ4 비트 플립 · 섹터 중첩 · 청크 좌표 불일치 · 팔레트 인덱스 범위 초과 · `level.dat` 누락/손상 · 무작위 바이트 리전에서 패닉 없음 |
| 플레이어 데이터 | 파일 유실(diff) · 재로드 후 인벤토리 변경(strict에서만 error) · Slot 중복 · NaN 좌표 · 파일 절단 |
| 중복/유실 아이템 | 상자→플레이어 복제 탐지 · 정상 이동은 보존 · 유실 warning/strict error/`--allow` · 과다 스택 |
| 청크/블록 | 청크 유실 · 블록 변경(strict에서만 error, non-strict는 통계) · 시드 변경 |
| CLI | 종료 코드 0/1/2, JSON 출력 |

이번 작업 중 테스트로 발견·수정한 결함:
1. `main.rs`: 존재하지 않는 월드 경로에서 문서상 exit 2 대신 1 반환(검사 대상 필드를 `location`→`message`로 수정).
2. 단위 테스트 `lz4_java_stream_roundtrip_and_checksum`: 손상 위치(바이트 30)가 LZ4 길이 확장 바이트에 해당해 체크섬이 아닌 구조 오류로 검출됨 → 실제 페이로드 바이트 손상으로 수정하고, 길이 바이트 손상도 별도로 거부되는지 확인 추가. (디코더 결함 아님)

### 2.3 `scripts/phase1/save_reload_check.py` (작성, 미실행)

Phase 0 `scripts/benchmark/servers/pumpkin_runner.py`와 `bench_manifest.toml`(시드·포트·바이너리 경로)을 재사용.

- N회 graceful start→ready→(선택) 콘솔 스크립트→idle→`stop` 사이클, 매 정지 후 월드 스냅샷.
- `--crash-cycle`: 첫 정상 사이클 이후 SIGKILL 사이클 삽입. 크래시 사이클은 구조 무결성만 게이트(미저장 진행분 유실은 허용), 다음 정상 사이클은 마지막 **정상** 스냅샷 대비 diff/conserve.
- 게이트: `verify` 오류, diff의 `chunk_lost/player_lost/level_dat`, conserve의 `item_duplication/item_loss/item_stack`. `diff --strict`는 기본 정보용(무인 서버도 로드된 스폰 청크를 랜덤 틱할 수 있음), `--strict-gate`로 게이트화.
- 바이너리 부재 시 `NOT_RUN`(exit 3)으로 기록 — PASS로 기록하지 않음.
- `--self-test`: 서버 없이 fixture로 파이프라인 확인 + 리전 절단 **음성 대조군**이 반드시 FAIL로 잡히는지 확인.
- 결과: `.work/phase1/<timestamp>/result.json`, 사이클별 서버 로그·스냅샷·검증 JSON.

미확인 가정: `PumpkinRunner.prepare()`는 월드가 `run_dir/world`에 생성된다고 가정(Phase 0 가정 승계), `pumpkin.toml.tmpl` 키(`[world.chunk] type`, `compression.algorithm = "LZ4"`)는 Phase 0 문서 기준이며 고정 커밋에서 미확인.

## 3. 생존 시나리오 체크리스트 (§7 Phase 1 작업 항목)

판정: `PASS / PARTIAL / FAIL / NOT TESTED`. 현재 **전 항목 NOT TESTED** (서버·클라이언트 미가동).

| # | 영역 | 시나리오 | 자동 판정 수단 | 상태 |
|---|---|---|---|---|
| S-01 | 접속 | Java 26.3 접속, whitelist 기본값 동작, 재접속 시 위치/인벤토리 유지 | `diff --strict`(재접속 사이 무조작) | NOT TESTED |
| S-02 | 이동 | 걷기/달리기/점프/수영/낙하 피해, 청크 경계 횡단 시 위치 동기화 | 봇 로그 + playerdata Pos | NOT TESTED |
| S-03 | 블록 | 채굴·설치·드롭 수집, 도구 내구도, 중력 블록 | `conserve`(드롭→인벤토리 보존) | NOT TESTED |
| S-04 | 제작 | 작업대/화로 레시피, 결과물 수량 | `conserve --allow <재료>` | NOT TESTED |
| S-05 | 인벤토리 | 상자/셜커/엔더상자 이동, shift-click, 드래그, 버리기/줍기 | `conserve --strict-loss` (중복·유실 0) | NOT TESTED |
| S-06 | 전투 | 근접/활, 방어구 감소, 몹 사망 드롭, 플레이어 사망·리스폰·드롭 회수 | `conserve`, playerdata Health | NOT TESTED |
| S-07 | 허기 | 소모·회복·굶주림 피해 | playerdata foodLevel | NOT TESTED |
| S-08 | 청크 | 신규 생성·언로드·재로드, 고속 이동 중 로딩 | `verify` + `diff`(chunk_lost 0) | NOT TESTED |
| S-09 | 네더 포털 | 점화, 오버월드↔네더 왕복, 포털 링크 좌표(1:8), 인벤토리 보존 | `diff`(Dimension), `conserve` | NOT TESTED |
| S-10 | 엔드 포털 | 엔드 진입, 귀환 포털, 리스폰 | `diff`, `conserve` | NOT TESTED |
| S-11 | 레드스톤/호퍼 | 기본 회로, 호퍼 체인 아이템 이동(중복 없음) | `conserve` | NOT TESTED |
| S-12 | 저장/재실행 | graceful stop 후 재기동 3회, 무결성·무변경 | `save_reload_check.py --cycles 3` | NOT TESTED |
| S-13 | 크래시 복구 | SIGKILL 후 재기동, 손상 0, 이전 저장분 유실 0 | `save_reload_check.py --crash-cycle` | NOT TESTED |
| S-14 | 연속 플레이 | 채집→네더→엔드 연속 진행 후 저장/재실행 | S-01~S-13 조합 | NOT TESTED |

S-02~S-11의 입력 구동은 Phase 0 봇 드라이버(`scripts/benchmark/drivers/bot_runner.py`: BotMark/SoulFire/Azalea) 또는 실클라이언트가 필요하며, 이번 단계에서는 시나리오별 봇 스크립트를 작성하지 않았다(드라이버 자체가 실측 미검증 상태이기 때문).

## 4. 실행 방법

```bash
# 검증기 빌드·테스트 (오프라인 가능, 이번 세션 실행 확인)
cargo test --offline --manifest-path tools/world-verify/Cargo.toml
cargo build --offline --release --manifest-path tools/world-verify/Cargo.toml

# 임의 월드 검사
tools/world-verify/target/release/world-verify verify  <world> --json verify.json
tools/world-verify/target/release/world-verify diff    <before> <after> --strict
tools/world-verify/target/release/world-verify conserve <before> <after> --strict-loss

# 하네스 (이번 세션 미실행)
python3 scripts/phase1/save_reload_check.py --self-test
python3 scripts/phase1/save_reload_check.py --cycles 3 --crash-cycle
```

## 5. 블로커

1. **네트워크 차단**: GitHub clone·crates.io 불가 → Pumpkin 소스/바이너리 없음 → Pumpkin 대상 모든 실측 불가(Phase 0과 동일 원인).
2. **도구 실행 권한**: 이번 세션에서 Python 스크립트 실행 및 외부 접근 명령이 승인되지 않음 → 하네스 미실행.
3. **클라이언트/봇 부재**: Java 26.3 클라이언트·봇 도구 미설치 → 상호작용 시나리오(S-02~S-11) 구동 불가.
4. **바닐라 26.3 참조 월드 부재**: DataVersion·청크 키 기준값을 얻을 출처 없음(추정값 사용 금지).

## 6. 네트워크 복구 후 수행 순서

1. `./scripts/build.sh` → 고정 커밋 clone·빌드, `.work/pumpkin-bin/pumpkin` 배치 (`docs/BUILD.md`).
2. Pumpkin 소스를 `upstream/` 포크로 준비(계획서 §11). 패치는 작은 단위로 분리.
3. `python3 scripts/phase1/save_reload_check.py --self-test` → `SELF_TEST_PASS` 확인(음성 대조군 탐지 포함).
4. `python3 scripts/phase1/save_reload_check.py --cycles 3 --crash-cycle` → 첫 실 Pumpkin 월드로 검증기 가정(레이아웃·키·압축) 확인. `verify` 오류가 Pumpkin 결함인지 검증기 가정 오류인지 분류해 기록.
5. 바닐라 26.3 서버로 같은 시드 월드를 만들어 DataVersion·청크 키 기준값 확보 → `--expect-data-version` 적용, Pumpkin 월드와 구조 비교.
6. **바닐라 미완성 메커니즘 목록화**: 고정 커밋에서 `crates/pumpkin*`의 `todo!()`/`unimplemented!()`/`TODO` 위치, 블록·엔티티·아이템 레지스트리의 행동(behavior) 미등록 항목, 업스트림 이슈 트래커를 대조하여 `docs/PHASE1-GAPS.md`에 근거(파일:라인·이슈 번호)와 함께 기록 → §3 체크리스트 우선순위에 반영.
7. 봇 드라이버로 S-02~S-11 스크립트 작성·실행 → `conserve --strict-loss`·`diff`를 각 단계 전후에 적용.
8. S-14 연속 플레이 + S-12/S-13 통과 시 Phase 1 완료 판정.
