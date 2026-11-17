# Rust 기반 초경량 Minecraft 서버 개발 계획서

> **문서 버전:** v1.1  
> **작성 기준일:** 2026-11-17  
> **기준 게임 버전:** Minecraft Java Edition 26.3 ("Wilderness Bound" 기준, 업그레이드 가능한 구조; Bedrock 1.26.x 추적)  
> **프로젝트 유형:** Pumpkin 기반 Rust 서버 포크 및 확장  
> **개발 원칙:** 호환성 검증 → 기능 구현 → 정확성 검증 → 프로파일링 → 최적화

## 1. 프로젝트 개요

### 1.1 목표

Rust 기반 Minecraft 서버인 **Pumpkin**을 포크하여, 기존 Java 서버의 운영 편의성을 가능한 한 유지하면서 CPU·RAM·MSPT를 절감하는 **실사용 가능한 초경량 서버 엔진**을 만든다.

핵심 목표:

1. **Java + Bedrock 크로스플레이:** 동일 월드·좌표·게임 규칙 아래 두 에디션이 함께 플레이.
2. **Paper/Bukkit 플러그인 지원:** PatchBukkit 기반의 검증된 플러그인 우선 호환(단, JVM 메모리·동기화 제약을 분리 관리).
3. **데이터팩 지원:** 일반 데이터팩부터 월드 생성형 데이터팩까지 단계적으로 확장.
4. **고품질 차원:** Terralith(오버월드), Incendium(네더) 우선; Nullscape(엔드)는 높이·Bedrock 제약 때문에 선택적 지원.
5. **Extreme Worldgen:** 선택형 초고도 오버월드 생성(기본 설계 Y=-512~511, 높이 1,024블록, 새 월드 전용·기본값 OFF).
6. **최적화:** 메모리, 청크 생성/저장, 월드 틱, 네트워크, 멀티코어 활용 개선.
7. **운영 안정성:** 자동 백업, 장애 복구, 성능 모니터링, 버전별 회귀 테스트.

### 1.2 범위 제외(초기 버전)

- 기존 Fabric/Forge 모드 `.jar` 직접 실행.
- 모든 Paper 플러그인의 100% 호환 보장.
- Bedrock 클라이언트의 물리적인 렌더링/차원 높이 제한 완전 제거(클라이언트·프로토콜 고유 한계).
- Nullscape(엔드 높이 384블록)를 Bedrock 호환이나 전체 출시 일정과 맞바꾸는 무제한 개발.
- 원본 데이터팩을 라이선스(Stardust Labs License 등) 확인 없이 프로젝트에 포함하여 재배포.

## 2. 기술 스택 및 아키텍처

| 계층 | 권장 기술 | 역할 및 주의사항 |
|---|---|---|
| 코어 | Pumpkin (Rust) 포크 | 2026년 8~9월 Beta 진입; Tokio(비동기 I/O) + Rayon(병렬 틱/월드생성) 기반 멀티스레드 코어. 실제 `crates/` 구성은 고정 커밋의 `Cargo.toml`에서 최종 확인 |
| 기본 확장 | Pumpkin WebAssembly 플러그인 | `Wasmtime` + `WIT` 기반 샌드박스(`.wasm`). Rust·Python·C#·Go·TypeScript 등 다언어 지원. 신규 기능은 가능하면 JVM 없이 구현 |
| Paper 호환 | PatchBukkit + 내장 JVM (Java 25+) | 2026년 10월 아키텍처 전환으로 **런타임에 Paperclip 기반 헤드리스(headless) Paper를 구동**하고 JNI/FFM + Protobuf 브리지로 실제 CraftBukkit/NMS 연결. **WIP(프로덕션 미성숙)** 상태이며 **전체 JVM + Paper 메모리를 점유**하므로 순수 Rust 모드와 혼동 금지 |
| Java 접속 | Pumpkin Java 프로토콜 | Java 26.3 지원 확인; 바닐라 메커니즘 1:1 동등성은 지속 검증 필요 |
| Bedrock 접속 | Pumpkin 내장 Bedrock 프로토콜 | **RakNet / NetherNet** 전송 계층을 코어에 내장하나 **실험적/WIP 상태**; 기능 동등성·고도 처리 별도 실측 필수 |
| Bedrock 대안 | 독립형 Geyser 프록시 | 내장 Bedrock 계층에서 오류·미구현이 발생할 때 비교·우회 경로로 시험 |
| 데이터팩 | Dynamic Registry Manager + 런타임 데이터팩 로더 + Worldgen 확장 | 바닐라 데이터(`assets/datapacks/<version>/`) 및 `Pumpkin-MC/Extractor` + `pumpkin-codegen` 파이프라인 활용. 커스텀 월드젠/데이터팩 완전 실행은 미검증 |
| 영속화 | `Anvil (.mca)` 명시 설정 우선 | **주의:** Pumpkin 기본 청크 포맷은 자체 `.pump` (`[world.chunk] type = "pump"`)이며 `anvil`, `linear`를 함께 지원함. Paper 호환 및 무손상 백업/롤백을 위해 `pumpkin.toml`에서 `type = "anvil"`을 명시적으로 우선 사용 |
| 관리 프로토콜 | MSMP (JSON-RPC) / 콘솔 | 최신 Pumpkin은 레거시 RCON 대신 MSMP(Minecraft Server Management Protocol)를 채택하므로 운영·모니터링 스크립트 설계 시 반영 |
| 테스트 | Rust 단위·통합 테스트 + Java/Bedrock 실접속 | Paper 26.3(Java 25 기준) 대조군과 공정한 조건으로 비교 |
| 배포 | Linux x86-64 우선, 이후 Windows | 재현 가능한 릴리스 빌드 |

