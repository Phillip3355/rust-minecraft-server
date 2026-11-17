# Paper 26.3 vs Pumpkin 26.3 기준선 벤치마크 프로토콜

> **문서 상태:** Phase 0 설계 초안 (기준선 측정 및 Phase 7 릴리스 검증 겸용)  
> **상위 계획서:** [Minecraft-Rust-Server-Plan.md](../Minecraft-Rust-Server-Plan.md) (v1.2, 8절 및 Phase 0/3/4/5/7/12 기준)  
> **대상 버전:** Minecraft Java Edition 26.3 (`Paper 26.3` on Java 25+ vs `Pumpkin 26.3` 포크)  
> **핵심 원칙:** 호환성 검증 → 기능 구현 → 정확성 검증 → 프로파일링 → 최적화. **모든 성능 수치는 통제된 실측 결과만 기록하며 추정치나 조작된 수치를 사용하지 않는다.**

---

## 1. 목적 및 적용 범위

본 문서는 프로젝트의 **Phase 0(기준선 구축)** 에서 `Paper 26.3` 대비 `Pumpkin 26.3` 포크의 초기 성능을 공정하고 재현 가능하게 측정하기 위한 표준 벤치마크 프로토콜을 정의한다. 동일한 통제 구조와 스키마는 **Phase 3(PatchBukkit 메모리 분리 계측)**, **Phase 4/5(데이터팩·Extreme Worldgen 생성 계측)**, **Phase 7(성능·안정화·릴리스 검증)**, 그리고 **12절 MVP 완료 판정**까지 일관되게 적용한다.

### 준수 원칙
1. **비교 공정성:** 하드웨어, OS 자원 상한, 시드, 거리 설정, 플레이어 행동, 사전 생성 여부를 동일하게 통제한다.
2. **모드 분리 계측:** 순수 Rust/WASM(`pumpkin-native`)과 내장 JVM을 포함하는 PatchBukkit(`pumpkin-compat`)의 수치를 절대 혼용하지 않는다.
3. **시나리오 분리:** **사전 생성(pregenerated) 월드 틱 부하**와 **실시간 신규 청크 생성 부하**를 반드시 분리 실행한다.
4. **계측 오버헤드 격리:** 기본 성능 측정 런(경량 1Hz 외부 샘플링)과 심층 프로파일링 런(`Spark profiler`, `perf record`)을 별도 실행으로 분리한다.
5. **투명한 보고:** 목표치(예: MSPT/메모리 30% 절감)는 **1차 도전 목표**이며 달성 보장이 아니다. 미달 수치, 회귀, 미구현 기능, 테스트 제약은 누락 없이 기록한다.

---

## 2. 공정한 비교 구성 (Controlled Comparison Setup)

### 2.1 비교 프로파일 정의

| 프로파일 ID | 대상 구성 | 적용 Phase | 비고 |
|---|---|---|---|
| `paper-baseline` | Paper 26.3 + Java 25+ + 동일 기능 플러그인·데이터팩 | Phase 0 ~ 7 | 기준 대조군. JVM 플래그·힙 상한 고정 |
| `pumpkin-native-anvil` | Pumpkin 26.3 포크 (순수 Rust / WASM) + `[world.chunk] type = "anvil"` | Phase 0 ~ 7 | **Phase 0 1차 비교 대상.** Paper와 동일 `.mca` 포맷 사용 |
| `pumpkin-native-pump` | Pumpkin 26.3 포크 (순수 Rust / WASM) + `[world.chunk] type = "pump"` | Phase 0(선택) / Phase 7 | 스토리지 포맷 차이(`anvil` vs `.pump`) 분리 평가용 |
| `pumpkin-compat` | Pumpkin 26.3 포크 + PatchBukkit (헤드리스 Paper + Java 25+) + 동일 `.jar` 플러그인 | Phase 3, Phase 7 | **Rust RSS + JVM 전체(힙/메타스페이스/네이티브) 합산 PSS/RSS 별도 집계** |

> **참고:** Phase 0에서는 `paper-baseline`과 `pumpkin-native-anvil`(및 선택적으로 `pumpkin-native-pump`)을 우선 구축·측정한다. `pumpkin-compat`은 PatchBukkit이 통합되는 Phase 3부터 동일 프로토콜로 편입한다.

### 2.2 하드웨어 · OS · 리소스 통제

모든 비교 프로파일은 **동일한 물리 머신(또는 베어메탈에 준하는 고정 인스턴스)** 에서 순차 실행하며, 부하 생성기(봇 클라이언트)가 서버와 CPU/메모리 경합을 일으키지 않도록 격리한다.

