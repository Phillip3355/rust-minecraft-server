# Rust 기반 초경량 Minecraft 서버 개발 계획서

> **문서 버전:** v1.0  
> **작성 기준일:** 2026-11-17  
> **기준 게임 버전:** Minecraft Java Edition 26.3 (최신 정식 버전 기준, 업그레이드 가능한 구조)  
> **프로젝트 유형:** Pumpkin 기반 Rust 서버 포크 및 확장  
> **개발 원칙:** 호환성 검증 → 기능 구현 → 정확성 검증 → 프로파일링 → 최적화

## 1. 프로젝트 개요

### 1.1 목표

Rust 기반 Minecraft 서버인 **Pumpkin**을 포크하여, 기존 Java 서버의 운영 편의성을 가능한 한 유지하면서 CPU·RAM·MSPT를 절감하는 **실사용 가능한 초경량 서버 엔진**을 만든다.

핵심 목표:

1. **Java + Bedrock 크로스플레이:** 동일 월드·좌표·게임 규칙 아래 두 에디션이 함께 플레이.
2. **Paper/Bukkit 플러그인 지원:** PatchBukkit 기반의 검증된 플러그인 우선 호환.
3. **데이터팩 지원:** 일반 데이터팩부터 월드 생성형 데이터팩까지 단계적으로 확장.
4. **고품질 차원:** Terralith(오버월드), Incendium(네더) 우선; Nullscape(엔드)는 가능할 때 지원.
5. **Extreme Worldgen:** 선택형 초고도 오버월드 생성(기본 설계 Y=-512~511, 높이 1,024블록).
6. **최적화:** 메모리, 청크 생성/저장, 월드 틱, 네트워크, 멀티코어 활용 개선.
7. **운영 안정성:** 자동 백업, 장애 복구, 성능 모니터링, 버전별 회귀 테스트.

### 1.2 범위 제외(초기 버전)

- 기존 Fabric/Forge 모드 .jar 직접 실행.
- 모든 Paper 플러그인의 100% 호환 보장.
- Bedrock 클라이언트의 물리적인 렌더링/차원 높이 제한 완전 제거.
- Nullscape를 원형 그대로 반드시 지원하기 위한 무제한 개발.
- 원본 데이터팩을 라이선스 확인 없이 프로젝트에 포함하여 재배포.

## 2. 기술 스택 및 아키텍처

| 계층 | 권장 기술 | 역할 및 주의사항 |
|---|---|---|
| 코어 | Pumpkin (Rust) 포크 | 월드, 게임 틱, 청크, 엔티티, 네트워크 처리 |
| 기본 확장 | Pumpkin WebAssembly 플러그인 | 신규 기능은 가능하면 JVM 없이 실행 |
| Paper 호환 | PatchBukkit + JVM | Paper/Bukkit .jar 실행; 초기 개발 단계, 추가 메모리 사용 |
| Java 접속 | Pumpkin Java 프로토콜 | 최신 정식 버전 호환 확인 |
| Bedrock 접속 | Pumpkin 내장 Bedrock 프로토콜 | 기본 선택지; 기능 동등성 별도 테스트 |
| Bedrock 대안 | 독립형 Geyser 프록시 | 내장 방식에서 오류가 있을 때 비교·우회 경로로 시험 |
| 데이터팩 | Pumpkin 데이터팩 로더 + 추가 Registry/Worldgen 구현 | 모든 데이터팩의 실행을 전제하지 않음 |
| 영속화 | 기존 Anvil(.mca) 우선 | 포맷 변환 및 손상 없이 백업/복원 가능한 운영 경로 |
| 테스트 | Rust 단위·통합 테스트 + Java/Bedrock 실접속 | Paper 기준선과 공정한 비교 |
| 배포 | Linux x86-64 우선, 이후 Windows | 재현 가능한 릴리스 빌드 |

### 2.1 개념 구조

              [Java Edition 클라이언트]
                        |
              Java 프로토콜 계층
                        |
    [Bedrock 클라이언트]--+--[Pumpkin 내장 Bedrock 계층]
                        |
                  [Rust 코어]
        +----------------+-------------------+
        |                |                   |
   [월드/청크]      [엔티티/틱]         [플레이어/명령]
        |                |                   |
        +----------------+-------------------+
                        |
                [확장 인터페이스]
                  /          \
          [WASM 플러그인]   [PatchBukkit/JVM]
                          |
                  [Paper 플러그인]