### 2.1 개념 구조

```text
              [Java Edition 클라이언트]
                        |
              Java 프로토콜 계층
                        |
    [Bedrock 클라이언트]--+--[Pumpkin 내장 Bedrock (RakNet/NetherNet, WIP)]
                        |
                  [Rust 코어 (Tokio / Rayon)]
        +----------------+-------------------+
        |                |                   |
   [월드/청크]      [엔티티/틱]         [플레이어/명령/MSMP]
   (pump/anvil/linear)   |                   |
        +----------------+-------------------+
                        |
                [확장 인터페이스]
                  /          \
     [WASM 플러그인]        [PatchBukkit 브리지 (JNI/FFM + Protobuf)]
     (Wasmtime/WIT)                       |
                           [내장 JVM (Java 25+): 헤드리스 Paper + NMS]
                                          |
                                 [Paper/Bukkit 플러그인 (.jar)]

별도 대안 경로: [Bedrock 클라이언트] -> [외부 Geyser 프록시] -> [Pumpkin Java 접속 엔드포인트]
```

> **원칙:** 게임 시뮬레이션의 단일 진실 공급원(source of truth)은 Rust 코어다. PatchBukkit이 내부적으로 헤드리스 Paper를 구동하더라도 Paper의 자체 틱 루프와 네트워크 리스너는 비활성화된 상태를 유지하며, JVM 측에 두 번째 독립 월드 시뮬레이션을 만들지 않는다.

## 3. 기능 요구사항

### P0 — 반드시 필요한 핵심

- Java Edition 26.3 접속, 온라인 인증, 저장/불러오기, 재접속(26.3 기본 `white-list=true` 동작 반영).
- Bedrock Edition 접속, 블록 배치/파괴, 인벤토리, 이동, 차원 이동.
- 바닐라 생존 핵심 루프(채집·제작·전투·허기·네더/엔드 포털·기본 레드스톤/컨테이너) 및 플레이어 데이터 무결성.
- 데이터팩 목록 확인, 유효성 검사, 로딩 실패 원인 진단 로그.
- 서버 콘솔, 로그, MSMP/SIGTERM 안전 종료, `Anvil` 기반 백업/복원.
- TPS/MSPT/RSS·PSS/CPU 및 청크 생성 시간 측정.

### P1 — 운영에 필요한 호환성

- PatchBukkit 통합(Java 25+ 런타임 포함) 및 호환성 검증.
- 대표 플러그인 테스트:
  - PatchBukkit 공식 테스트 보고 목록(작성 기준 검증 대상 포함): `LuckPerms`, `EssentialsX`, `WorldEdit`, `WorldGuard`, `ProtocolLib`, `PacketEvents`, `GrimAC`, `Multiverse-Core`, `TAB`, `AuraSkills`, `VeinMiner`.
  - 추가 검증 필요(미확인): `PlaceholderAPI` 및 운영에 필요한 기타 `.jar` 플러그인.
- 핵심 데이터팩 실행 지원: 함수·태그·레시피·루트 테이블·발전과제(advancement triggers / loot predicates).
- 월드 생성 Registry: 바이옴, 차원 타입, 노이즈 설정, Density Function, Carver, Feature, Structure.
- Terralith/Incendium 실행 및 Java·Bedrock 비교.
- 크로스플레이 권한·명령·인벤토리·포털 동작 일치.

### P2 — 차별화 기능

- 관리자 설정형 Extreme Worldgen (`experimental_extreme_worldgen`).
- 비동기 청크 생성 작업 큐 및 플레이어 주변 청크 우선순위.
- 선택적 월드 사전 생성(pregeneration), 청크 캐시, 메모리 상한.
- 시드 재현성과 지형 생성 결정성(determinism) 검증.
- Nullscape(Java 우선) 또는 Bedrock 높이 제한(`0..256`)을 지키는 고품질 대체 엔드 데이터팩 호환.
- 관리용 진단 명령 및 경량 웹 대시보드(선택사항).

