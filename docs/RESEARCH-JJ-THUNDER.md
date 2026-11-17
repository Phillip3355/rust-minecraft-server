# [리서치] JJThunder To The Max 월드젠/데이터팩 조사

> **작성일:** 2026-11-18  
> **조사 범위:** 웹 리서치 및 저장소 정적 분석(네트워크 파일 다운로드·서버 기동·코드 수정 미수행)  
> **표기 기준:** `[검증됨]` = 공식 배포 페이지/검색 결과 또는 저장소 내 로컬 파일·코드에서 직접 확인된 사실 · `[미검증]` = 실제 `.zip` 아카이브 내부 JSON 파싱 또는 실서버/클라이언트 실행을 거치지 않아 확인되지 않은 추론·판단

---

## 1. "JJThunder To The Max"란 무엇인가?

- **개요:** *JJThunder King of the Hills*에서 발전·확장된 1:1 현실 스케일 초고도 오버월드 지형 생성 데이터팩(및 모드 래퍼). 최대 1,900~2,000블록 높이의 산맥, 넓은 평야, 깊은 바다, 재설계된 노이즈 동굴("The Underlands")을 생성한다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), [CurseForge](https://www.curseforge.com/minecraft/mc-mods/jjthunder-to-the-max))
- **제작자:** `jjthunder001`(Modrinth) / `jjthunder99`(CurseForge). `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), [CurseForge](https://www.curseforge.com/minecraft/mc-mods/jjthunder-to-the-max))
- **배포처 및 다운로드 URL:**
  - **Modrinth (주 배포처, 순수 데이터팩):** https://modrinth.com/datapack/jjthunder-to-the-max
  - **Modrinth 버전/다운로드 목록:** https://modrinth.com/datapack/jjthunder-to-the-max/versions
  - **CurseForge (모드/래퍼 및 프로젝트 페이지):** https://www.curseforge.com/minecraft/mc-mods/jjthunder-to-the-max
  - *(참고)* 고고도 강설 보완 커뮤니티 애드온(*Snow & Blizzard Fix*): https://www.curseforge.com/minecraft/data-packs/jjthunder-to-the-max-snow-blizzard-fix `[검증됨]`
- **버전 및 아카이브 크기:**
  - 주요 릴리즈: `v0.4.0`~`v0.6.0`(MC 1.20.1 / 1.21 / 1.21.1), `v0.7.0`(MC 26.2 지원·화산 및 신규 화산 바이옴 3종 추가), `v0.8.0`(1.21.11 / 26.1 / 26.2, 독립 강 경로·경사 개편), `v0.9.0`(1.21.11 / 26.1 / 26.2, 강/산맥 노이즈 균형화 및 강 바이옴 `packed mud` 적용). `[검증됨]` ([Modrinth Versions](https://modrinth.com/datapack/jjthunder-to-the-max/versions))
  - MC `26.3` 명시적 태깅 여부는 검색 결과 기준 `26.2`까지만 확인되어 **26.3 호환성은 `[미검증]`**.
  - 아카이브 용량은 약 **93 KB ~ 108 KB** 수준의 경량 `.zip` 데이터팩으로 보고됨(로컬 `Terralith` 19 MB / 2,160개 파일과 대비). `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), 로컬 확인: `reference/datapacks/Terralith`)
