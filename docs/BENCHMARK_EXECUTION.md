# Phase 0 벤치마크 실행 가이드 및 도구 조사 결과

> **상위 프로토콜:** [BENCHMARK_PROTOCOL.md](BENCHMARK_PROTOCOL.md)  
> **조사·작성일:** 2026-11-17  
> **대상:** `Paper 26.3` (Java 25+) vs `Pumpkin 0.2.2+26.3-26.60` (`eda69440e6369c5f8a05c3f69fc809c40dab93a6`)

---

## 1. 하네스 구성 파일

| 경로 | 역할 |
|---|---|
| [`scripts/benchmark/bench_manifest.toml`](../scripts/benchmark/bench_manifest.toml) | 고정 SHA/버전, 64-bit 시드, CPU/RAM 핀닝, 프로파일, `S1-A`~`S4` 시나리오 매개변수 |
| [`scripts/benchmark/fetch_artifacts.sh`](../scripts/benchmark/fetch_artifacts.sh) | Paper 26.3(Fill v3), Chunky(Modrinth), BotMark, SoulFireCLI 다운로드/빌드 (`--proxy`, `--dry-run` 지원) |
| [`scripts/benchmark/run_benchmark.py`](../scripts/benchmark/run_benchmark.py) | 8단계 라이프사이클 오케스트레이터 (`--preflight`, `--dry-run`, `--matrix`, S4 `SIGKILL` 복구) |
| [`scripts/benchmark/servers/paper_runner.py`](../scripts/benchmark/servers/paper_runner.py) | Paper 26.3 전용 실행/설정 렌더링 (`standalone` 및 `velocity` 프록시 모드 구분, Java 25+ 검사) |
| [`scripts/benchmark/servers/pumpkin_runner.py`](../scripts/benchmark/servers/pumpkin_runner.py) | Pumpkin 26.3 전용 실행/설정 렌더링 (`pumpkin.toml`, `anvil`/`pump`, `use_tty = false`) |
| [`scripts/benchmark/drivers/bot_runner.py`](../scripts/benchmark/drivers/bot_runner.py) | 부하 드라이버 래퍼 (`soulfire` / `botmark` / `azalea`, `bot_cpus` 코어 격리) |
| [`scripts/benchmark/collectors/proc_sampler.py`](../scripts/benchmark/collectors/proc_sampler.py) | 1Hz `/proc` 프로세스 트리 계측기 (RSS/PSS/SwapPss/CPU/IO, Rust vs JVM 분리, 선택적 `pidstat`) |
| [`scripts/benchmark/collectors/log_parser.py`](../scripts/benchmark/collectors/log_parser.py) | Paper(`/tps`, `/mspt`, Spark) & Pumpkin(`tps`, `tick query`) 로그 파서 (`ticks.jsonl`) |
| [`scripts/benchmark/report_summary.py`](../scripts/benchmark/report_summary.py) | 단일 런 `summary.json` 생성 및 반복 회차 중앙값·범위 Markdown 표 생성 |
| [`tests/performance/configs/`](../tests/performance/configs/) | `server.properties`, `paper-global.yml`, `paper-world-defaults.yml`, `pumpkin.toml` 템플릿 |
| [`tests/performance/snapshots/README.md`](../tests/performance/snapshots/README.md) | Golden Snapshot 생성 및 `sha256sum` 검증 절차 |
| [`tests/performance/test_harness.py`](../tests/performance/test_harness.py) | 하네스 단위/로컬 통합 테스트 (`unittest`) |

---

## 2. 최신 도구 조사 결과 (Minecraft 26.3 기준)

### 2.1 부하 드라이버 비교 및 선정 (`BotMark` vs `Azalea` vs `SoulFire`)