## 4. 월드 생성 정책

### 4.1 기본 모드: `vanilla_compatible`

- 목표: 원본 데이터팩의 차원 높이, 노이즈, 바이옴 및 구조물 정의를 존중.
- Terralith / Incendium / Nullscape 모두 Modrinth 기준 `26.1`~`26.3` 호환 데이터팩을 제공하므로, **모드 버전(예: Terralith 2.6.0+의 Lithostitched 의존)이 아닌 순수 데이터팩(.zip/폴더) 형식**을 기준으로 검증.
- Terralith/Incendium을 **먼저 기본 높이에서 테스트**하여 Worldgen Engine의 정확성을 확인.
- 신규 월드는 별도 테스트 시드로 생성. 기존 Paper 월드를 원본 상태에서 직접 변경하지 않음.
- 데이터팩이 여러 월드 생성 레지스트리를 덮어쓸 경우 로드 순서·우선순위·충돌을 명시적으로 기록하고 처리.

### 4.2 확장 모드: `extreme_worldgen` (기본값 OFF)

**초기 실험용 설정값(실제 엔진 TOML 키와 구별되는 개념 예시):**

```toml
[experimental_extreme_worldgen]
enabled = false
min_y = -512
height = 1024
reference_surface_y = -200
target_peak_y = 400
sea_level = -180
max_generation_threads = 4
```

- 설정된 블록 Y 범위: **-512 ~ 511**, 총 **1,024블록**(Geyser 문서 기준 오버월드 Bedrock 경계인 -512~512 범위와 정합).
- 기준 지면 Y=-200부터 산 정상 Y=400까지의 **고도차는 600블록**.
- 16블록 청크 섹션 기준: 24개(기본 오버월드 384블록) → 64개(확장 1,024블록).
- 고도차가 600블록인 모든 지형을 강제로 생성하지 않음. 산맥 분포와 동굴 깊이는 별도 매개변수로 설계.
- 단순 Y 좌표 확대만으로 Terralith와 완벽히 호환되지 않으므로 **별도 월드 생성 프리셋**으로 구현.
- 고도 범위 변경 시 레지스트리(`dimension_type`, `noise_settings`), 지형 노이즈, 대수층(aquifer), 광물 배치, 월드 표면, 구조물 Y 경계, 하이트맵, 조명(skylight/blocklight), 생성 범위, 직렬화 및 Java/Bedrock 클라이언트 동기화를 함께 검증.
- **새 월드에서만 활성화**하며, 운영 도중 높이 설정을 바꾸는 기능은 초기 범위에서 제외.

### 4.3 차원별 데이터팩 정책

| 차원 | 기본 높이/특성 | 1차 목표 | 2차 목표 | 실패 시 전략 |
|---|---|---|---|---|
| 오버월드 | Terralith 기본 구성(`min_y=-64`, `height=384`, Y `-64..319`) | Terralith 기본 높이 정확성 검증 | Extreme Worldgen 별도 프리셋(`-512..511`, 64섹션) | 일반 바닐라 지형 유지 |
| 네더 | Incendium(지형/논리 높이 `192`, `0..256` 범위 내 동작) | Incendium 기본 구성 검증(Bedrock `0..256` 한계 내 양립 가능) | 고도·포털(`logical_height=192`) 및 구조물 호환 확대 *(단 320/384 확장 애드온은 Bedrock 제외)* | 기본 네더 또는 높이 제한형 커스텀 팩 |
| 엔드 | Nullscape(`min_y=0`, `height=384`, `logical_height=384`) | 일반 엔드 안정성 확보 | Nullscape Java 전용/실험 분기 검토(Bedrock `0..256` 한계 초과) | 높이 `256` 이내 대체 데이터팩/커스텀 엔드 |

**중요:** Nullscape는 **선택 사항**이다. Nullscape가 요구하는 엔드 384블록 높이는 Bedrock의 비오버월드 `0..256` 한계를 초과하므로, Nullscape 때문에 전체 프로젝트의 안정화와 출시를 지연시키지 않는다.

## 5. Bedrock 호환성 및 높이 제한

### 5.1 지켜야 할 원칙

- Bedrock의 물리적 클라이언트 한계를 Rust 서버 코드만으로 없앨 수 있다고 가정하지 않는다.
- Geyser 공식 문서(`Current Limitations`) 기준, 프로토콜/클라이언트 수준에서 해결 불가능한 높이 한계는 다음과 같다:
  - **오버월드(및 오버월드로 매핑된 커스텀 차원):** **-512 미만 또는 512 초과 불가**(블록 좌표 `-512..511`, 일반 확장 기본은 `-64..320`).
  - **기타 차원(네더, 엔드 등):** **0 미만 또는 256 초과 불가**.