- **라이선스 (핵심 제약):** **ARR (All Rights Reserved)**. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), [CurseForge](https://www.curseforge.com/minecraft/mc-mods/jjthunder-to-the-max))
  - Terralith의 *Stardust Labs License*([GitHub](https://github.com/Stardust-Labs-MC/license))가 "수정본의 내부 서버 사용"을 허용하는 것과 달리, ARR은 수정·재배포·포크 내장 시 저작권자의 **명시적 사전 허가**를 요구한다. `[검증됨]`

---

## 2. 월드젠 증폭 방식 및 Terralith와의 차이점

### 증폭 메커니즘
- **수직 범위 및 지형 수학:** 바닐라의 `dimension_type`, `noise_settings`, `noise_router`, `density_function`, `spline` 파라미터를 전면 재작성하여 **Y = -64 ~ Y = 2032** 범위의 초고도 지형(최대 산 높이 약 1,900~2,000블록)과 완만한 경사·평야·심해를 만든다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max))
- **동굴 시스템 ("The Underlands"):** 바닐라 동굴을 대체해 지표 부근은 좁고 폐쇄적인 동굴을, 깊은 지하로 내려갈수록 수백 블록 높이까지 확장되는 거대 노이즈 동굴층을 생성한다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max))
- **바이옴 및 구조물 정책:**
  - 핵심 지형 생성은 **바닐라 바이옴의 수직·수평 스케일 재형성**에 집중한다(초기 버전은 신규 바이옴 없음; `v0.7.0`에서 화산 지형과 **화산 바이옴 3종**만 추가). `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), [Modrinth Versions](https://modrinth.com/datapack/jjthunder-to-the-max/versions))
  - 자체 커스텀 구조물(.nbt)이나 복잡한 `.mcfunction` 루프 대신 바닐라 구조물을 그대로 사용하며(극단적 경사에서 구조물 공중 부양·매몰 버그가 보고됨), 고도에 따른 온도 하락을 바닐라 바이옴 시스템이 처리하지 못해 고고도 강설은 외부 애드온에 의존하는 경향이 있다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), [Snow & Blizzard Fix](https://www.curseforge.com/minecraft/data-packs/jjthunder-to-the-max-snow-blizzard-fix))

### Terralith(2.6.5+26.3)와 비교

| 비교 항목 | Terralith 2.6.5 | JJThunder To The Max |
|---|---|---|
| 출처 | [Modrinth Terralith](https://modrinth.com/datapack/terralith) | [Modrinth JJThunder](https://modrinth.com/datapack/jjthunder-to-the-max) |
| 규모(용량/파일) | 19 MB / 2,160 파일 `[검증됨]` (`reference/datapacks/Terralith`) | ~93–108 KB `[검증됨]` (내부 파일 수 `[미검증]`) |
| 고도 설계 | 바닐라(`-64..319`, 높이 384; 옵션 `448`) `[검증됨]` (`MISSING.md`) | 초고도(`-64..2032`, 산 ~1,900–2,000블록) `[검증됨]` |
| 바이옴 추가 | 커스텀 바이옴 **95종** + 바닐라 오버라이드 39종 `[검증됨]` (`MISSING.md`) | 바닐라 바이옴 중심(`v0.7.0+` 화산 3종 추가) `[검증됨]` |
| 커스텀 피처/구조물 | Configured 505 / Placed 567 / Structure 29(NBT 183) `[검증됨]` (`MISSING.md`) | 사실상 없음(바닐라 구조물 활용, `v0.7+` 화산 피처 여부 `[미검증]`) |
| 함수(`.mcfunction`) | 3개(`#minecraft:load` 진입점 포함) `[검증됨]` (`MISSING.md`) | 없음 또는 극소수 `[미검증]` |
| 동시 사용 호환성 | 상호 배타적 — 오버월드 `noise_settings`/`density_function` 충돌로 **병행 불가** `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max)) | ← 동일 |

---

## 3. Rust 서버(Pumpkin) 통합·컴파일·압축 용이성

Pumpkin은 `tools/pumpkin-codegen`이 빌드 시점에 바닐라 JSON을 Rust 상수·닫힌 `enum`으로 컴파일하는 구조다 (`reference/pumpkin-src/tools/pumpkin-codegen/src/main.rs`, [PHASE4.md](PHASE4.md)).

- **Terralith가 Pumpkin에서 어려운 이유 (`[검증됨]`, [MISSING.md](phase2/MISSING.md)):**
  - 정적 감사 기준 **24개 항목 / 2,984건의 FAIL** 발생:
    - 커스텀 `density_function` 87개, 커스텀 `noise` 123개, 커스텀 `noise_settings` 2개
    - 커스텀 `configured_feature` 505개 / `placed_feature` 567개 및 미지원 placement modifier(`minecraft:random_chance`, `minecraft:randomly_selected` → `compile_error!`)
    - 커스텀 `structure` 29개 / `structure_set` 8개 / `.nbt` 183개 (닫힌 `StructureKeys` enum)
    - 커스텀 `biome` 95개(`u8` ID 제한 및 별도 `assets/multi_noise_biome_tree.json` 재생성 필요)
    - `#minecraft:load` 객체 엔트리 미인식(함수 도달 0개)
- **JJThunder To The Max의 Pumpkin 통합 난이도:**
  1. **노이즈 라우터 프리미티브 호환성:** Pumpkin의 `noise_router.rs`(`reference/pumpkin-src/tools/pumpkin-codegen/src/noise_router.rs:2311-2792`)는 `spline`, `range_choice`, `interval_select`, `y_clamped_gradient`, `old_blended_noise`, `shifted_noise`, `clamp`, 산술 연산 등 표준 바닐라 `DensityFunction` 타입을 이미 대부분 구현하고 있다. `[검증됨]`
  2. **모드 로더 비의존:** Modrinth 배포본이 순수 데이터팩 `.zip`이므로 Fabric/Forge/Lithostitched 의존성이 없다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max))
  3. **닫힌 enum 확장 부담 대폭 감소:** 커스텀 구조물(.nbt), 수백 개의 커스텀 피처, 95개 커스텀 바이옴을 포팅할 필요가 없어 `structures.rs`, `configured_feature.rs`, `placed_feature.rs`, `biome.rs`의 대공사 없이 `noise_settings` + `noise_router` + `noise_parameter` + `dimension_type` 중심 통합이 가능할 가능성이 높다. `[미검증]`