| 도구 | 26.3 지원 상태 (조사 결과) | 제어·스크립팅 기능 | Phase 0 채택 역할 |
|---|---|---|---|
| **SoulFire** (`soulfiremc-com/SoulFire`) | **네이티브 MC 26.3 지원** (`--bot-protocol-version 26.3`) | 헤드리스 `SoulFireCLI.jar`, `@soulfiremc/sdk`(TS/Python), `.sfscript`, Fabric/Java 플러그인으로 결정적 경로/상호작용 스크립팅 가능 | **`S2`(이동·상호작용), `S3`(방사형 탐험), `S4`(Soak) 주력 드라이버** |
| **BotMark** (`Pumpkin-MC/BotMark` v0.1.1) | `pumpkin-protocol`/`pumpkin-data` 직접 의존(`[patch]`로 로컬 Pumpkin 크레이트 연결 가능) → **Pumpkin 26.3과 일치** | `--ip`, `--count`, `--delay`, `--bedrock`, `--enable_movement`, `--enable_jumping`, `--enable_spam_message` (랜덤 이동·점프·채팅 위주, S3 방사형 궤적 미지원) | **`S1-B`(정지 접속) 및 경량 Java/Bedrock 접속 스트레스용 보조 드라이버** |
| **Azalea** (`azalea-rs/azalea`) | `azalea-protocol` `26.3` 개발 브랜치 관리 (+ `azalea-viaversion` 보조) | Rust 기반 물리·경로탐색 라이브러리 (커스텀 바이너리 컴파일 필요) | **커스텀 Rust 시나리오 확장 슬롯 (`azalea` 어댑터 유지)** |

> **주의:** `SoulFireCLI.jar`는 JVM을 사용하므로 반드시 `scripts/benchmark/bench_manifest.toml`의 `bot_cpus`(`4-7`)로 격리해 `server_cpus`(`0-3`)와 경합하지 않도록 실행한다.

### 2.2 Paper 26.3 다운로드 및 실행 규약

- **다운로드 API 변경:** 기존 `api.papermc.io/v2`가 종료되고 **PaperMC Fill v3 API** (`https://fill.papermc.io/v3/projects/paper/versions/26.3/builds/latest`)를 사용한다. 비범용 `User-Agent` 헤더가 필수이며, `.downloads["server:default"].checksums.sha256`로 무결성을 검증한다.
- **런타임 요구:** **Java 25 이상 필수**. `-Xms == -Xmx` 고정 힙 및 `-XX:+UseZGC`를 기본으로 사용하며 `--nogui`로 기동한다.
- **프록시 구성 분리:**
  - `proxy_mode = "standalone"`: 로컬 격리 벤치마크 기본값(`online-mode=false`, `proxies.velocity.enabled: false`).
  - `proxy_mode = "velocity"`: 프록시 경유 테스트 시 `paper-global.yml`의 `proxies.velocity.enabled: true` 및 시크릿을 자동 반영한다.
  - 네트워크 프록시 환경에서의 다운로드 시 `./scripts/benchmark/fetch_artifacts.sh --proxy <URL>` 또는 `HTTPS_PROXY`를 지원한다.

### 2.3 Pumpkin `0.2.2+26.3-26.60` 실행 규약

- **통합 설정 파일:** `0.2.2` 기준 `configuration.toml`/`features.toml` 분리안 대신 단일 **`pumpkin.toml`**을 사용한다.
- **핵심 제어 설정:**
  - `[world.chunk] type = "anvil"` (Paper 비교 기준선) / `"pump"` (포맷 분리 프로파일).
  - `[commands] use_console = true`, `use_tty = false` (`rustyline` TTY 비활성화로 파이프 제어 안정화).
  - `[networking.management] enabled = true` (MSMP JSON-RPC 포트 `25585`).
- **내부 틱 계측:** `tps` 및 `tick query` 콘솔 명령을 주기적으로 전송하여 TPS, MSPT, 백분위(P50/P95/P99)를 수집한다.

---

## 3. 온라인 Linux x86-64 호스트에서의 실행 절차