별도 옵션: [외부 Geyser 프록시] -> [Pumpkin Java 접속 엔드포인트]

> **원칙:** 게임 시뮬레이션의 단일 진실 공급원(source of truth)은 Rust 코어다. JVM/플러그인 측에 두 번째 독립 월드 시뮬레이션을 만들지 않는다.

## 3. 기능 요구사항

### P0 — 반드시 필요한 핵심

- Java Edition 최신 정식 버전 접속, 인증, 저장/불러오기, 재접속.
- Bedrock Edition 접속, 블록 배치/파괴, 인벤토리, 이동, 차원 이동.
- 바닐라 생존 기본 기능 및 플레이어 데이터 무결성.
- 데이터팩 목록 확인, 유효성 검사, 로딩 실패 진단.
- 서버 콘솔, 로그, 안전 종료, 백업/복원.
- TPS/MSPT/RAM/CPU 및 청크 생성 시간 측정.

### P1 — 운영에 필요한 호환성

- PatchBukkit 통합 및 호환성 검증.
- 대표 플러그인 테스트: LuckPerms, EssentialsX, WorldEdit, PlaceholderAPI 등(성공 여부는 실측).
- 핵심 데이터팩 실행 지원: 함수·태그·레시피·루트 테이블·발전과제.
- 월드 생성 Registry: 바이옴, 차원 타입, 노이즈 설정, Density Function, Feature, Structure.
- Terralith/Incendium 실행 및 Java·Bedrock 비교.
- 크로스플레이 권한·명령·인벤토리·포털 동작 일치.

### P2 — 차별화 기능

- 관리자 설정형 Extreme Worldgen.
- 비동기 청크 생성 작업 큐 및 플레이어 주변 청크 우선순위.
- 선택적 월드 사전 생성(pregeneration), 청크 캐시, 메모리 한도.
- 시드 재현성과 지형 생성 결정성 검증.
- Nullscape 또는 다른 고품질 엔드 데이터팩 호환.
- 관리용 진단 명령 및 경량 웹 대시보드(선택사항).

## 4. 월드 생성 정책

### 4.1 기본 모드: `vanilla_compatible`

- 목표: 원본 데이터팩의 차원 높이, 노이즈, 바이옴 및 구조물 정의를 존중.
- Terralith/Incendium을 **먼저 기본 높이에서 테스트**하여 Worldgen Engine의 정확성을 확인.
- 신규 월드는 별도 테스트 시드로 생성. 기존 Paper 월드를 원본 상태에서 직접 변경하지 않음.
- 데이터팩이 여러 월드 생성 레지스트리를 덮어쓸 경우 우선순위와 충돌을 명시적으로 처리.

### 4.2 확장 모드: `extreme_worldgen` (기본값 OFF)

**초기 실험용 설정값(실제 엔진 TOML 키와 구별되는 개념 예시):**

[experimental_extreme_worldgen]
enabled = false
min_y = -512
height = 1024
reference_surface_y = -200
target_peak_y = 400
sea_level = -180
max_generation_threads = 4

- 설정된 블록 Y 범위: **-512 ~ 511**, 총 **1,024블록**.
- 기준 지면 Y=-200부터 산 정상 Y=400까지의 **고도차는 600블록**.
- 16블록 섹션 기준: 24개(기본 오버월드 384블록) → 64개(확장 1,024블록).
- 고도차가 600블록인 모든 지형을 강제로 생성하지 않음. 산맥 분포와 동굴 깊이는 별도 매개변수로 설계.
- 단순 Y 좌표 확대만으로 Terralith와 완벽히 호환되지 않으므로 **별도 월드 생성 프리셋**으로 구현.
- 고도 범위 변경 시 레지스트리, 지형 노이즈, 지하수, 광물 배치, 월드 표면, 구조물, 하이트맵, 조명, 생성 범위, 직렬화 및 클라이언트 동기화를 함께 검증.
- **새 월드에서만 활성화**하며, 운영 도중 높이 설정을 바꾸는 기능은 초기 범위에서 제외.

### 4.3 차원별 데이터팩 정책