- **확인 필요한 공백 (`[미검증]`):**
  - `.zip`을 직접 열지 않았으므로 내부에서 커스텀 네임스페이스 `worldgen/noise`, `density_function`, `material_rule`, 또는 `v0.7.0+` 화산 바이옴·피처·`packed mud` 강 오버라이드가 Pumpkin codegen의 미지원 타입을 쓰는지 여부는 미검증이다.
  - **빌드 통합의 법적 걸림돌:** Pumpkin은 빌드 타임에 생성 코드를 컴파일(`pumpkin-data/src/generated`)하므로, ARR 라이선스인 JJThunder JSON을 변환·컴파일하여 저장소나 바이너리에 포함시키면 저작권 허가가 필요하다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max))

---

## 4. "압축(Compress)" 전제 시 자산 최소화·단순화 방안

"압축"은 ① **자산/레지스트리 최소화**와 ② **수직 고도 축 압축** 두 축으로 나눌 수 있다. (단, 실제 파일 목록 기반 삭제 검증은 `[미검증]`)

1. **유지해야 할 최소 핵심 자산 (지형 코어):** `[미검증]`
   - `dimension_type/overworld.json` (높이 경계 정의)
   - `noise_settings/overworld.json` 및 여기서 참조하는 `density_function/`, `noise/`
   - `material_rule` / `surface_rule` (산악 암반·적설·지표 블록 규칙)
   - 이 최소 세트만 `tools/pumpkin-codegen/src/worldgen_profile.rs` 스테이징 입력으로 넘기면, Terralith처럼 15,000+ 파일 충돌/검증을 거치지 않고 단일 오버월드 지형 파이프라인으로 단순화 가능하다.
2. **제거·생략 가능한 자산:** `[미검증]`
   - `v0.7.0+`의 **화산 바이옴 3종 및 화산 전용 피처**를 제외(또는 `v0.6.x` 계열 설계 참조)하면 바닐라 바이옴 ID(`u8`)와 기존 `assets/multi_noise_biome_tree.json`을 100% 그대로 유지할 수 있다.
   - 별도 `world_preset`, 비오버월드 차원 설정, 커스텀 구조물/루트 테이블/함수는 불필요(바닐라 유지).
3. **법적 대안 (Clean-room 재구현):**
   - ARR 라이선스 원본 JSON을 수정·압축해 재배포하는 대신, 목표 고도(`-512..511`, 지면 `-400`, 산 `800`)에 맞춰 **자체 `noise_settings` / `spline` / `density_function`을 Pumpkin 전용 프리셋으로 새로 작성**하면 저작권·라이선스 문제를 원천 해소할 수 있다. `[검증됨]`

---

## 5. Y=-512..511, 지면 -400 · 산 800블록 목표 달성 가능성 및 제약