| 통제 항목 | 고정 방법 |
|---|---|
| **CPU 코어·스레드 핀닝** | Linux `taskset` 또는 `systemd-run --scope -p AllowedCPUs=...` (cgroup v2 `cpuset.cpus`) 사용. 예: 코어 `0..7`은 서버 전용(8 논리 코어/4 물리 코어 등 머신 스펙에 맞춰 고정), 나머지 코어는 OS·부하 생성기(봇) 전용으로 분리. Pumpkin의 Tokio/Rayon 워커 스레드 수와 Paper의 워커 스레드 설정을 기록 |
| **RAM 상한 및 스왑** | 서버 프로세스 트리에 cgroup v2 `memory.max` (및 `memory.swap.max=0`) 적용. Paper/PatchBukkit의 JVM은 `-Xms`와 `-Xmx`를 동일 값으로 고정(예: `<SERVER_RAM_CAP_GB>` GB, cgroup 상한 내 여유분 포함)하여 동적 힙 확장 편차 제거 |
| **CPU 주파수/전력 정책** | 측정 전 `cpupower frequency-set -g performance` 확인(또는 현재 governor를 결과 메타데이터에 강제 기록). 터보 부스트/열 스로틀링 여부 모니터링 |
| **백그라운드 프로세스** | 불필요한 IDE, 컨테이너, 크론 작업, 자동 업데이트 중단. 유휴 시 CPU 사용률 `< 1%` 확인 |
| **디스크 및 캐시 정책** | 동일 NVMe/SSD 볼륨 사용. 측정 목적에 따라 **Warm Cache 모드**(워밍업 중 OS 페이지 캐시 안정화) 또는 **Cold Cache 모드**(런 시작 직전 `sync && echo 3 > /proc/sys/vm/drop_caches`) 중 하나를 택일해 메타데이터에 명시 |
| **빌드 최적화 고정** | Pumpkin은 `cargo build --release` 기준(특정 CPU 플래그 예: `RUSTFLAGS='-C target-cpu=native'` 사용 시 적용 여부를 반드시 명시하고 커밋 SHA 고정). Paper는 공식 릴리스 빌드 번호 및 Java 25+ 벤더/버전 고정 |

### 2.3 월드 · 서버 설정 통제표

두 서버의 설정이 어긋나면 틱/메모리 비교가 무효화되므로 아래 항목을 측정 매니페스트에 1:1 매핑하여 고정한다.

| 설정 항목 | Paper 26.3 (`server.properties` / `paper-*.yml`) | Pumpkin 26.3 (`pumpkin.toml` / 설정 파일) | 고정 규칙 |
|---|---|---|---|
| **게임 버전** | `26.3` | `26.3` | 서버·클라이언트·봇 프로토콜 버전 일치 |
| **청크 포맷** | `Anvil (.mca)` (기본값) | `[world.chunk] type = "anvil"` 명시 | 기본 비교는 `anvil` 고정; `.pump`는 별도 프로파일로 분리 |
| **시드 (`seed`)** | `<BENCH_SEED_*>` | `<BENCH_SEED_*>` | 64-bit 정수 동일 지정 (4.1절 참조) |
| **시야 거리 (`view-distance`)** | `10` (기본 기준) / `16` (확장 매트릭스) | 동일 값 (`10` / `16`) | Phase 0 기본값 `10` 고정, Phase 7에서 가변 비교 |
| **시뮬레이션 거리 (`simulation-distance`)** | `10` (기본 기준) / `8` (매트릭스) | 동일 값 (`10` / `8`) | 미지원 시 지원 범위 차이 명시 및 Paper 측 동등 조정 검토 |
| **난이도 / 게임모드** | `normal` / `survival` | `normal` / `survival` | 몹 스폰·허기·전투 조건 일치 |
| **스폰 보호 / 엔티티 규칙** | `spawn-protection=0`, 동일 `gamerule` | 동일 설정 | `doDaylightCycle`, `doWeatherCycle`, `randomTickSpeed` 등 고정 |
| **화이트리스트 / 인증 (26.3)** | 26.3 기본 `white-list=true` 확인; 로컬 격리 벤치마크 시 `online-mode=false` + 테스트 화이트리스트 또는 로컬 인증 정책 통일 | 동일 (`white-list` 및 인증 정책 일치) | 외부 인터넷 노출 금지(로컬 루프백/격리 네트워크 바인딩) |
| **관리 프로토콜** | 콘솔 파이프 / RCON(로컬 한정) | MSMP (JSON-RPC, `[networking.management]`) / 콘솔 파이프 | 제어 채널이 틱 루프에 부하를 주지 않도록 폴링 주기 제한 |

> **중요 (동등성 확인):** Pumpkin 26.3 Beta에서 특정 `gamerule`이나 몹 스폰 캡(`mob-cap`), 시뮬레이션 거리 처리가 Paper 26.3과 완전히 동일하지 않을 수 있다. Phase 0 실행 시 **실제 활성화된 청크 수와 로드된 엔티티 수**를 양쪽에서 함께 로그로 남겨 조건 불일치 여부를 검증한다.

### 2.4 사전 생성 청크 고정 및 실행 간 초기화 절차