| 차원 | 1차 목표 | 2차 목표 | 실패 시 전략 |
|---|---|---|---|
| 오버월드 | Terralith 기본 구성 | Extreme Worldgen 별도 프리셋 | 일반 바닐라 지형 유지 |
| 네더 | Incendium | 고도 및 구조물 호환 확대 | 기본 네더 또는 높이 제한형 커스텀 팩 |
| 엔드 | 일반 엔드 안정성 | Nullscape(384블록 높이) | 높이 제한 내 대체 데이터팩/커스텀 엔드 |

**중요:** Nullscape는 **선택 사항**. Nullscape 때문에 전체 프로젝트의 안정화와 출시를 지연시키지 않는다.

## 5. Bedrock 호환성 및 높이 제한

### 5.1 지켜야 할 원칙

- Bedrock의 물리적 클라이언트 한계를 Rust 서버 코드만으로 없앨 수 있다고 가정하지 않는다.
- Geyser 문서 기준 오버월드/오버월드로 매핑된 커스텀 차원은 대략 **Y=-512~512**까지, 다른 차원은 **Y=0~256** 범위 제약이 있다. 실제 경계 블록 처리와 사용 클라이언트 버전을 별도 검증.
- Pumpkin 내장 Bedrock 계층이 위 고도 범위를 동일하게 처리하는지는 **별도 실측 필수**. Geyser 문서의 한계를 그대로 Pumpkin의 보장으로 간주하지 않는다.
- 네더/엔드의 확장 높이는 별도 차원 매핑 및 하늘·바이옴·포털·엔티티 호환성을 검토.

### 5.2 Bedrock 호환성 테스트

- 오버월드 Y=-511, -200, 0, 319, 400, 510 부근에서 청크 렌더링/블록 상호작용 확인.
- Java/Bedrock 두 클라이언트의 동일 좌표 블록 상태 비교.
- 차원 전환, 리스폰, 멀티플레이어 동기화, 데이터팩 구조물 및 바이옴 효과 검증.
- 불가능한 높이가 있으면 **경고·해당 프리셋 제한·기능 비활성화**를 우선하고, 검증되지 않은 좌표 조작은 기본값으로 적용하지 않음.
- 부가 효과(잔디색·안개·하늘색·날씨 등)는 게임플레이 호환성과 별개의 시각적 호환성 항목으로 추적.

## 6. 성능 최적화 전략

| 영역 | 최적화 후보 | 정확성 보호 장치 |
|---|---|---|
| 틱 처리 | 안전한 병렬화, 배치 처리, 불필요한 반복 제거 | 레드스톤/엔티티 이벤트 순서 회귀 테스트 |
| 청크 생성 | 병렬 생성, 우선순위 큐, 캐시, 사전 생성 | 동일 시드 결과 비교, 경계 청크 검증 |
| 메모리 | 객체 할당 감소, 풀링, 팔레트 압축, 상한 관리 | 메모리 누수·데이터 손상 테스트 |
| 스토리지 | 압축/쓰기 배치, 비동기 I/O | 크래시 복구 및 저장 무결성 테스트 |
| 네트워크 | 패킷 배치, 관심 영역 최적화, 압축 비용 측정 | Java/Bedrock 패킷 동기화 및 지연 측정 |
| 플러그인 | WASM 우선, JVM 선택형, 호출 최소화 | 기존 플러그인 기능 검사 및 격리 테스트 |

**측정 원칙:** 최적화는 가설이 아니라 프로파일링으로 결정한다. 성능 저하가 발생하면 되돌릴 수 있도록 기능별 플래그와 벤치마크 결과를 남긴다.

## 7. 개발 단계 및 완료 기준

### Phase 0 — 기준선 구축

**작업**
- Pumpkin 안정적인 커밋/릴리스 고정, 포크, 빌드 자동화.
- 동일 머신에서 Paper 26.3과 Pumpkin 26.3의 기본 설정 비교.
- 기준 테스트 월드, 시드, 클라이언트 버전, 봇/실유저 시나리오 정의.

**완료 조건:** Linux에서 빌드 성공, Java·Bedrock 기본 접속, 기준 성능 로그 확보.

### Phase 1 — 기본 생존 엔진 안정화

**작업**
- 이동, 전투, 인벤토리, 블록, 청크, 포털, 월드 저장 검증.
- 크래시·청크 손상·중복 아이템·플레이어 데이터 유실 회귀 테스트.

**완료 조건:** 핵심 생존 시나리오 연속 플레이 및 저장/재실행 테스트 통과.

### Phase 2 — 데이터팩 런타임