```bash
# 1. Pumpkin 기준선 바이너리 빌드 (별도 build.sh 절차)
./scripts/build.sh
mkdir -p .work/pumpkin-bin
ln -sf ../.work/build-<ID>/target/x86_64-unknown-linux-gnu/release/pumpkin .work/pumpkin-bin/pumpkin

# 2. 외부 아티팩트 준비 (Java 25+ 설치된 머신에서 실행)
./scripts/benchmark/fetch_artifacts.sh --all

# 3. 사전 점검 (CPU governor, 유휴 CPU, 바이너리, 스냅샷 확인)
./scripts/benchmark/run_benchmark.py --preflight

# 4. Golden Snapshot 1회 생성 (tests/performance/snapshots/README.md 참조)
# 5. 드라이런 확인 후 실제 Phase 0 매트릭스 실행
./scripts/benchmark/run_benchmark.py --matrix --dry-run
./scripts/benchmark/run_benchmark.py --matrix
```

---

## 4. 검증 결과 및 미실행/미검증 항목 (2026-11-17)

### 4.1 현재 환경에서 실제 검증한 항목

- `bash -n scripts/benchmark/fetch_artifacts.sh`: 통과 (exit 0).
- `python3 -m py_compile ...` (모든 Python 스크립트 및 테스트): 통과 (exit 0).
- `python3 -m unittest -v tests/performance/test_harness.py`: **7/7 통과**.
  - `bench_manifest.toml` 시나리오/프로파일/시드 로딩 검증.
  - Paper(`standalone`/`velocity` 프록시) 및 Pumpkin(`anvil` `pumpkin.toml`) 템플릿 렌더링 검증.
  - `ProcSampler`가 실제 부모-자식 Python 서브프로세스 트리를 4Hz로 추적하여 `/proc/<pid>/smaps_rollup` (`Rss`/`Pss`) 및 `pid_count >= 2`를 CSV로 기록하는 동작 검증.
  - `log_parser.py`의 Paper(`/tps`, `/mspt`) 및 Pumpkin(`tps`, `mspt`, `p50/p95/p99`, `Generated chunk`) 파싱 검증.
  - `report_summary.py`의 `summary.json` 스키마 및 미수집 지표 `null`/`TBD` 보존 검증.
  - `bot_runner.py` `S3` 방사형 계획 생성 및 Golden Snapshot `sha256sum` 검증 로직 통과.
- `./scripts/benchmark/fetch_artifacts.sh --all --dry-run`: 통과.
- `./scripts/benchmark/run_benchmark.py --preflight`: 통과 (누락 의존성을 정직하게 보고).
- `./scripts/benchmark/run_benchmark.py --matrix --dry-run`: 통과.
- `./scripts/benchmark/run_benchmark.py --profile paper-baseline --scenario S1-A`: preflight 누락으로 **예상대로 실행 차단(RuntimeError)** 확인 (가짜 결과 생성 안 함).

### 4.2 미실행 / 미검증 항목과 사유

| 항목 | 상태 | 사유 |
|---|---|---|
| `Pumpkin 0.2.2+26.3-26.60` 컴파일 및 서버 기동 | **미실행 / 미검증** | 작업 환경 네트워크/DNS 차단으로 GitHub/crates.io 클론·빌드 불가 (`docs/BUILD-VALIDATION.md` 참조) |
| `Paper 26.3` 및 `Chunky` 다운로드/기동 | **미실행 / 미검증** | 네트워크 차단 + 로컬 머신에 `java` (Java 25+) 미설치 |
| `SoulFireCLI.jar` / `BotMark` / `Azalea` 실제 접속·부하 테스트 | **미실행 / 미검증** | 네트워크 차단 + Java 25+ 미설치 + 대상 서버 미기동 |
| `pidstat` 사이드카 연동 | **미실행 / 미검증** | 로컬 머신에 `sysstat`(`pidstat`) 미설치 (단, `/proc` 직접 수집기는 실제 서브프로세스로 검증 완료) |
| Golden Snapshot (`vanilla_overworld_r2048`) 생성 및 Pumpkin Anvil 호환 로드 | **미실행 / 미검증** | Paper 26.3 / Chunky / Pumpkin 바이너리 부재 |
| `S1`~`S4` 실측 TPS / MSPT / RSS·PSS / 청크 생성 수치 | **미실행 / 미검증** | 실제 서버 미기동 (프로토콜 원칙에 따라 placeholder `TBD` / `null` 유지) |