**사전 생성 월드 테스트(S1, S2)** 와 **실시간 생성 테스트(S3)** 가 섞이지 않도록 아래의 **Golden Snapshot 복제 규칙**을 적용한다.

1. **Golden Snapshot 생성 (1회):**
   - 기준 시드(`<BENCH_SEED_VANILLA_OVERWORLD>`)로 **Paper 26.3 + Chunky**를 사용하여 지정 반경(예: 스폰 `0, 0` 기준 반경 `R = 2,048` 블록 = `128×128` 청크 이상, `view-distance` 여유분 포함)을 사전 생성한다.
   - 생성 완료 후 `/save-all` 및 정상 종료한 뒤, `world/` (`region/`, `entities/`, `poi/`, `level.dat`)를 **읽기 전용 Golden Snapshot** 으로 보관하고 디렉터리 파일 목록 및 체크섬(`sha256sum`)을 기록한다.
2. **Pumpkin 호환성 사전 점검:**
   - Phase 0 준비 단계에서 Golden Snapshot을 Pumpkin 26.3(`type = "anvil"`)으로 1회 로드·저장 검증하여 청크 포맷 해석 오류나 빈 청크 현상이 없는지 먼저 확인한다.
3. **매 측정 회차(Run) 실행 전 초기화:**
   - **S1/S2(사전 생성 시나리오):** Golden Snapshot을 임시 작업 디렉터리(`run_workdir/world/`)로 새로 복사한 뒤 서버를 기동한다. **이전 회차의 변경된 월드를 다음 회차에 재사용하지 않는다.**
   - **S3(실시간 생성 시나리오):** `world/` 디렉터리를 완전히 삭제(또는 시드와 `level.dat`만 남긴 빈 초기 상태)하고 매 회차 깨끗한 상태에서 시작하여 동일 경로를 탐험할 때의 순수 생성 비용을 측정한다.

---

## 3. 측정 지표 및 도구 선정

### 3.1 핵심 측정 지표

| 분류 | 지표 | 집계 단위 | 수집 주기/방법 | 비고 |
|---|---|---|---|---|
| **틱 안정성** | **TPS** | 평균, 최소, `< 20.0` 비율, `< 19.5` 비율 | 1초~5초 단위 서버 출력 / Spark | 정상 부하 목표 20.0 |
| **틱 지연** | **MSPT** (ms/tick) | 평균, **p50, p95, p99**, 최대, `> 50ms` 비율 | **매 틱(또는 롤링 윈도우) 원시 틱 타임 로그** | 50ms 초과 시 TPS 20 붕괴 시작 |
| **메모리** | **RSS / PSS** (MiB) | 워밍업 후 평균, 피크(Max), 종료 전 추세(누수) | OS `/proc/<pid>/smaps_rollup` 1Hz 샘플링 | **프로세스 트리 전체 합산** (`pumpkin-compat`은 Rust vs JVM 분리 표기) |
| **CPU** | **CPU 사용률** (%) | 전체 코어 대비 %, 할당 코어 대비 %, user/sys, context switch | `pidstat -u -w` 1Hz 샘플링 | 스레드별 병목 분석은 별도 `perf` 런 수행 |
| **청크 생성** | **청크 생성 시간** | `ms/chunk` (p50/p95/p99), `chunks/s` | 생성 구간 시작~완료 타임스탬프 / 서버 생성 로그 | S3 시나리오 전용 핵심 지표 |
| **디스크 I/O** | **스토리지 I/O** | Read/Write MB/s, IOPS, fsync 지연, 최종 월드 용량(MiB) | `pidstat -d` / `/proc/<pid>/io` / `du -sb` | `anvil` vs `.pump` 비교 필수 지표 |

### 3.2 측정 도구 선정 (Minecraft 26.3 기준)

단일 도구의 내부 보고 방식 차이에 의존하지 않기 위해 **OS 외부 공통 계측기**를 1차 진실 공급원(resource metrics)으로 삼고, 서버 내부 틱 지표를 병행한다.

#### (1) Paper 26.3 전용 도구
- **Spark (Paper 26.3 기본 내장):** 별도 JAR 설치 없이 사용 가능.
  - 경량 런: `/spark health --memory` (또는 콘솔 `/tps`, `/mspt` 출력 수집).
  - 별도 심층 프로파일링 런: `/spark profiler start --timeout 300` → 플레임그래프 저장(MSPT 급등 원인 메서드 분석용).
- **JVM 세부 계측 (`jcmd` / JFR):** `paper-baseline`과 `pumpkin-compat`(PatchBukkit)에서 JVM Heap 사용량, Metaspace, GC 일시정지(Pause time), Off-heap(DirectBuffer) 비중을 Rust 네이티브 RSS와 분리 기록하기 위해 사용.