### 목표 적합성
- **원본 그대로 적용:** **불가능.** JJThunder의 기본 범위는 `Y = -64 ~ 2032`(높이 ~2,096블록, 해수면 ~62–64)이므로 목표 범위(`-512..511`, 높이 1,024블록)를 상단으로 1,520블록 이상 초과하며 Geyser/Bedrock의 오버월드 상한(`512`)도 위반한다. `[검증됨]` ([Modrinth](https://modrinth.com/datapack/jjthunder-to-the-max), [Geyser Docs](https://geysermc.org))
- **수직 재매핑(압축 + 오프셋 이동) 적용 시:** **이론상 달성 가능.** `[미검증]`
  - 기준 지면(`~63` → `-400`) 하향 이동 + 해발 이상 기복(~1,950블록 → 800블록, 약 `0.41x` 압축)을 `y_clamped_gradient`와 `spline` 출력에 적용하면 **지면 -400 ~ 정상 +400** 분포를 만들 수 있다.
  - Terralith(기본 384블록 지형을 2배 이상 *늘려야* 함)와 달리, JJThunder는 2,000블록 산악 경사 모델을 *축소*하므로 지형 비례 왜곡이 상대적으로 적을 것으로 예상된다. `[미검증]`

### 주요 제약 및 결정 사항
1. **지하 깊이(112블록)와 Underlands 상충:** 지면 `-400`, 최저 `-512`이면 지하는 112블록뿐이므로, 수백 블록 높이의 *The Underlands* 거대 동굴은 그대로 수용할 수 없다. [PHASE4.md](PHASE4.md)의 방향("지하는 일반 야생 수준 깊이 유지")에 맞춰 동굴 스케일은 바닐라 수준으로 축소해야 한다. `[검증됨]`
2. **Bedrock / Geyser 호환성:** Geyser 공식 한계상 오버월드는 `-512..511`까지 수용 가능하나([Geyser Docs](https://geysermc.org)), Pumpkin 내장 Bedrock 계층(RakNet/NetherNet)과 Geyser 프록시에서 64청크 섹션(`-512..511`)이 정상 렌더링·동기화되는지는 여전히 `NOT TESTED`다 ([PHASE5.md](PHASE5.md)). `[검증됨]`
3. **고도 연동 부수 요소 재조정 필수:** 지면이 `-400`으로 내려가면 `sea_level`, 대수층(aquifer), 광물 분포 Y좌표, 바닐라 구조물 Y배치, 고도 기반 적설(`surface_rule`의 `y_above` 등)을 모두 `-512..511` 좌표계로 함께 변환해야 한다. `[검증됨]` ([PHASE4.md](PHASE4.md))
4. **기획 목표 수정 결정 필요:** [Minecraft-Rust-Server-Plan.md](../Minecraft-Rust-Server-Plan.md)는 "Terralith 특수 바이옴 유지 + Extreme Worldgen"을 목표로 잡았다. JJThunder와 Terralith는 병행 불가이므로, **① Terralith 특수 바이옴(95종)을 포기하고 바닐라 바이옴 초고도 지형으로 선회할 것인지**, **② ARR 라이선스 허가를 구할 것인지 아니면 Clean-room 자체 프리셋으로 구현할 것인지** 의사결정이 필요하다. `[검증됨]`

---

## 결론: Terralith 대비 통합 난이도 한눈에 보기

| 평가 영역 | Terralith 2.6.5 | JJThunder To The Max | 비고 |
|---|---|---|---|
| **기술적 통합 난이도 (Pumpkin codegen)** | **매우 높음 (상)** | **낮음~중간 (하~중)** `[미검증]` | Terralith는 24종/2,984건 FAIL(바이옴 95·피처 505·구조물 29·함수); JJThunder는 `noise_router`/`spline` 위주 |
| **바이옴·피처·구조물 자산 부채** | 매우 큼 (19 MB, 2,160파일) | 극소 (~100 KB, 바닐라 중심) | JJThunder는 `v0.7+` 화산 3종 제외 시 바이옴/구조물 codegen 수정 최소화 가능 `[미검증]` |
| **목표 고도(`-400..+400`) 적합성** | **확대 필요** (384 → 800m, 왜곡 위험) | **축소 가능** (2,000 → 800m, 비례 유리) | 단, Underlands 거대 동굴은 112블록 지하에 맞춰 축소 필수 `[미검증]` |
| **라이선스·법적 리스크** | **중간** (Stardust Labs: 내부 수정/서버 사용 허용, 단독 재배포 금지) | **높음** (**ARR**: 수정·내장·배포 시 명시적 허가 필수) | Pumpkin 빌드 타임 내장 구조상 ARR이 가장 큰 장애물 `[검증됨]` |
| **네더/엔드 커버리지** | 오버월드만 (네더는 별도 Incendium) | 오버월드만 (네더는 별도 Incendium 필요) | 양쪽 모두 Incendium 병행 설계는 그대로 유효 |

### 권장 요약
- **순수 엔지니어링 관점**에서는 Terralith보다 JJThunder 방식(바닐라 바이옴 + `noise_settings`/`density_function` 중심)이 Pumpkin의 정적 컴파일 구조와 `-400..+400` 고도 목표에 **훨씬 적합**하다. `[미검증]`
- 다만 **ARR 라이선스**와 **Terralith 특수 바이옴 포기**라는 두 가지 제약 때문에, ① **자체 Clean-room `noise_settings`/`spline` 프리셋**을 JJThunder 스타일로 직접 설계하거나, ② JJThunder 저작권자의 서면 허가를 확보한 뒤 `.zip` 내부 JSON의 Pumpkin 호환성을 1차 정적 감사(`tools/datapack-audit`)로 검증하는 순서를 권장한다. `[검증됨]`