- Pumpkin 내장 Bedrock 계층(RakNet/NetherNet)이 위 고도 범위(특히 64섹션 `-512..511` 오버월드 및 `0..192`/`0..255` 네더·엔드)를 동일하게 처리하는지는 **아직 미검증이며 별도 실측이 필수**다. Geyser 문서의 한계를 그대로 Pumpkin의 보장으로 간주하지 않는다.
- Incendium(생성 높이 192)은 `0..256` 범위 안에 있으므로 원리상 Bedrock과 양립 가능하나, 포털 연결 높이(`logical_height`)와 천장 처리 실측이 필요하다. 반면 Nullscape(`0..383`, 높이 384)는 Bedrock 한계를 초과하므로 `Y>=256` 지형 미렌더링/디싱크 문제가 필연적이다.
- 불가능한 높이가 감지되면 서버 시작/월드 로드 시 **명확한 경고·해당 프리셋 제한·기능 비활성화**를 우선하고, 검증되지 않은 좌표 조작은 기본값으로 적용하지 않는다.

### 5.2 Bedrock 호환성 테스트 체크리스트

- 오버월드 `Y=-511, -200, 0, 319, 400, 510` 부근에서 청크 렌더링·블록 설치/파괴·액체/조명 확인.
- 네더 `Y=0, 128, 191, 255` 부근(Incendium 지형·구조물·포털) 및 엔드 `Y=0, 128, 255` 경계 동작 확인.
- Java/Bedrock 두 클라이언트의 동일 좌표 블록 상태 및 엔티티 위치 실시간 비교.
- 차원 전환, 리스폰, 멀티플레이어 동기화, 데이터팩 구조물 및 바이옴 효과 검증.
- 부가 효과(잔디색·안개·하늘색·날씨 등)는 게임플레이 호환성과 별개의 시각적 호환성 항목으로 추적.

## 6. 성능 최적화 전략

| 영역 | 최적화 후보 | 정확성 보호 장치 |
|---|---|---|
| 틱 처리 | Rayon 기반 안전한 병렬화, 배치 처리, 불필요한 반복 제거 | 레드스톤/엔티티 이벤트 순서 회귀 테스트 |
| 청크 생성 | 병렬 생성, 우선순위 큐, 캐시, 사전 생성 | 동일 시드 결과 비교, 경계 청크 검증 |
| 메모리 | 객체 할당 감소, 풀링, 팔레트 압축, 상한 관리 | 메모리 누수·장시간 soak 테스트·데이터 손상 검사 |
| 스토리지 | 압축/쓰기 배치, 비동기 I/O; 호환용 `anvil`(LZ4/ZLib)과 성능용 `.pump` 비교 평가 | 크래시 복구 및 저장 무결성 테스트 |
| 네트워크 | 패킷 배치, 관심 영역 최적화, 압축 비용 측정 | Java/Bedrock 패킷 동기화 및 지연 측정 |
| 플러그인 | WASM(Wasmtime/WIT) 우선, PatchBukkit(JVM) 선택형, FFI 호출 최소화 | 기존 플러그인 기능 검사 및 이벤트/GUI 격리 테스트 |

**측정 원칙:** 최적화는 가설이 아니라 프로파일링으로 결정한다. 성능 저하가 발생하면 되돌릴 수 있도록 기능별 플래그와 벤치마크 결과를 남긴다.

## 7. 개발 단계 및 완료 기준

### Phase 0 — 기준선 구축

**작업**
- Pumpkin 안정적인 커밋 SHA 고정, 포크 생성, Linux x86-64 빌드 자동화(`Cargo.toml` 실제 크레이트 멤버 확인).
- 동일 머신에서 Paper 26.3(Java 25+)과 Pumpkin 26.3 기본 구성 구동(`pumpkin.toml`에서 `[world.chunk] type = "anvil"` 명시 설정 및 MC 26.3 기본 whitelist 정책 확인).
- 기준 테스트 월드, 시드, 클라이언트 버전(Java 26.3 / Bedrock 1.26.x), 봇/실유저 부하 시나리오 고정.

**완료 조건:** Linux에서 재현 가능하게 빌드 성공, Java·Bedrock 기본 접속 확인, Paper 26.3 대비 기준 성능 로그 확보.

### Phase 1 — 기본 생존 엔진 안정화

**작업**
- 이동, 전투, 인벤토리, 블록, 청크, 네더/엔드 포털, 기본 레드스톤/호퍼, 월드 저장/재로딩 검증.
- Pumpkin Beta 단계에서 미완성된 바닐라 메커니즘(특정 몹 AI, 컨테이너·전투 예외 사례 등) 목록화 및 우선순위 보완.
- 크래시·청크 손상·중복 아이템·플레이어 데이터 유실 회귀 테스트 구축.