#### (2) Pumpkin 26.3 전용 도구
- **내부 콘솔 / 텔레메트리 로그:** Pumpkin 서버가 출력하는 틱 타임/TPS/청크 생성 로그 수집.
- **MSMP (JSON-RPC):** Pumpkin의 관리 프로토콜(`[networking.management]`)을 통해 서버 상태 조회 및 정상 종료 제어(정확한 MSMP 메서드 스펙은 고정 커밋에서 확인).
- **Linux `perf` + `cargo-flamegraph` (심층 프로파일링 런 전용):** `RUSTFLAGS`에 debuginfo를 포함한 릴리스 바이너리로 `perf stat`(IPC, 캐시 미스, 분기 예측 실패) 및 `perf record -F 99 -g`를 별도 런에서 수행.

#### (3) 모든 프로파일 공통 외부 계측기 (1Hz 경량 샘플러)
- **메모리 (`RSS` / `PSS`):** `ps` / `/proc/<pid>/task/*/children`으로 서버 루트 PID 및 모든 자식 프로세스(특히 PatchBukkit의 헤드리스 JVM 하위 프로세스/스레드)를 추적하고 `/proc/<pid>/smaps_rollup`의 `Rss`, `Pss`, `SwapPss`를 합산. (`smem -P` 또는 Python 수집기 활용)
- **CPU 및 디스크 I/O:** `pidstat -u -r -d -w -p <PID_LIST> 1` 또는 `/proc/<pid>/stat`, `/proc/<pid>/io` 직접 파싱.
- **엔티티·청크 동등성 검증:** 각 시나리오 종료 직전 양쪽 서버의 로드된 청크 수, 활성 엔티티 수, 플레이어 좌표 도달률을 함께 기록하여 "적게 시뮬레이션해서 빠르게 보이는" 왜곡을 차단.

---

## 4. 테스트 시드 · 월드 · 부하 시나리오 목록

### 4.1 시드 및 월드 Placeholder 규약

벤치마크 실행 전 개발자(또는 자동화 파이프라인)는 아래 placeholder를 **고정 64-bit 정수 시드**로 확정하여 `bench_manifest.toml`에 기입해야 한다. (임의 랜덤 시드 실행 금지)

| Placeholder 키 | 용도 | 적용 Phase | 권장 고정 규약 |
|---|---|---|---|
| `<BENCH_SEED_VANILLA_OVERWORLD>` | 바닐라 오버월드 기준 시드 (`S1`, `S2`, `S3`) | Phase 0 ~ 7 | 평지·바다·산악·동굴·마을이 균형 포함된 고정 정수 1개 선정 후 영구 고정 |
| `<BENCH_SEED_TERRALITH>` | Terralith 26.x 데이터팩 오버월드 시드 (`S3-DP`) | Phase 4, 7 | 데이터팩 바이옴 다양성 확인 가능한 고정 정수 1개 |
| `<BENCH_SEED_INCENDIUM>` | Incendium 26.x 네더 시드 (`S3-DP`) | Phase 4, 7 | 높이 `0..192` 구조물/바이옴 검증용 고정 정수 1개 |
| `<BENCH_SEED_EXTREME_WG>` | Extreme Worldgen (`-512..511`) 전용 시드 (`S3-EXT`) | Phase 5, 7 | 지면 `-300`, 정상 `+500` 근처 산맥 생성 검증용 고정 정수 1개 |

> **라이선스 준수:** `Terralith` / `Incendium` 데이터팩 파일은 Stardust Labs License에 따라 저장소에 직접 커밋하지 않으며, 테스트 환경 외부 다운로드 스크립트/경로 참조로만 연결한다.

### 4.2 표준 부하 시나리오 (Phase 0 필수 3종 + Phase 7 확장 1종)

#### 시나리오 `S1`: 빈 월드 · 정적 유휴 기준선 (Idle Overhead)
- **목적:** 월드 시뮬레이션 부하가 거의 없을 때 엔진 자체의 기본 메모리 상주량(Baseline RSS/PSS), 유휴 CPU, 공회전 MSPT를 측정.
- **월드 구성:** `<BENCH_SEED_VANILLA_OVERWORLD>` 사전 생성 스냅샷 (`0, 0` 스폰).
- **부하 조건:**
  - `S1-A`: 플레이어 `0명` (스폰 청크 유지 상태).
  - `S1-B`: 플레이어 `5명` 접속 후 스폰 안전 지대 반경 16블록 내 정지(Idle Keep-alive + 기본 패킷 송수신).
- **구간:** 워밍업 `60초` → 본측정 `180초` (3분).