**작업**
- 데이터팩 ZIP/폴더 로딩, 우선순위, 오류 처리.
- 함수·레시피·루트 테이블 등 기본 기능 테스트.
- 바이옴/차원/Worldgen 관련 Registry 누락 항목 구현.

**완료 조건:** 대표 데이터팩을 로드하고 의미 있는 결과가 실제 월드에서 재현됨.

### Phase 3 — Paper 플러그인

**작업**
- PatchBukkit 통합, JVM 의존성/메모리 별도 계측.
- 플러그인별 명령, 이벤트, 권한, 월드 조작 검증.
- 호환성 표: PASS / PARTIAL / FAIL / NOT TESTED 유지.

**완료 조건:** 선정한 필수 플러그인의 기능 테스트 통과 및 재시작 후 상태 일관성 확보.

### Phase 4 — 고품질 데이터팩 적용

**작업**
- Terralith → Incendium 순서로 월드 생성 비교.
- 바이옴, 구조물, 지형 경계, 차원 이동, 서버 측 이벤트 검사.
- Nullscape는 별도 실험 분기로 검토하고 실패 시 대체 전략 적용.

**완료 조건:** Java 기준 주요 지형/구조물 검증 통과, 치명적인 월드 생성 오류 없음.

### Phase 5 — Extreme Worldgen

**작업**
- 1,024블록 높이 지원에 필요한 청크/레지스트리/조명/저장 구조 수정.
- 고도차 600블록 산맥, 깊은 지하 구조, 지형 노이즈 프리셋 개발.
- Vanilla 모드와 분리하고 개발 플래그로 활성화.

**완료 조건:** 새 월드 생성·저장·재시작 가능, 목표 좌표대의 Java 렌더링/블록 상호작용 검증 통과.

### Phase 6 — Bedrock 완전성 검증

**작업**
- Pumpkin 내장 Bedrock 방식과 Geyser 프록시 방식 비교.
- 높이/차원/바이옴/포털 및 주요 데이터팩 지형의 동등성 테스트.
- 불가능한 조건의 자동 탐지와 운영 경고.

**완료 조건:** 공식 지원으로 표시한 구성에서 주요 기능 정상, 미지원 조합 명시.

### Phase 7 — 성능·안정화·릴리스

**작업**
- TPS/MSPT/RSS/PSS/CPU/청크 생성 시간/디스크 I/O 측정.
- 플레이어 수, 시뮬레이션 거리, 플러그인 조합별 부하 테스트.
- 자동 백업 및 복원, SIGTERM 정상 종료, 월드 손상 시 복구 테스트.
- 릴리스 노트, 알려진 제한, 재현 가능한 설치 문서 작성.

**완료 조건:** 치명적 데이터 유실 버그 0건, 지원 기능 테스트 통과, 성능 개선을 재현 가능한 데이터로 제시.

## 8. 성능 목표 및 벤치마크 설계

### 8.1 목표 수치(달성 보장 아님)

- **TPS:** 정상 부하에서 20 유지.
- **MSPT:** 동일 기능·설정·플레이어 부하 기준 Paper 대비 **30% 이상 감소**를 1차 도전 목표로 설정.
- **메모리:** 동일 조건의 **프로세스 전체 RSS/PSS 30% 이상 절감**을 1차 도전 목표로 설정(순수 Rust 모드).
- **PatchBukkit 모드:** JVM 포함 총 메모리와 성능을 별도 집계. 순수 Rust 수치를 혼용하지 않음.
- **월드 저장/재시작:** 데이터 손상 없이 동작.
- **지형 생성:** vanilla-compatible과 extreme_worldgen 결과를 따로 계측.

### 8.2 공정한 비교를 위한 3개 프로파일
1. paper-baseline: Paper + 동일 기능 플러그인·데이터팩.
2. pumpkin-native: Pumpkin + WASM 또는 내장 기능만.
3. pumpkin-compat: Pumpkin + PatchBukkit + 동일 플러그인.

각 구성에서 CPU 스레드, RAM 상한, 시드, view/simulation distance, 플레이어 위치·행동, 사전 생성 청크 여부를 고정한다. 플레이어 수와 장시간 테스트는 부하에 따라 단계적으로 증가시킨다.

## 9. 운영·안전·라이선스