**완료 조건:** 핵심 생존 시나리오(채집→네더→엔드) 연속 플레이 및 `Anvil` 저장/재실행 테스트 무결성 통과.

### Phase 2 — 데이터팩 런타임

**작업**
- Pumpkin의 `assets/datapacks/`·Dynamic Registry Manager·런타임 데이터팩 로더 동작 범위 점검.
- 데이터팩 ZIP/폴더 로딩, 로드 우선순위, 오류 진단 로그 정비.
- 함수·태그·레시피·루트 테이블·발전과제 실행 테스트.
- Terralith/Incendium JSON 스키마를 파싱해 Pumpkin에 누락된 Density Function, Carver, Feature, Structure, Registry 항목 목록화 및 구현.

**완료 조건:** 대표 일반 데이터팩이 월드에서 정상 작동하고, Terralith/Incendium 로딩 시 파싱 실패 항목이 모두 식별·문서화됨.

### Phase 3 — Paper 플러그인 (PatchBukkit)

**작업**
- PatchBukkit 통합, 호스트 Java 25+ 런타임 및 헤드리스 Paper 구동 절차 검증.
- **메모리 분리 계측:** Rust 프로세스 RSS와 내장 JVM(힙/메타스페이스/오프힙) 포함 전체 PSS를 별도 기록.
- 플러그인별 명령, 이벤트, 권한, 인벤토리 GUI, 월드 조작, 엔티티 AI 동기화 검증.
- 호환성 표(`PASS / PARTIAL / FAIL / NOT TESTED`) 유지(우선 대상: `LuckPerms`, `EssentialsX`, `WorldEdit`, `WorldGuard`, `PlaceholderAPI` 등).

**완료 조건:** 선정한 필수 플러그인 테스트 통과, 재시작 후 상태 일관성 확보, 미지원 이벤트/GUI 한계 명시.

### Phase 4 — 고품질 데이터팩 적용

**작업**
- Terralith → Incendium 순서로 Vanilla 26.3 / Paper 26.3과 시드별 지형·바이옴 분포·구조물·지형 경계 비교.
- Incendium 네더 `logical_height=192` 설정에서 포털 연결·천장·몹 스폰 검사.
- Nullscape는 별도 실험 분기(`0..383`)로 분리 검토하고 Bedrock 제한에 따라 실패 시 대체 전략 적용.

**완료 조건:** Java 기준 Terralith·Incendium 주요 지형/구조물 검증 통과, 치명적 월드 생성 크래시·보이드 청크 오류 0건.

### Phase 5 — Extreme Worldgen

**작업**
- 1,024블록(`-512..511`, 64섹션) 지원에 필요한 청크 직렬화·레지스트리·하이트맵·조명(skylight/blocklight)·저장(`anvil`/`.pump`) 구조 수정.
- 고도차 600블록 산맥, 깊은 지하 구조, 대수층/광물/구조물 Y-범위 프리셋 개발.
- Vanilla 모드와 코드/설정으로 분리하고 개발 플래그(`enabled = false` 기본값)로 가드.

**완료 조건:** 새 월드 생성·저장·재시작 가능, Y=-511~510 구간 Java 렌더링·조명·블록 상호작용 검증 통과, 기존 월드 오작동 방지 확인.

### Phase 6 — Bedrock 완전성 검증

**작업**
- Pumpkin 내장 Bedrock(RakNet/NetherNet)과 외부 Geyser 프록시 방식 A/B 비교 매트릭스 실행.
- 오버월드(`-511..510`), 네더(Incendium `0..192`), 엔드(`0..255` vs Nullscape) 경계 및 포털·바이옴·인벤토리 동등성 테스트.
- 클라이언트 한계를 초과하는 프리셋/데이터팩 로드 시 자동 탐지·경고·차단 로직 구현.

**완료 조건:** 공식 지원으로 표시한 구성에서 주요 기능 정상 동작, 내장 Bedrock vs Geyser 차이 및 미지원 조합 문서화.

### Phase 7 — 성능·안정화·릴리스

**작업**
- TPS/MSPT(평균·p95·p99)/RSS·PSS/CPU/청크 생성 시간/디스크 I/O 계측.
- 플레이어 수, `view/simulation distance`, `anvil` vs `.pump`, 플러그인 모드(`native` vs `PatchBukkit`)별 부하 테스트.
- 장시간 soak test, 자동 백업/복원 리허설, SIGTERM/MSMP 정상 종료, 강제 종료(SIGKILL) 시 월드 복구 테스트.
- 릴리스 노트, 알려진 제한, 라이선스 고지, 재현 가능한 설치 문서 작성.