#### 시나리오 `S2`: 사전 생성 월드 바닐라 생존 · 이동 부하 (Pregenerated Tick Load)
- **목적:** **청크 생성 비용을 완전히 배제한 상태**에서 순수 틱 루프, 엔티티 AI/스폰, 블록/인벤토리 상호작용, 네트워크 패킷 브로드캐스트 성능(TPS/MSPT/PSS)을 측정.
- **월드 구성:** `<BENCH_SEED_VANILLA_OVERWORLD>` 사전 생성 스냅샷(스폰 기준 충분한 반경 사전 생성 완료, 매 런 복제 초기화).
- **플레이어 위치·행동 (결정적 스크립트):**
  - 부하 봇 `N`명을 스폰 중심 반경 `64~256` 블록 내 고정 앵커 좌표(예: 5개 분산 클러스터 또는 원형 트랙)에 배치.
  - **행동 패턴 (동일 틱/초당 빈도):**
    - 사전 생성 영역 내부의 고정 웨이포인트 순환 도보/질주 이동 (청크 경계 횡단 포함, 신규 미생성 청크 진입 금지).
    - 주기적 점프, 시선 회전, 인접 블록 파괴/재설치(또는 상자 열기/닫기 등 기본 생존 상호작용), 주기적 채팅/명령어 1회/10초.
    - 야간 또는 고정 스폰 룸에서 기본 적대적/중립적 몹 AI 활성화 확인(엔티티 수 로그 필수 기록).
- **부하 단계 (Phase 0 기본 / Phase 7 확장):**
  - Phase 0 기본: `N = 10`, `N = 25` (단일 머신 허용 한계 내 `N = 50` 추가).
  - Phase 7 확장: `view-distance = 10 vs 16`, `N = 10 / 25 / 50 / 100`.
- **구간:** 워밍업 `120초`(엔티티 스폰·JIT/캐시 안정화) → 본측정 `300초` (5분).

#### 시나리오 `S3`: 대규모 실시간 청크 생성 · 탐험 부하 (Live Chunk Generation Storm)
- **목적:** 미생성 청크의 실시간 지형 생성 속도(`ms/chunk`, `chunks/s`), 멀티스레드 확장성(Rayon vs Paper chunk workers), 메모리 스파이크 및 디스크 쓰기 I/O(`anvil` vs `.pump`)를 분리 측정.
- **월드 구성:** `<BENCH_SEED_VANILLA_OVERWORLD>` 초기 빈 월드(스폰 청크만 초기 로드, 매 런 `world/` 초기화).
- **플레이어 위치·행동:**
  - 부하 봇 `N`명(예: Phase 0 기본 `N = 8` 또는 `16`)이 스폰 `(0, y, 0)`에서 서로 다른 방위각(`360° / N` 등간격 방사형 직선 경로)으로 **미생성 영역을 향해 일정 속도**로 이동.
  - 이동 속도 밴드 분리(선택): `S3-Walk`(보통 속도, ~4.3~5.6 m/s) / `S3-Fast`(보트·겉날개 모사 고속 텔레포트/비행 스텝, ~20~30 m/s; 단 봇이 청크 로딩 완료를 대기하는지 타임아웃 없이 밀어붙이는지 제어 규칙을 양쪽 동일 적용).
- **구간:** 서버 기동 직후 스폰 청크 워밍업 `30초` → 본측정 `300초` (또는 총 생성 청크 `M`개 도달 시점까지 측정하여 **동일 청크 수 생성 소요 시간**과 **고정 시간 내 생성 청크 수**를 함께 산출).
- **후속 Phase 확장 슬롯 (구조 예약):**
  - `S3-DP` (Phase 4): `<BENCH_SEED_TERRALITH>` / `<BENCH_SEED_INCENDIUM>` 적용 상태 생성 벤치마크.
  - `S3-EXT` (Phase 5): `<BENCH_SEED_EXTREME_WG>` (`Y=-512..511`, 64섹션) 생성 벤치마크 (`vanilla_compatible`과 절대 혼합 비교 금지).

#### 시나리오 `S4`: 장시간 안정성(Soak) 및 정상/강제 종료 복구 (Phase 7 & 회귀용)
- **목적:** 메모리 누수(PSS 우상향), 장시간 GC/할당 파편화, `Anvil` 저장 무결성 및 크래시 복구 검증.
- **조건:** `S2` 중부하(`N = 25`)를 `4~24시간` 연속 실행 후 (1) MSMP/`SIGTERM` 정상 종료 및 재기동, (2) 쓰기 부하 중 `SIGKILL` 강제 종료 후 재기동 및 청크 손상/데이터 유실 `0건` 검사.

### 4.3 반복 횟수 및 통계 규칙
- 각 `(프로파일 × 시나리오)` 조합은 최소 **3회(Phase 0 최소 기준) ~ 5회(Phase 7 릴리스 기준)** 반복 실행한다.
- 보고 값은 **반복 회차별 중앙값(Median)과 범위(Min–Max 또는 IQR)** 를 함께 제시한다.
- 워밍업 구간 데이터는 본측정 통계에서 제외하되, 별도 타임라인 로그로 보관하여 초기 JIT/할당 스파이크를 확인할 수 있게 한다.
- 이상치(outlier)를 임의로 삭제하지 않으며, OS 스파이크 등 외부 원인이 확인된 경우에도 원시 데이터는 보존하고 주석으로만 표기한다.