- 첫 개발 및 성능 테스트는 **운영 서버와 분리된 독립 환경**에서 진행.
- 기존 월드의 원본을 직접 덮어쓰지 않으며, 수정 전에 전체 백업과 복원 가능성 확인.
- 사용 중인 플러그인/데이터팩에 대한 개별 호환성 문서 작성.
- 서버 공개 시 온라인 인증, 권한 분리, 방화벽, 네트워크 입력 검증을 기본으로 적용.
- Pumpkin은 **GPL-3.0** 라이선스이므로 포크 배포와 라이선스 의무를 사전에 검토.
- Terralith/Incendium/Nullscape는 저작권자 조건을 따르며, **무단 번들 배포하지 않는다**. 필요 시 사용자가 공식 배포처에서 설치하도록 안내.
- 원본 프로젝트와 혼동되지 않도록 별도 프로젝트명·고지·저작권 표시 사용.

## 10. 핵심 리스크 및 대응

| 위험 | 영향 | 대응 전략 |
|---|---|---|
| Pumpkin 바닐라 기능 미완성 | 심각 | Phase 1 기능 검증, 문제 발견 시 모듈 단위 보완 |
| 월드 생성 Registry 기능 부족 | 심각 | 필수 Registry부터 구현 후 실제 데이터팩 검증 |
| PatchBukkit 호환성 불완전 | 높음 | 플러그인별 PASS/PARTIAL/FAIL 관리; WASM 대체 가능성 검토 |
| Bedrock 네더/엔드 높이 제한 | 높음 | 안전한 차원 설정 우선, Nullscape 선택적 지원 |
| 초고도 월드의 메모리/생성 비용 | 높음 | 기본값 OFF, 청크 우선순위·압축·사전 생성 도입 |
| Java/Bedrock 상태 불일치 | 높음 | 동일 좌표·엔티티·포털 자동/수동 비교 |
| 업스트림 Pumpkin 변경 | 중간 | 업스트림 커밋 고정, 작은 단위 패치, 정기 리베이스 |
| 라이선스/원본 데이터팩 재배포 | 높음 | 배포 전 라이선스 검토, 외부 다운로드 연동 |

## 11. 개발 작업 구조(제안)

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
├── upstream/                 # Pumpkin 포크 또는 Git 관리 영역
├── custom/                   # 프로젝트 확장 코드(실제 크레이트 배치는 조사 후 확정)
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

> 위 폴더는 **예시 개발 구조**이며 Pumpkin 실제 저장소의 크레이트/API와 통합 방식에 맞춰 조정한다.

## 12. MVP 완료 판정

다음 조건을 모두 만족하면 **MVP(첫 사용 가능 버전)** 로 인정한다.

- [ ] Java와 Bedrock에서 같은 월드에 정상 접속.
- [ ] 기본 생존 시스템 및 저장/복원 기능이 안정적으로 동작.
- [ ] 최소 한 개의 실용적인 Paper 플러그인과 기본 데이터팩 기능 검증.
- [ ] Terralith 또는 Incendium 중 하나 이상에서 실제 월드 생성 검증.
- [ ] Paper 대비 성능 비교 결과를 재현 가능하게 기록.
- [ ] 치명적인 월드 손상·데이터 유실 문제가 없음.
- [ ] 지원되는 조합과 미지원 조합을 명확히 문서화.

**MVP 이후 확장 목표:** Terralith + Incendium 완성 → Extreme Worldgen → Nullscape/대체 엔드 → 추가 최적화.

## 13. 참고 자료

- Pumpkin 공식 저장소: https://github.com/Pumpkin-MC/Pumpkin
- Pumpkin 공식 사이트: https://pumpkinmc.org/
- Pumpkin 월드 엔진 문서: https://docs.pumpkinmc.org/developer/world
- Pumpkin 데이터팩 레지스트리 추적: https://github.com/Pumpkin-MC/Pumpkin/issues/731
- PatchBukkit: https://github.com/Pumpkin-MC/PatchBukkit
- PatchBukkit 기술 설명(2026-10-23): https://blog.pumpkinmc.org/2026-10-23-patchbukkit-nms-and-headless-paper/
- Geyser 현재 한계: https://geysermc.org/wiki/geyser/current-limitations/
- Terralith: https://modrinth.com/datapack/terralith
- Incendium: https://modrinth.com/datapack/incendium
- Nullscape: https://modrinth.com/datapack/nullscape

---

**프로젝트 최종 정의:** Rust-first, Java/Bedrock cross-play, datapack-capable, Paper-compatible, optionally extreme-worldgen Minecraft server.