**완료 조건:** 치명적 데이터 유실 버그 0건, 지원 기능 회귀 테스트 통과, 성능 비교 결과를 재현 가능한 데이터로 제시.

## 8. 성능 목표 및 벤치마크 설계

### 8.1 목표 수치(달성 보장 아님 — 1차 도전 목표)

- **TPS:** 정상 부하에서 20 유지.
- **MSPT:** 동일 기능·설정·플레이어 부하 기준 Paper 26.3 대비 **30% 이상 감소**를 1차 도전 목표로 설정(실측으로 검증, 미달 시 프로파일링 후 재설계).
- **메모리:** 동일 조건의 **프로세스 전체 RSS/PSS 30% 이상 절감**을 1차 도전 목표로 설정(**순수 Rust / WASM 모드 한정**).
- **PatchBukkit 모드:** 헤드리스 Paper + JVM(Java 25+)을 내장하므로 **Rust RSS + JVM 힙/메타스페이스/네이티브 메모리를 합산한 전체 프로세스 트리 PSS/RSS를 별도 집계**하며, 순수 Rust 수치와 절대 혼용하지 않음.
- **월드 저장/재시작:** 데이터 손상 없이 동작.
- **지형 생성:** `vanilla_compatible`과 `extreme_worldgen` 결과를 분리 계측하고, `anvil` 포맷과 `.pump` 포맷 결과도 구분 기록.

### 8.2 공정한 비교 프로토콜

비교 프로파일 3종:
1. `paper-baseline`: Paper 26.3 (Java 25+, 동일 JVM 플래그/힙 상한) + 동일 기능 플러그인·데이터팩.
2. `pumpkin-native`: Pumpkin + WASM 또는 내장 기능만 (`anvil` 구성과 `.pump` 구성 각각 기록 가능).
3. `pumpkin-compat`: Pumpkin + PatchBukkit (Java 25+) + 동일 `.jar` 플러그인.

공통 통제 조건:
- **하드웨어/OS 통제:** 동일 머신, 고정 CPU 스레드/코어 핀닝, 동일 RAM 상한, 유휴 백그라운드 프로세스 최소화.
- **월드/설정 통제:** 동일 시드, 동일 `view-distance` / `simulation-distance`, 동일 엔티티 스폰/난이도 규칙.
- **시나리오 분리:** (A) **사전 생성(pregenerated) 월드 틱 부하 테스트**와 (B) **신규 청크 실시간 생성 부하 테스트**를 반드시 분리 실행.
- **워밍업 및 지표:** 초기 JIT/캐시 안정화를 위한 워밍업 구간 이후 고정 시간 동안 TPS, MSPT(평균·p50·p95·p99), 프로세스 트리 전체 RSS/PSS, CPU 사용률, 청크 생성 지연(ms/chunk), 디스크 I/O를 기록.

## 9. 운영·안전·라이선스

- 첫 개발 및 성능 테스트는 **운영 서버와 분리된 독립 환경**에서 진행.
- 기존 월드의 원본을 직접 덮어쓰지 않으며, 수정·포맷 변환 전에 전체 백업과 복원 리허설 확인.
- 사용 중인 플러그인/데이터팩에 대한 개별 호환성 문서(`PASS / PARTIAL / FAIL / NOT TESTED`) 유지.
- 서버 공개 시 온라인 인증, MC 26.3 기본 whitelist 정책 점검, 권한 분리, 방화벽, MSMP 접근 제어, 네트워크 입력 검증을 기본 적용.
- **라이선스 준수:**
  - **Pumpkin 서버 코어**는 **GPL-3.0** 라이선스이며, **`pumpkin-plugin-api` / `pumpkin-plugin-wit`** 은 **MIT OR Apache-2.0** 이중 라이선스이다. 포크 배포 시 GPL-3.0 소스 공개 의무를 사전에 검토하고 준수한다(PatchBukkit 저장소 라이선스 조건도 함께 확인).
  - **Terralith / Incendium / Nullscape**는 **Stardust Labs License**를 따르며(서버 사용 및 출처 표기 모드팩 포함 허용, **단독 파일 재배포/재업로드 엄격 금지**), 프로젝트 저장소에 무단 번들링하지 않고 사용자가 공식 배포처에서 직접 받도록 안내한다.
- 원본 프로젝트와 혼동되지 않도록 별도 프로젝트명·고지·저작권 표시 사용.

## 10. 핵심 리스크 및 대응