---

## 5. 경량 벤치마크 하네스 자동화 설계

계획서 11절의 디렉터리 구조(`scripts/benchmark/`, `tests/performance/`)에 맞춰, 외부 의존성이 적은 경량 자동화 하네스를 다음과 같이 구성한다.

### 5.1 디렉터리 및 책임 구조 (제안)

```text
scripts/benchmark/
├── bench_manifest.toml       # 커밋 SHA, Java/Paper 빌드, 시드, 거리, 프로파일, CPU/RAM 핀닝 정의
├── run_benchmark.py          # 오케스트레이터 (단일/매트릭스 실행)
├── collectors/
│   ├── proc_sampler.py       # 1Hz OS 계측 (/proc/<pid>/smaps_rollup, stat, io + 자식 PID 트리 추적)
│   └── log_parser.py         # Paper(Spark/콘솔) & Pumpkin(콘솔/MSMP) 틱/청크 타임라인 파서
├── drivers/
│   └── bot_runner.py         # 결정적 봇 부하 드라이버 래퍼 (BotMark / SoulFire / Azalea 어댑터)
└── report_summary.py         # 원시 CSV/JSONL -> p50/p95/p99 요약 및 Markdown 비교표 생성

tests/performance/
├── configs/                  # 통제된 server.properties, paper-*.yml, pumpkin.toml 템플릿
└── snapshots/                # Golden Snapshot 경로 및 sha256sum 매니페스트 (Git 추적 제외, 메타만 추적)
```

### 5.2 단일 실행 라이프사이클 (Run Lifecycle)

```text
[1. 환경 검증]  CPU governor, 유휴 CPU < 1%, 스왑 상태, cgroup/taskset 권한 확인
      │
      ▼
[2. 월드 준비]  S1/S2: Golden Snapshot -> 임시 workdir 복제 (+ sha256 확인)
      │         S3:    빈 workdir 초기화 (+ 필요 시 page cache drop)
      ▼
[3. 서버 기동]  cgroup/taskset 격리 하에 Paper 또는 Pumpkin 기동 + PID 트리 등록
      │
      ▼
[4. 준비 대기]  로그("Done" / MSMP ready) 감지 후 안정화
      │
      ▼
[5. 봇 투입 & 워밍업]  봇 드라이버(별도 CPU 코어) 접속 및 스크립트 시작 -> Warmup 대기
      │
      ▼
[6. 본측정]     1Hz OS 샘플러(RSS/PSS/CPU/IO) + 서버 틱/청크 로그 동시 기록 (고정 시간 T)
      │
      ▼
[7. 정상 종료]  봇 해제 -> MSMP / stop / SIGTERM 정상 종료 대기 -> 최종 월드 디스크 크기 측정
      │
      ▼
[8. 아티팩트 저장]  results/<run_id>/ (manifest.json, samples.csv, ticks.jsonl, summary.json)
```

### 5.3 부하 생성 봇 도구 선정 가이드 (Phase 0 확인 필요)

| 후보 도구 | 특징 | 장점 | Phase 0 확인 필요 사항 |
|---|---|---|---|
| **Pumpkin-MC/BotMark** | Pumpkin 생태계 공식 Rust 스트레스 툴 | 경량, Java·Bedrock 동시 테스트 지원 | 26.3 프로토콜 호환 여부, 결정적 웨이포인트 이동/상호작용 스크립팅 가능 범위 |
| **Azalea** (`azalea-rs`) | Rust 기반 헤드리스 봇 프레임워크 | 물리/이동 정확도 높음, 커스텀 시나리오 코드화 용이 | 26.3 프로토콜 지원 커밋 확인 및 다수 봇 스웜 안정성 |
| **SoulFire** | Java 기반 대규모 봇/부하 테스트 툴 | 경로 탐색/채굴/상호작용 스크립트 풍부, CLI 지원 | 26.3 지원 여부 및 부하 생성기 자체 JVM 메모리/CPU 점유 분리 |

> **선정 원칙:** Phase 0에서는 **"단순 패킷 스팸"이 아니라 S2(이동+상호작용)와 S3(방사형 탐험)를 결정적으로 재현할 수 있는지**를 기준으로 1개 드라이버를 확정하고, 모든 비교 프로파일(`paper-baseline`, `pumpkin-native`, `pumpkin-compat`)에 **동일 드라이버·동일 스크립트**를 사용한다.

### 5.4 산출물 JSON 요약 스키마 (예시 필드)

각 실행의 `summary.json`은 재현성을 위해 아래 필드를 포함한다(수치 필드는 실측 전 생성 금지).