| 위험 | 영향 | 대응 전략 |
|---|---|---|
| Pumpkin Beta의 바닐라 파리티 공백(몹 AI·레드스톤 등) | 심각 | Phase 1 생존·메커니즘 체크리스트 선검증, 미구현 모듈 단위 보완 또는 업스트림 패치 반영 |
| 월드 생성 Registry / Density Function 구현 부족 | 심각 | Terralith/Incendium JSON 스키마 대조로 누락 항목 우선 구현 후 실제 시드 검증 |
| PatchBukkit 동기화 불일치(이벤트·GUI·AI) 및 JVM 메모리 증가 | 높음 | 플러그인별 `PASS/PARTIAL/FAIL` 관리; Rust+JVM 합산 메모리 별도 모니터링; 핵심 기능은 WASM 전환 검토 |
| Pumpkin 내장 Bedrock(WIP) 품질 불확실성 | 높음 | 외부 Geyser 프록시를 비교/대안 경로로 유지하고 A/B 테스트로 동등성 검증 |
| Bedrock 차원 높이 제한(네더/엔드 `0..256`, 오버월드 `-512..512`) | 높음 | Incendium(`0..192`) 우선 적용, Nullscape(`0..384`) 선택적 분리, 초과 프리셋 자동 경고·차단 |
| 청크 포맷 선택(`.pump` 기본값 vs `anvil`) 및 롤백 제약 | 중간 | Paper 호환/백업 경로에서는 `type = "anvil"` 명시 고정; `.pump` 사용 시 변환/복원 절차 사전 검증 |
| 초고도 월드(`1,024`블록)의 메모리/생성 비용 | 높음 | 기본값 OFF, 새 월드 전용 가드, 청크 우선순위·압축·사전 생성 도입 |
| 업스트림 Pumpkin 변경 및 MC 버전 업그레이드 | 중간 | 업스트림 커밋 SHA 고정, 작은 단위 패치 분리, 정기 리베이스 및 회귀 테스트 |
| 라이선스/원본 데이터팩 재배포 위반 | 높음 | GPL-3.0 및 Stardust Labs License 준수 체크리스트, 외부 다운로드 스크립트/가이드 분리 |

## 11. 개발 작업 구조(제안)

```text
project/
├── README.md
├── docs/
│   ├── PROJECT_PLAN.md
│   ├── ARCHITECTURE.md
│   ├── COMPATIBILITY_MATRIX.md
│   ├── WORLDGEN_SPEC.md
│   ├── BEDROCK_LIMITATIONS.md
│   ├── BENCHMARK_PROTOCOL.md
│   └── RELEASE_CHECKLIST.md
├── upstream/                 # Pumpkin 포크 (참고: Pumpkin 실제 워크스페이스는 crates/pumpkin*, tools/pumpkin-codegen, assets/datapacks/ 구조)
├── custom/                   # 프로젝트 확장 코드(실제 Cargo.toml workspace 멤버 및 패치 전략은 커밋 고정 후 확정)
│   ├── worldgen/
│   ├── bedrock_compat/
│   └── diagnostics/
├── tests/
│   ├── datapack_fixtures/
│   ├── plugin_matrix/
│   ├── client_e2e/
│   └── performance/
└── scripts/
    ├── build/
    ├── benchmark/
    └── backup_restore/
```

> 위 폴더는 **예시 개발 구조**이며, 실제 Pumpkin 저장소의 `Cargo.toml` 워크스페이스 멤버(`pumpkin`, `pumpkin-world`, `pumpkin-protocol`, `pumpkin-data`, `pumpkin-plugin-*` 등)와 통합 방식에 맞춰 조정한다.

## 12. MVP 완료 판정

다음 조건을 모두 만족하면 **MVP(첫 사용 가능 버전)** 로 인정한다.

- [ ] Java(26.3)와 Bedrock(1.26.x)에서 같은 월드에 정상 접속(내장 Bedrock 또는 검증된 Geyser 우회 경로 명시).
- [ ] 기본 생존 시스템 및 `Anvil` 저장/백업/복원 기능이 안정적으로 동작.
- [ ] 최소 한 개 이상의 실용적인 Paper 플러그인(PatchBukkit 또는 WASM 대체)과 기본 데이터팩 기능 검증 및 호환성 표 문서화.
- [ ] Terralith 또는 Incendium 중 하나 이상에서 실제 월드 생성 검증.
- [ ] Paper 26.3 대비 `pumpkin-native`(및 필요 시 `pumpkin-compat`) 성능 비교 결과를 통제 조건과 함께 재현 가능하게 기록.
- [ ] 치명적인 월드 손상·데이터 유실 문제가 없음.
- [ ] 지원되는 조합과 미지원 조합(라이선스·Bedrock 제한 포함)을 명확히 문서화.

**MVP 이후 확장 목표:** Terralith + Incendium 완성 → Extreme Worldgen → Nullscape/대체 엔드 → 추가 최적화.

## 13. 참고 자료

- Pumpkin 공식 저장소: https://github.com/Pumpkin-MC/Pumpkin
- Pumpkin 공식 사이트: https://pumpkinmc.org/
- Pumpkin 개발 문서: https://docs.pumpkinmc.org/
- Pumpkin 월드 엔진 문서: https://docs.pumpkinmc.org/developer/world
- Pumpkin 데이터팩 레지스트리 추적(작성 시점 웹 검색으로 현행 여부 미확인 — 저장소에서 최신 런타임 데이터팩 로더 이슈/PR과 함께 직접 확인 필요): https://github.com/Pumpkin-MC/Pumpkin/issues/731
- PatchBukkit 저장소: https://github.com/Pumpkin-MC/PatchBukkit
- PatchBukkit 기술 설명(Headless Paper & Real NMS, 2026-10): https://blog.pumpkinmc.org/2026-10-23-patchbukkit-nms-and-headless-paper/
- Geyser 현재 한계: https://geysermc.org/wiki/geyser/current-limitations/
- Terralith: https://modrinth.com/datapack/terralith
- Incendium: https://modrinth.com/datapack/incendium
- Nullscape: https://modrinth.com/datapack/nullscape

## 14. 개정 및 검증 이력 (Review Log)

### v1.1 (2026-11-17)

**검증 완료(사실 확인됨):**
- **Minecraft 26.3:** Mojang의 연도 기반 버전 체계 전환에 따라 `26.3`("Wilderness Bound") 및 Paper 26.3(Java 25 필요, 기본 `white-list=true`), Pumpkin의 26.3 지원 확인.
- **Pumpkin 생태계:** 2026년 8~9월 Beta 단계 진입, `Wasmtime`/`WIT` 기반 다언어 WebAssembly 플러그인 아키텍처, GPL-3.0(코어) / MIT OR Apache-2.0(`pumpkin-plugin-api`, `pumpkin-plugin-wit`) 라이선스 확인.
- **데이터팩 및 라이선스:** Terralith·Incendium·Nullscape의 `26.1~26.3` 지원 및 **Stardust Labs License**(단독 재배포 금지) 확인.
- **Geyser 높이 제한:** 오버월드(및 매핑 차원) `-512..512` 초과 불가, 기타 차원(네더/엔드) `0..256` 초과 불가 확인.

**수정 및 구체화(v1.0 → v1.1):**
- **PatchBukkit 구조 반영:** 단순 API 모킹이 아닌 **런타임 Paperclip 기반 헤드리스 Paper + Java 25+ 내장 JVM + JNI/FFM·Protobuf 브리지** 구조와 WIP 상태를 반영하고, 순수 Rust와 JVM 합산 메모리(PSS/RSS) 분리 계측 원칙을 강화. 테스트 보고 목록과 미검증 대상(`PlaceholderAPI`)을 구분.
- **영속화 기본 포맷 교정:** Pumpkin 기본 청크 포맷이 `.pump`임을 명시하고, Paper 호환·롤백을 위해 `pumpkin.toml`에서 `type = "anvil"` 명시 설정이 필요함을 반영.
- **차원별 높이 정합성 보완:** Incendium(`192` 높이, `0..256` 이내 → Bedrock 양립 가능)과 Nullscape(`384` 높이 → Bedrock `0..256` 한계 직접 충돌)의 기술적 차이를 구체화.
- **네트워크/관리 디테일 보완:** Pumpkin 내장 Bedrock(RakNet/NetherNet, WIP) 및 MSMP(JSON-RPC) 도입 반영.
- **실행 계획 구체화:** Phase 0~7의 작업 및 완료 조건, 벤치마크 통제 프로토콜(사전 생성 vs 실시간 생성 분리, p50/p95/p99 MSPT), 리스크 대응표를 실무 검증 가능 수준으로 구체화.

**미확인 / 실측 필요 항목(코드·실환경 확인 대기):**
- 고정할 Pumpkin 커밋에서의 정확한 `Cargo.toml` 크레이트 멤버 목록(`pumpkin-core` 분리 여부 포함) 및 `issues/731` 현행 상태.
- Pumpkin 내장 Bedrock 계층이 실제로 오버월드 `-512..511`(64섹션) 및 Incendium 네더(`0..192`)를 Geyser와 동일하게 처리하는지 여부.
- Terralith/Incendium의 모든 Density Function·Feature·Structure 레지스트리가 현재 Pumpkin 런타임 데이터팩 로더에서 어느 수준까지 파싱되는지.
- `PlaceholderAPI` 등 추가 Paper 플러그인의 PatchBukkit 실동작 여부 및 실제 MSPT/RAM 절감 수치.

---

**프로젝트 최종 정의:** Rust-first, Java/Bedrock cross-play, datapack-capable, Paper-compatible, optionally extreme-worldgen Minecraft server.