```json
{
  "run_id": "<timestamp>_<profile>_<scenario>_r1",
  "profile": "pumpkin-native-anvil",
  "scenario": "S2",
  "environment": {
    "git_commit_sha": "<PINNED_PUMPKIN_SHA>",
    "paper_build": "<PAPER_26_3_BUILD>",
    "java_version": "<JAVA_25_VERSION>",
    "kernel": "<UNAME_R>",
    "cpu_model": "<CPU_MODEL>",
    "pinned_cpus": "<SERVER_CORES>",
    "ram_cap_mib": "<RAM_CAP>",
    "chunk_format": "anvil",
    "seed": "<BENCH_SEED_VANILLA_OVERWORLD>",
    "view_distance": 10,
    "simulation_distance": 10
  },
  "metrics": {
    "tps": { "mean": null, "min": null, "pct_below_20": null },
    "mspt_ms": { "mean": null, "p50": null, "p95": null, "p99": null, "max": null, "pct_over_50ms": null },
    "memory_mib": {
      "total_tree_rss_mean": null,
      "total_tree_pss_mean": null,
      "total_tree_pss_peak": null,
      "rust_rss_mean": null,
      "jvm_pss_mean": null
    },
    "cpu_pct": { "process_tree_mean": null, "user_mean": null, "sys_mean": null },
    "chunk_gen": { "chunks_generated": null, "chunks_per_sec": null, "ms_per_chunk_p50": null, "ms_per_chunk_p95": null },
    "disk_io": { "read_mb_total": null, "write_mb_total": null, "world_size_mib": null }
  },
  "parity_checks": {
    "loaded_chunks_end": null,
    "active_entities_end": null,
    "bot_completion_rate": null,
    "errors_or_disconnects": null
  }
}
```

---

## 6. Phase 0 / Phase 7 / MVP 완료 판정 기준

계획서 7절(개발 단계) 및 8.1절(성능 목표), 12절(MVP 완료 판정)에 따른 수치·검증 기준을 다음과 같이 구분한다.

### 6.1 Phase 0 완료 기준 (기준선 구축)
- [ ] 고정된 Pumpkin 커밋 SHA의 Linux x86-64 재현 가능 빌드 성공 (`Cargo.toml` 실제 멤버 확정).
- [ ] 동일 하드웨어/설정 통제 하에 `paper-baseline` (Paper 26.3, Java 25+)과 `pumpkin-native-anvil` (Pumpkin 26.3, `type = "anvil"`)의 `S1`, `S2`, `S3` 기준선 측정 로그(각 최소 3회 반복) 확보.
- [ ] Java(26.3) 및 최신 Bedrock 기본 접속 확인.
- [ ] 측정 결과에 환경 핑거프린트, 미완성 바닐라 메커니즘 차이(예: 엔티티/청크 수 차이), 미확인 항목이 투명하게 기록됨.

### 6.2 Phase 7 완료 기준 (성능·안정화·릴리스)
- [ ] `paper-baseline`, `pumpkin-native-anvil`, `pumpkin-native-pump`, `pumpkin-compat`(PatchBukkit) 전 프로파일 매트릭스(`N`, `view/simulation distance`) 계측 완료(각 최소 5회 반복).
- [ ] 장시간 `S4` Soak 테스트 및 `SIGTERM`/MSMP 정상 종료, `SIGKILL` 강제 종료 복구 테스트에서 치명적 월드 손상/데이터 유실 `0건`.
- [ ] `vanilla_compatible`과 `extreme_worldgen` (`Y=-512..511`), 그리고 `anvil` vs `.pump` 결과가 분리 표기되어 재현 가능한 데이터로 공개.

### 6.3 MVP 완료 판정에서의 성능 판정 기준 (계획서 8.1절 & 12절)

MVP 판정은 **필수 통과 게이트(Hard Gate)** 와 **1차 도전 목표(Stretch Target)** 를 엄격히 구분한다.

| 구분 | 항목 | 기준 | 판정 규칙 |
|---|---|---|---|
| **필수 통과 게이트 (MVP Hard Gate)** | **재현 가능한 비교 데이터 확보** | `paper-baseline` 대비 `pumpkin-native` (및 적용 시 `pumpkin-compat`)의 통제 조건·원시/요약 지표 기록 | 미제출 시 MVP 미달. 모든 비교 조건과 한계 명시 필수 |
| **필수 통과 게이트 (MVP Hard Gate)** | **실사용 안정성 / 무결성** | 기본 생존·저장(`Anvil`)·백업/복원 중 치명적 월드 손상·데이터 유실 `0건`, 봇/테스트 중 비정상 크래시 없음 | 1건이라도 발생 시 MVP 보류 |
| **필수 통과 게이트 (MVP Hard Gate)** | **메모리 집계 정직성** | `pumpkin-compat` 사용 시 Rust RSS + JVM(힙/메타스페이스/네이티브) 합산 PSS/RSS 별도 집계 | 순수 Rust 수치와 JVM 모드 수치 혼용 금지 |
| **1차 도전 목표 (실측 검증 대상)** | **TPS** | 목표 부하 구간(S2 기준 플레이어 수)에서 `20.0` 유지 | 실측 결과로 기록. 20 미만 저하 지점(`N` 임계치)을 명시 |
| **1차 도전 목표 (실측 검증 대상)** | **MSPT 절감** | 동일 조건 `paper-baseline` 대비 `pumpkin-native` **30% 이상 감소** (`MSPT_pumpkin <= 0.70 * MSPT_paper`) | **달성 보장 아님.** 미달 시 수치를 수정하지 않고 `perf`/Spark 프로파일링으로 병목 원인 분석 및 재설계 계획 기록 |
| **1차 도전 목표 (실측 검증 대상)** | **메모리(RSS/PSS) 절감** | 동일 조건 `paper-baseline` 대비 `pumpkin-native` **전체 프로세스 트리 RSS/PSS 30% 이상 절감** (순수 Rust/WASM 한정) | **달성 보장 아님.** `pumpkin-compat`에는 30% 목표를 강제 혼용하지 않으며 별도 실측 표기 |

### 6.4 결과 보고 템플릿 (실측 전 placeholder 상태)

| 시나리오 | 프로파일 | TPS (Mean/Min) | MSPT p50 / p95 / p99 (ms) | Process Tree PSS Mean/Peak (MiB) | CPU Mean (%) | 청크 생성 (chunks/s) | Disk Write (MB) | Paper 대비 변화율 |
|---|---|---|---|---|---|---|---|---|
| `S1-A` (0명) | `paper-baseline` | `TBD` | `TBD` | `TBD` | `TBD` | N/A | `TBD` | 기준 (0%) |
| `S1-A` (0명) | `pumpkin-native-anvil` | `TBD` | `TBD` | `TBD` | `TBD` | N/A | `TBD` | `TBD` |
| `S2` (N=25) | `paper-baseline` | `TBD` | `TBD` | `TBD` | `TBD` | N/A | `TBD` | 기준 (0%) |
| `S2` (N=25) | `pumpkin-native-anvil` | `TBD` | `TBD` | `TBD` | `TBD` | N/A | `TBD` | `TBD` |
| `S3` (Live Gen) | `paper-baseline` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` | 기준 (0%) |
| `S3` (Live Gen) | `pumpkin-native-anvil` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` |
| `S3` (Live Gen) | `pumpkin-native-pump` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` | `TBD` |

---

## 7. 미확인 / 실측 필요 항목 (Phase 0 착수 시 확인 목록)

아래 항목은 현 시점(`v1.2` 계획서 기준, 코드 및 테스트 머신 미배치 상태)에서 확정되지 않았으며, Phase 0 착수 직후 실제 저장소와 테스트 환경에서 반드시 확인·기입해야 한다.

1. **고정 Pumpkin 커밋 SHA 및 크레이트 구조:** `Cargo.toml`의 실제 워크스페이스 멤버, release 빌드 플래그, `pumpkin-core` 등 구조 확인.
2. **Pumpkin 내부 틱/청크 메트릭 출력 형식:** Pumpkin 26.3이 콘솔 또는 MSMP(JSON-RPC)로 `매 틱 MSPT 원시값(p50/p95/p99 산출용)`과 `청크 생성 완료 타임스탬프`를 직접 노출하는지 여부(미지원 시 경량 진단 패치 또는 외부/내부 틱 히스토그램 로거 추가 필요).
3. **Paper vs Pumpkin MSPT 정의 일치 여부:** Paper(`/spark health`, `/mspt`)의 틱 시간 측정 범위(네트워크/비동기 청크 대기 포함 여부 등)와 Pumpkin 틱 루프 계측 범위의 차이 확인.
4. **부하 봇 도구 26.3 호환성:** `BotMark`, `Azalea`, `SoulFire` 중 어떤 도구가 Minecraft `26.3` 프로토콜 및 `S2`/`S3` 결정적 이동·상호작용 스크립트를 안정적으로 지원하는지 실환경 검증.
5. **Chunky 26.3 Golden Snapshot 호환성:** Paper 26.3 + Chunky(`1.5.66+` 등)로 생성한 `Anvil (.mca)` 스냅샷을 Pumpkin 26.3(`type = "anvil"`)이 무손실로 로드·인식하는지 검증.
6. **테스트 머신 하드웨어 및 JVM 기준선:** 실제 벤치마크를 수행할 Linux x86-64 머신의 CPU 모델/코어 수, RAM 용량, NVMe 스펙, 그리고 `Paper 26.3`에서 사용할 Java 25+ GC 플래그(G1GC vs Generational ZGC 등) 확정.
7. **Pumpkin `simulation-distance` / 몹 스폰 패리티:** Phase 0/1 단계에서 Pumpkin Beta의 실제 엔티티 스폰·AI 활성 범위가 Paper와 얼마나 차이나는지 계측하여 `S2` 부하 해석 시 보정 주석으로 반영.
