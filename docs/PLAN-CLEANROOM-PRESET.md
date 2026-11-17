# Clean-room Extreme Worldgen 프리셋 설계 계획안 및 Phase 5b 정적 검토

> **작성일:** 2026-11-18  
> **작성 범위:** 문서 설계 및 정적 코드 검토만 수행 (**실행 코드 수정·서버 기동·네트워크 다운로드 미수행**)  
> **상태:** 설계 초안 (`NOT TESTED` — 실서버 생성·Java/Bedrock 접속·빌드 실행 미검증)  
> **기준 커밋:** `0340629` (부모 `5e87898` Phase 5 부분 구현 포함)

---

## 0. 배경 및 설계 원칙

1. **라이선스 제약 해소 (Clean-room):**
   - Terralith(2.6.5+26.3)는 커스텀 바이옴 95종·피처 505개·구조물 29개 등으로 Pumpkin 정적 codegen 통합 비용이 매우 크다(`docs/phase2/MISSING.md`, 24개 항목 2,984건 FAIL).
   - 대안으로 검토한 *JJThunder To The Max*는 `All Rights Reserved (ARR)` 라이선스여서 원본 JSON/데이터팩을 수정·포크 내장할 수 없다(`docs/RESEARCH-JJ-THUNDER.md`).
   - 따라서 JJThunder의 **고도 극대화 지형 철학(넓은 평원·완만한 산기슭·초고도 산맥·깊은 지하 동굴 축소판)**만 개념적으로 참고하고, 수치·스플라인·밀도 함수·표면 규칙은 **100% 자체 작성 Clean-room 프리셋(`cleanroom_extreme_v1`)**으로 설계한다.
2. **확정 좌표계 (`Minecraft-Rust-Server-Plan.md` v1.3 / `docs/PHASE4.md` / `docs/PHASE5.md`):**
   - **블록 Y 범위:** `-512 .. 511` (`min_y = -512`, `height = 1024`, `logical_height = 1024`, 16블록 청크 **64섹션**). Geyser/Bedrock 오버월드 경계(`-512..512`) 정합.
   - **기준 지면 / 해수면:** `ground_y_target = -400`, `sea_level = -400`.
   - **산 높이 목표:** 지면 기준 `+800`블록 → **정상 목표 `Y = +400`** (상한 `511` 아래 111블록 여유 확보, 수목·구조물·들쭉날쭉한 봉우리 클리핑 방지).
   - **지하 깊이:** `-512 .. -400` (**112블록**; 기반암 `-512..-508` 포함). 바닐라 일반 야생의 지하 두께(`-64..63` ≈ 127블록)와 유사한 수준 유지.
3. **바닐라 바이옴·구조물 보존:**
   - 커스텀 바이옴(`u8` ID 확장, `multi_noise_biome_tree.json` 재생성, Bedrock 바이옴 매핑 추가)과 커스텀 구조물(`.nbt` / 닫힌 `StructureKeys` 확장)을 추가하지 않고 **바닐라 26.3 바이옴·구조물·피처를 유지**한다.

---

## 1. Clean-room 월드젠 파라미터 설계

### 1.1 수직 구역 예산 (Vertical Budget: `-512 .. 511`)

| 구역 | Y 범위 | 두께 | 설계 의도 |
|---|---|---:|---|
| 기반암 바닥 (`bedrock_floor`) | `-512 .. -508` | 5 | `above_bottom: 0..5` 수직 그라데이션 |
| 심층 지하 / 딥슬레이트 (`deepslate`) | `-507 .. -445` | 63 | 딥슬레이트·용암 대수층(`-502` 부근)·축소형 Underlands 거대 동굴 |
| 천층 지하 / 일반 암반 (`stone`) | `-444 .. -401` | 44 | 일반 동굴·협곡·광맥(구리/철)·지하 구조물 |
| 해저 / 심해 (`ocean` / `deep_ocean`) | `-465 .. -400` | 0..65 | 음수 `continentalness` 구역에서 수심 확보 |
| **기준 해수면 / 평원 (`sea_level`)** | **`-400`** | — | 강(`-402..-396`), 해변, 평원(`-400..-375`) |
| 구릉 / 고원 (`hills` / `plateau`) | `-374 .. -150` | 225 | 완만한 경사, 계곡, 산악 전이대 |
| 고산 / 설산 정상 (`peaks`) | `-149 .. +400` | 550 | JJThunder 스타일 장거리 경사 + 알파인 능선, 목표 정상 `Y ≈ +400` |
| 상단 여유 공간 (`headroom`) | `+401 .. +511` | 111 | 극단 시드 피크·나무·구름(`cloud_height ≈ 432`)·건축 여유 |

### 1.2 `dimension_type` 및 `noise_settings`

Clean-room 프로파일 전용 오버월드 오버라이드(기본 바닐라와 분리 생성):

```jsonc
// dimension_type/overworld (cleanroom_extreme_v1 프로파일)
{
  "min_y": -512,
  "height": 1024,
  "logical_height": 1024,
  "coordinate_scale": 1.0,
  "has_skylight": true,
  "has_ceiling": false,
  "ambient_light": 0.0,
  "attributes": {
    "minecraft:visual/cloud_height": 432.0
  }
}
```
> **참고:** 바닐라 오버월드 `cloud_height`는 `192.33`(`crates/pumpkin-data/src/generated/dimension.rs:104`)이므로 산 중턱을 가로지르지 않도록 정상 목표(`400`) 위인 `432.0` 부근으로 상향한다.

```jsonc
// worldgen/noise_settings/cleanroom_extreme.json (설계안)
{
  "sea_level": -400,
  "disable_mob_generation": false,
  "aquifers_enabled": true,
  "ore_veins_enabled": true,
  "legacy_random_source": false,
  "default_block": { "Name": "minecraft:stone" },
  "default_fluid": { "Name": "minecraft:water", "Properties": { "level": "0" } },
  "noise": {
    "min_y": -512,
    "height": 1024,
    "size_horizontal": 1,
    "size_vertical": 2
  },
  "spawn_target": [
    // 바닐라와 동일한 기후 구간 유지 (SpawnFinder는 depth=0 고정 평가이므로 좌표계 독립)
  ]
}
```
- **노이즈 셀 해상도:** `size_horizontal=1`, `size_vertical=2` → 셀 크기 `4x8x4`. 수직 셀 수는 바닐라 `48`개(`384/8`) → **`128`개(`1024/8`, 약 2.67배)** 로 증가하므로, `WrapperType::Interpolated`와 `Cache` 배치를 유지해 3D 노이즈 평가를 셀 격자로 제한한다.

### 1.3 `noise_router` & `density_function` 수학 설계

Pumpkin `tools/pumpkin-codegen/src/noise_router.rs`가 이미 지원하는 바닐라 프리미티브(`Gradient`/`y_clamped_gradient`, `Spline`, `ShiftedNoise`, `Noise`, `InterpolatedNoiseSampler`, `RangeChoice`, `IntervalSelect`, `Clamp`, `Wrapper`, `Blend*`, `Beardifier`, `FindTopSurface`)만 사용한다. 새로운 커스텀 `DensityFunction` 타입 추가는 불필요하다.

#### A. 지형 골격 (`noise.depth`, `offset` 스플라인, `factor`, `jaggedness`, `sloped_cheese`)

바닐라 오버월드의 `depth`는 다음 구조로 계산된다(`crates/pumpkin-data/src/generated/noise_router.rs:6604-6613, 8269-8296`):
- 바닐라: `y_clamped_gradient(-64, 320, +1.5, -1.5)` (기울기 `-3.0 / 384 = -1/128`) + `(offset_spline - 0.50375)` → `Y ≈ 63.5`에서 `depth = 0`.

Clean-room `-512..511` 매핑 설계:
1. **수직 깊이 그라데이션 (`cleanroom/y_gradient`):**
   - `y_clamped_gradient(from_y = -512, to_y = 512, from_value = +1.5, to_value = -1.5)`
   - 기울기: `-3.0 / 1024` (블록당 `-0.0029296875`).
   - 기준점 값:
     - `Y = -512` → `+1.5`
     - `Y = -400` (지면/해수면) → `1.5 - 3.0 * (112 / 1024) = +1.171875`
     - `Y = 0` (중간) → `0.0`
     - `Y = +400` (산 정상 목표) → `1.5 - 3.0 * (912 / 1024) = -1.171875`
     - `Y = +512` → `-1.5`
2. **기준 오프셋 시프트 & `offset` 스플라인 (`cleanroom/offset`):**
   - `offset_spline(continents, erosion, ridges)`의 출력에 **`-1.171875`** 를 더해(`LinearOperation::Add`) `offset_spline == 0`일 때 정확히 `Y = -400`에서 `terrain_depth = 0`이 되도록 맞춘다.
   - 스플라인 값 설계 (모두 `Add(-1.171875)` 이전의 순수 고도 기여분 `Δdepth = ΔY * (3.0 / 1024)`):
     - **심해 / 일반 바다 (`continents` `-1.0 .. -0.19`):** `Δdepth ≈ -0.19 .. -0.06` → 해저 `Y ≈ -465 .. -420` (해수면 `-400` 아래 수심 `20..65`블록).
     - **해안 / 평원 / 강 (`continents` `-0.11 .. +0.3`, `erosion` 높음):** `Δdepth ≈ 0.00 .. +0.08` → 고도 `Y ≈ -400 .. -373`. 계곡·강은 `ridges_folded` 저지대에서 `Δdepth ≈ -0.02 .. 0.0`으로 자연 침식.
     - **구릉 / 고원 (`continents` `+0.1 .. +0.6`, `erosion` 중간):** `Δdepth ≈ +0.15 .. +0.75` → 고도 `Y ≈ -349 .. -144`.
     - **극단 산맥 (`continents` `>= +0.35`, `erosion` `< -0.45`, `ridges_folded` `> +0.4`):** `Δdepth`를 최대 **`+2.25 .. +2.35`** 까지 선형/큐빅 스플라인으로 완만히 상승 → `depth = 0` 교차점이 `Y ≈ +368 .. +402`에 형성된다.
     - **상한 클램프:** `Δdepth` 최대치를 `+2.45` 이하로 제한해 `Y = +437` 이상으로 치솟지 않게 하고, `sloped_cheese`의 3D 노이즈 변동을 더해도 `Y = 511` 상한에 닿지 않도록 보호한다.
3. **`factor` 및 `jaggedness` 설계 (JJThunder 스타일 완만한 대지형 + 뾰족한 봉우리):**
   - `y_gradient`의 기울기가 바닐라 대비 `384 / 1024 = 0.375`배로 완만해지므로, 동일 `factor` 값에서도 경사가 더 길고 웅장해진다.
   - **평원/바다(`erosion > 0.2`):** `factor ≈ 4.5 .. 6.5`, `jaggedness = 0.0` → 평탄하고 넓은 대지 유지.
   - **산기슭/중턱(`erosion` `-0.4 .. +0.2`):** `factor ≈ 2.2 .. 3.8`, `jaggedness` 소량(`0.05..0.15`) → 거대한 산체와 완만한 계곡(JJThunder 특유의 1:1 스케일감).
   - **고산 능선/정상(`erosion < -0.5`, `ridges_folded > 0.5`):** `factor ≈ 1.4 .. 2.2`, `jaggedness ≈ 0.35 .. 0.65` → 정상 부근(`Y = +200 .. +400`)에서 암봉과 날카로운 능선 형성.
4. **상단·하단 슬라이드 (`top_slide` / `bottom_slide`):**
   - **하단 슬라이드:** `Y = -512 .. -492` (`from_value = +1.0` 또는 밀도 강제 양수, `to_value = 0.0`) → `Y = -512` 바닥 누수 방지.
   - **상단 슬라이드:** `Y = +488 .. +512` (`from_value = 0.0`, `to_value = -1.0` 램프; 바닐라 `304..320` 대응) → 정상 `Y = 400`은 전혀 깎지 않으면서도 극단 노이즈가 `Y = 511` 천장에 납작하게 잘리는 현상을 원천 차단.

#### B. 기후·바이옴 조회용 `multi_noise` 라우터 분리 (`multi_noise.depth`)

> **핵심 설계 포인트:** `crates/pumpkin-data/src/generated/noise_router.rs:3373-3400`에서 Pumpkin은 지형 생성용 `BaseNoiseRouter::depth`와 바이옴 선택용 `BaseMultiNoiseRouter::depth`를 **분리된 인덱스**로 가진다.

- **표면 바이옴 보존:** `temperature`, `vegetation`, `continents`, `erosion`, `ridges`는 바닐라 노이즈(`TEMPERATURE`, `VEGETATION`, `CONTINENTALNESS`, `EROSION`, `RIDGE`, `OFFSET`)와 동일 스케일·분포(`[-1, 1]`)를 사용한다(산맥 체인 크기를 키우고 싶다면 `continents`/`erosion`의 `xz_scale`을 `0.25 → 0.16` 수준으로 완만히 조정 가능하나, 값의 분산이 바닐라 `multi_noise_biome_tree.json` 범위 내에 있어야 함).
- **`multi_noise.depth` 재매핑 (지하 바이옴 보존):**
  - 바닐라 `BiomeTree`는 `depth ≈ 0.0`에서 지표 바이옴을, `depth ∈ [0.2, 0.9]`에서 `lush_caves`/`dripstone_caves`를, `depth >= 1.1`에서 `deep_dark`를 선택한다.
  - 만약 `multi_noise.depth`에 지형용 `noise.depth`를 그대로 쓰면 `Y = -512`에서도 `depth`가 `+0.33`에 불과해 `deep_dark`가 전혀 나오지 않는다.
  - 따라서 `multi_noise.depth`는 `terrain_depth`를 입력으로 받아 구간별 스케일링(또는 `RangeChoice(Y < -400)` + `y_clamped_gradient(-512, -400, +1.15, 0.0)`)을 적용한다:
    - **`Y >= -400` (지면 이상, 산 내부 포함):** `multi_noise.depth = max(0.0, terrain_depth)` → 지표에서는 `0.0`, 거대 산맥 심부(예: `Y = -200`인데 정상이 `+400`인 산 속)에서는 `0.0..0.7`이 되어 산악 내부 동굴에 `dripstone_caves`/`lush_caves`가 자연 배치된다.
    - **`Y < -400` (지면 이하 지하 112블록):** `y_clamped_gradient(-512, -400, +1.15, 0.0)`(또는 `terrain_depth * 3.4` + `offset` 보정)를 적용해 `-425..-465`에 `lush_caves`/`dripstone_caves`, `-470..-507`에 `deep_dark`가 안정적으로 출현하도록 한다.

#### C. `preliminary_surface_level` (`FindTopSurface`)

- `BaseSurfaceEstimator`의 `FindTopSurfaceData.lower_bound`를 바닐라 `-64` → **`-512`** 로 변경한다(`tools/pumpkin-codegen/src/noise_router.rs:2975, 3055`).
- `upper_bound` 밀도 함수 역시 `Clean-room`의 `sloped_cheese` 상한(최대 `Y = 511`)과 정합되는 `y_clamped_gradient`/`offset` 기반 추정 함수로 재구성한다. `SurfaceHeightEstimateSampler`(`surface_height_sampler.rs:84-96`)는 `self.minimum_y`까지 `cell_height` 단위로 하강 스캔하므로 `lower_bound = -512`이면 `-512..511` 전 범위를 정확히 탐색한다.

#### D. 동굴 시스템 & *The Underlands* 축소안

지하 두께가 `112`블록(`-512..-400`)이므로 수백 블록 높이의 원본 JJThunder *Underlands*는 불가능하다(`docs/RESEARCH-JJ-THUNDER.md` §5). 다음 3계층으로 **축소·재구성**한다:

1. **스파게티·엔트런스 동굴 (`entrances`, `spaghetti_2d`, `spaghetti_3d`, `pillars`):**
   - `caves/entrances`: 바닐라의 `y_clamped_gradient(-10, 30, 0.3, 0.0)`을 **`y_clamped_gradient(-440, -370, 0.3, 0.0)`** 로 이동 → 지면 `-400` 부근과 산기슭으로 동굴 입구가 열린다.
   - `caves/spaghetti_2d`: 바닐라의 `y_clamped_gradient(-64, 320, 8.0, -40.0)`을 **`y_clamped_gradient(-512, 120, 8.0, -40.0)`** 로 확장/이동 → 지하 깊은 곳부터 해발 `Y=120` 고산 내부까지 터널이 이어진다(단, 고산 외벽 붕괴를 막기 위해 `Y > -350`에서는 희소도 상향).
2. **축소형 Underlands 거대 공동 (`compressed_underlands_cheese`):**
   - `Y = -502 .. -420` (약 82블록 구간)에 바닐라 `cave_cheese` / `cave_layer` 밀도 임계값을 완화(`+0.27 → +0.16` 수준으로 음수 공동 확대)하고 `y_clamped_gradient` 창(window)을 `-505..-495` (바닥 0→1 페이드) 및 `-430..-415` (천장 1→0 페이드)로 씌운다.
   - 결과: **높이 25~50블록, 폭 60~120블록 급의 지하 거대 동굴층**이 `-495..-425` 사이에 형성되며, 상단 천장(`-420..-400`, 최소 20블록 암반)이 지면 평원을 지탱해 싱크홀 붕괴를 방지하고, `pillars` 노이즈로 거대 석주가 형성된다.
3. **누들 동굴 (`noodle`):**
   - 바닐라 `RangeChoice(min=-60, max=321)`(`generated/noise_router.rs:19408-19505`)를 **`min = -508, max = +350`** 으로 재매핑한다.

### 1.4 `surface_rule` (`material_rule`) 설계

Pumpkin `tools/pumpkin-codegen/src/material_rule.rs`는 `noise_settings.surface_rule` 인라인이 아니라 `worldgen/material_rule/*.json`과 `worldgen/material_condition/*.json`에서 규칙을 컴파일한다.

| 규칙 항목 | 바닐라 기준 (`material_rule.rs`) | Clean-room 재매핑 (`cleanroom_extreme`) | 비고 |
|---|---|---|---|
| **기반암 바닥** | `above_bottom: 0..5` | **`above_bottom: 0..5`** (`-512..-507`) | 상대 오프셋(`i8`)이므로 수정 불필요 |
| **딥슬레이트 전이** | `Absolute(0) .. Absolute(8)` | **`Absolute(-452) .. Absolute(-444)`** | 지하 하부 60블록만 딥슬레이트; 산 내부(`-444..+400`)는 일반 돌 유지 |
| **지표/해저 토양** | `StoneDepth`, `Water(offset=0,-1)` | 바닐라 동일 (단 `sea_level=-400` 연동) | `MaterialRuleContext.sea_level`이 자동 반영됨 |
| **저지대/강/해변** | `YAbove` 앵커 다수가 `60..64` 기준 | `Absolute(-404)..Absolute(-396)` 등으로 이동 | 모래/자갈/진흙 해안대 재정렬 |
| **고산 설선·암석** | `YAbove(90..256)` (`stony_peaks`, `jagged_peaks`, `frozen_peaks`, `snowy_slopes`, `grove`, `meadow`) | **`YAbove(-240 .. +250)`** 구간으로 단계 재배치 | 예: `Y >= -240`에서 암벽 노출 증가, `Y >= -120` 이상에서 만년설/얼음/가루눈 지배 |
| **온도 기반 적설** | `MaterialCondition::Temperature` | 바닐라 동일 (`compute_temperature`가 `sea_level` 사용) | `Weather::compute_temperature`(`pumpkin-util/src/biome.rs:127-142`)가 `sea_level + 17 = -383` 기준 고도 하강(`-0.05/40` per Y)을 계산하므로 `Y > +50` 이상에서는 대부분 바이옴이 자연 결빙·적설 |

> **주의 (`YOffsetStruct` 타입 범위):** `tools/pumpkin-codegen/src/material_rule.rs:39-43` 및 `crates/pumpkin-util/src/y_offset.rs:70-88`에서 `Absolute`는 `i16` (`-32768..32767`)이므로 `-512..511`을 무리 없이 표현하나, `AboveBottom`/`BelowTop`은 `i8` (`-128..127`)이다. 따라서 바닥/천장에서 127블록 이상 떨어진 기준(예: `-452`, `-120`, `+250`)은 **반드시 `Absolute(i16)`** 를 사용해야 한다(또는 `RelativeToSeaLevel(i16)` 확장 검토).

---

## 2. 바닐라 바이옴·구조물·피처 배치 설계 및 코드 갭 분석

### 2.1 바이옴 배치 전략

- **유지:** 바닐라 26.3 바이옴 목록(`crates/pumpkin-data/src/generated/biome.rs`)과 `assets/multi_noise_biome_tree.json` 및 `assets/bedrock/biomes.json`을 그대로 사용한다.
- **고도-바이옴 매칭 정합성:**
  - `Badlands` / `Desert` / `Savanna` 등 온난 바이옴은 `offset` 스플라인 상 고산 피크 구간(`erosion < -0.45`, `PV > 0.5`)보다 중간 고도/고원 비중을 높이고, `Jagged Peaks` / `Frozen Peaks` / `Stony Peaks` / `Snowy Slopes` / `Meadow` / `Grove` 바이옴이 출현하는 `(low erosion, high PV, high continentalness)` 지점에서 `800`블록 초고산(`Y = +200..+400`)이 솟아오르도록 `offset` 스플라인을 맞춘다.
  - 이를 통해 사막/정글 한복판에 부자연스러운 800m 흑요석/모래 기둥이 솟기보다, **알파인 산맥과 산악 계곡·고원 메도우** 중심의 자연스러운 800블록 경관이 형성된다.

### 2.2 바닐라 구조물(Structures) 호환성 및 갭

| 구조물 그룹 | Pumpkin 배치 방식 (정적 확인) | `-512..511` / 지면 `-400` 정합성 |
|---|---|---|
| **지표 구조물** (`village`, `pillager_outpost`, `ruined_portal`, `desert_pyramid`, `jungle_temple`, `swamp_hut`, `igloo`, `woodland_mansion`, `trail_ruins`, `ocean_monument`) | `WORLD_SURFACE_WG` / `OCEAN_FLOOR_WG` 하이트맵 및 `context.sea_level` 참조 (`jigsaw.rs:984`, `ruined_portal.rs:351`) | **원칙상 호환** (단, 고산 극단 경사에서 jigsaw 투영 범위 초과 가능성 있음; `jigsaw_placement.rs:49`의 `vertical: 384` 하드코딩은 `1024` 대응 검토 필요) |
| **난파선 (`shipwreck`)** | `height_sampler`가 `None`일 때 fallback `64` 사용 (`shipwreck.rs:119`) | **`context.sea_level`로 수정 필요** |
| **폐광 (`mineshaft`)** | `surface_height <= context.sea_level` 비교 (`mineshaft.rs:115`) + 지하 랜덤 Y | `min_y..sea_level` 범위가 `-512..-400`으로 자동 축소되나 세부 `placed_feature` / 높이 설정 감사 필요 |
| **고대 도시 / 시련의 회당 / 스트롱홀드** | `start_height`가 `placed_feature`/`structure` JSON의 `YOffset`에 의존. 특히 `GlobalStructureCache`(`placement.rs:381,530`)가 `BaseNoiseRouters::from_dimension(&Dimension::OVERWORLD)`를 직접 호출 | **중요 갭:** 스트롱홀드 검색 및 `start_height` 절대 Y(`-51` 등)가 `-400` 지면 위(허공)에 배치될 위험! 프로파일별 `BaseNoiseRouters` 주입 및 구조물 `start_height` 오프셋/검증 필수 |

### 2.3 광물·카버·지형 피처의 절대 Y 하드코딩 문제 (정적 감사 결과)

Clean-room 설계 시 가장 주의해야 할 부분은 **노이즈 라우터만 바꾸면 지하 광물과 동굴 카버가 `-512..-400`에 생성되지 않는다**는 점이다. 이번 정적 검토에서 다음 하드코딩 지점이 확인되었다:

1. **용암 수위 & 대수층 (`aquifer_sampler.rs:91, 129, 614`, `proto_chunk.rs:816`):**
   - `FluidLevel::new(-54, &Block::LAVA)`가 하드코딩되어 있다. 지면 `-400` 환경에서 용암 수위가 `-54`이면 공중/산 중턱까지 용암 대수층으로 채워지는 치명적 버그가 발생한다!
   - 또한 `aquifer_sampler.rs:614`에 `if level <= -10 && level != MIN_HEIGHT_CELL ...` 조건이 있어 `-10` 이하의 모든 지하 대수층이 용암 후보가 된다.
   - **해결:** `NoiseSettings` 또는 프로파일 설정에 `lava_level: i32` (예: Clean-room `-502`, 바닐라 `-54`)와 `lava_transition_offset`을 도입해 `aquifer_sampler`가 하드코딩 `-54`/`-10` 대신 프로파일 값을 읽도록 수정해야 한다.
2. **거대 광맥 (`OreVeinSampler` & `VeinContext`):**
   - `crates/pumpkin-world/src/generation/noise/ore_sampler.rs:71-86`: 구리 `0..50`, 철 `-60..-8` 하드코딩.
   - `crates/pumpkin-world/src/generation/surface/mod.rs:88-102`: `VeinContext::get_density`에 `-60..-8`, `0..50` 하드코딩.
   - `crates/pumpkin-world/src/generation/proto_chunk.rs:1217-1219`: `vein_min_y = (-60).max(chunk_min_y)`, `vein_max_y = 52.min(chunk_max_y)` 하드코딩.
   - `generated/noise_router.rs:19577-19638`: `vein_toggle`/`vein_ridged`의 `RangeChoice(-64..57)` 하드코딩.
   - **해결:** Clean-room 라우터에서는 `RangeChoice`를 **`-508 .. -390`**(철 `-508..-452`, 구리 `-448..-390`)으로 정의하고, `VeinContext` / `OreVeinSampler` / `proto_chunk.rs`의 범위를 프로파일(또는 `sea_level`/`min_y` 상대 오프셋) 기반으로 일반화한다.
3. **바닐라 광물 `placed_feature` (`generated/placed_features_generated.rs`):**
   - 바닐라 다이아몬드·레드스톤·금·철·석탄·청금석·에메랄드 등의 `HeightRange`는 `Absolute { absolute: -64..256 }` 등을 사용한다(`generated/placed_features_generated.rs:73, 1778, 1796, ...`).
   - 지면이 `-400`이면 `-64..256`에 생성되는 철/석탄은 모두 **공기(또는 산악 중턱)에만 배치**되고, `-512..-400` 지하에는 바닐라 광물이 전혀 생기지 않는다!
   - **해결:** Clean-room 프로파일 빌드 시 **광물/지하 피처용 `placed_feature` 높이 오프셋 변환**(예: 오버월드 지하·지표 광물의 `Absolute(y)`를 `-464` 시프트하거나, `Y = -508..+400` 분포에 맞춘 Clean-room 광물 테이블 생성)을 반드시 포함해야 한다.
4. **카버 (`generated/carver.rs:87-170`):**
   - `CAVE`: `above_bottom: 8 .. absolute: 180`, `CAVE_EXTRA_UNDERGROUND`: `above_bottom: 8 .. absolute: 47`, `CANYON`: `absolute: 10 .. absolute: 67`.
   - `CAVE`가 `above_bottom: 8`(`-504`)부터 `180`까지는 커버하지만 `CANYON`(`10..67`)은 `-400` 지면 위 허공에서 생성된다. `CANYON`을 `-450..-390`으로, `CAVE_EXTRA_UNDERGROUND`를 `-504..-410`으로 재정의하는 프로파일 오버라이드가 필요하다.
5. **기타 절대 높이 하드코딩:**
   - `SurfaceTerrainBuilder::place_badlands_pillar`(`surface/terrain.rs:170`): `let elevation = 64.0 + ...` → `sea_level + 1.0` 기반 상대화 필요.
   - `World::load`(`crates/pumpkin-core/src/world/mod.rs:377`): `// TODO`와 함께 `NoiseSettings::from_dimension(&dimension)`을 직접 호출하여 `World.sea_level` 및 `World.min_y`를 설정함 → 프로파일의 `NoiseSettings` 및 `Dimension`을 읽도록 수정 필수.

---

## 3. Pumpkin `pumpkin-codegen` 및 런타임 소비 매핑

### 3.1 빌드 타임 → 런타임 데이터 파이프라인 매핑

Clean-room JSON 자산(`profiles/cleanroom_extreme_v1/data/minecraft/...` 등 분리된 소스 디렉터리)은 기존 `tools/pumpkin-codegen`을 통해 `crates/pumpkin-data/src/generated/`의 **별도 프로파일 상수**로 컴파일되며, 기본 바닐라 상수(`OVERWORLD`, `OVERWORLD_BASE_NOISE_ROUTER`)를 덮어쓰지 않는다.

| Clean-room 입력 자산 | `pumpkin-codegen` 모듈 | 생성 Rust 파일 (`pumpkin-data/src/generated/`) | 런타임 소비자 |
|---|---|---|---|
| `dimension_type/overworld.json` (오버라이드) | `dimension.rs` + `registry.rs` | `dimension.rs` (`Dimension::OVERWORLD_CLEANROOM_EXTREME` 신규), `registry.rs` (`CRegistryData` 동기화 바이트) | `Server::new` (`server/mod.rs:274`), `DynamicRegistryManager`, `Level::from_root_folder` |
| `worldgen/noise_settings/cleanroom_extreme.json` | `noise_settings.rs` | `noise_settings.rs` (`NoiseSettings::CLEANROOM_EXTREME`, `from_name("cleanroom_extreme")`) | `VanillaGenerator::new_with_all_settings` (`generator/mod.rs:198`), `World::load` (`world/mod.rs:377`), `AquiferSampler` |
| `worldgen/noise_router` + `density_function/*.json` | `noise_router.rs` | `noise_router.rs` (`CLEANROOM_EXTREME_BASE_NOISE_ROUTER`, `BaseNoiseRouters::from_name`) | `VanillaGenerator` (`generator/mod.rs:208`), `GlobalStructureCache` (`placement.rs:381,530`), `SurfaceHeightEstimateSampler` |
| `worldgen/material_rule/cleanroom_extreme.json` + `material_condition/*.json` | `material_rule.rs` | `material_rule.rs` (`MaterialRule::CLEANROOM_EXTREME`, `from_name`) | `VanillaGenerator` (`generator/mod.rs:201`), `ProtoChunk::build_surface` (`proto_chunk.rs:1202`), `CarvingContext` |
| `worldgen/noise/*.json` | `noise_parameter.rs` | `noise_parameter.rs` (바닐라 노이즈만 재사용 시 **변경 0**) | `GlobalRandomConfig` / `NoiseEvaluationContext` |
| `worldgen/biome/*.json` | `biome.rs` | `biome.rs` (**변경 0** — 바닐라 ID `u8` 및 `multi_noise_biome_tree.json` 유지) | `ActiveBiomeSupplier`, `ProtoChunk` |
| `worldgen/configured_carver/*.json` & `placed_feature/*.json` *(2단계 확장)* | `carver.rs`, `placed_feature.rs` | `carver.rs`, `placed_features_generated.rs` (프로파일별 또는 오버라이드 맵) | `ProtoChunk::carve`, `ProtoChunk::generate_features` |

### 3.2 `noise_router.rs` 경로 해석 주의점

현재 `tools/pumpkin-codegen/src/noise_router.rs:2229-2244`의 `load_df_json`과 `load_vanilla_noise_routers`(`2983-3005`)는 `../../assets/datapack/data/minecraft/worldgen/...` 경로와 `minecraft:` 접두사만 하드코딩으로 읽는다.
- **Clean-room 확장 방식:**
  1. Clean-room 오버레이 디렉터리(예: `profiles/cleanroom_extreme_v1/data/minecraft/...`)를 바닐라 `assets/datapack/data/minecraft` 위에 **가상 오버레이로 조회**(`overlay_dir` 우선, 없으면 `vanilla_dir` fallback)하는 헬퍼를 `noise_router.rs` / `noise_settings.rs` / `material_rule.rs` / `dimension.rs`에 추가한다.
  2. 오버레이 내부의 Clean-room JSON은 `minecraft:` 네임스페이스 경로(예: `minecraft:cleanroom_extreme/...`)를 사용하되 바닐라 파일(`overworld.json`)을 직접 덮어쓰지 않고 별도 이름(`cleanroom_extreme`)으로 등록한다.
  3. 이를 통해 `BaseNoiseRouters::from_name("cleanroom_extreme")`, `NoiseSettings::from_name("cleanroom_extreme")`, `MaterialRule::from_name("cleanroom_extreme")`가 **바닐라 `overworld`와 완전히 병존**하게 된다.

---

## 4. 프리셋 활성화 전략 (`enabled=false` 가드, 영속화, 기존 월드 호환)

### 4.1 설정 스키마 및 `enabled = false` 기본값 가드

Phase 5(`5e87898`)에서 추가된 `crates/pumpkin-config/src/lib.rs:188-213`의 `ExperimentalConfiguration` / `ExperimentalWorldgen` 구조를 그대로 활용하되, `profile` 값으로 프로파일을 구분한다.

```toml
# advanced.toml (기본값)
[experimental.worldgen]
enabled = false
profile = "cleanroom_extreme_v1"   # 선택 가능: "cleanroom_extreme_v1" (신규), "terralith_incendium_extreme_v1" (보류/미완성)
```

- **`enabled = false` (기본값):**
  - 기존 바닐라 `Dimension::OVERWORLD` (`-64..319`), `NoiseSettings::OVERWORLD`, `OVERWORLD_BASE_NOISE_ROUTER`, `MaterialRule::OVERWORLD` 경로를 100% 그대로 탄다.
  - 서버/월드 동작에 어떤 변화도 주지 않는다.
- **`enabled = true`:**
  - `Server::new` (`crates/pumpkin-core/src/server/mod.rs:181-186`)의 무조건 실패 가드를 **프로파일 레지스트리 조회**로 교체:
    - `"cleanroom_extreme_v1"` → 컴파일된 프로파일 디스크립터(`Dimension`, `settings_name = "cleanroom_extreme"`, `min_y = -512`, `height = 1024`, `sea_level = -400`, `dimension_type_nbt`) 활성화.
    - 미구현 프로파일(예: `"terralith_incendium_extreme_v1"`) → 기존처럼 `WorldInfoError::UnavailableWorldgenProfile` 반환 후 즉시 중단.

### 4.2 프로파일 영속화 및 기존 월드 보호 (Fail-Fast Guard)

`crates/pumpkin-core/src/server/mod.rs:203-245`에서 `AnvilLevelInfo.read_world_info(&world_path)` 직후, **`Level::from_root_folder` 및 청크 I/O 이전에** 아래 순서로 검증한다:

1. **영속화 파일 (`<world>/pumpkin_worldgen_profile.json` + `level.dat` 정합):**
   ```json
   {
     "profile": "cleanroom_extreme_v1",
     "version": 1,
     "overworld": {
       "min_y": -512,
       "height": 1024,
       "logical_height": 1024,
       "sea_level": -400,
       "noise_settings": "pumpkin:cleanroom_extreme"
     },
     "definition_sha256": "<빌드 시 산출된 프로파일 정의 해시>"
   }
   ```
   - 또한 새 월드 생성 시 `WorldGenSettings`(`crates/pumpkin-world/src/world_info/mod.rs:442`)의 `minecraft:overworld` `generator.settings`에 `GeneratorSettings::Reference("cleanroom_extreme")`를 기록하여 `Level::from_root_folder`가 재시작 시에도 해당 설정 이름을 읽을 수 있게 한다.
2. **시작 시 4가지 분기 매트릭스:**

| 월드 상태 | `experimental.worldgen.enabled` | 동작 |
|---|---|---|
| **신규 월드** (`level.dat` 없음) | `false` | 바닐라 월드 생성 (프로파일 메타 미생성 또는 `"vanilla"` 표기) |
| **신규 월드** (`level.dat` 없음) | `true` (`cleanroom_extreme_v1`) | `-512..511` 차원 + `cleanroom_extreme` 생성기로 새 월드 초기화 및 `pumpkin_worldgen_profile.json` 저장 |
| **기존 바닐라 월드** (`level.dat` 있음, 프로파일 파일 없음) | `false` | 정상 바닐라 로드 |
| **기존 바닐라 월드** (`level.dat` 있음, 프로파일 파일 없음) | `true` | **즉시 시작 거부** (`WorldInfoError` 반환: 기존 `-64..319` 월드에 `-512..511` 적용 금지, 자동 마이그레이션 미지원) |
| **기존 Clean-room 월드** (`pumpkin_worldgen_profile.json` 있음) | `false` 또는 다른 `profile`/해시 불일치 | **즉시 시작 거부** (24섹션 바닐라 청크가 64섹션 월드에 덮어써지는 청크 경계 단절·데이터 손상 방지) |
| **기존 Clean-room 월드** | `true` (일치) | 정상 로드 및 `DynamicRegistryManager`에 `-512..511` `dimension_type` 재주입 |

3. **클라이언트 동기화 영속 정합:**
   - Java 클라이언트: `Server::new` 시 `DatapackManager.dynamic_registry_manager`의 `minecraft:dimension_type` / `"minecraft:overworld"` 엔트리를 프로파일의 `-512..511` NBT로 갱신해 `CRegistryData`(`known_packs.rs:9`)로 전송한다.
   - Bedrock 클라이언트: `crates/pumpkin-core/src/net/bedrock/level_chunk.rs:93`는 `min_y_section = (-512 >> 4) as i8 = -32`로 이미 범위 내(`-128..127`)이며, 로그인/StartGame 패킷의 오버월드 높이 전달 여부 및 실제 클라이언트 수용은 `NOT TESTED`.

---

## 5. 데이터팩 (Terralith / Incendium)과의 관계 정리

| 대상 | Clean-room 프리셋과의 관계 | 상세 설명 |
|---|---|---|
| **Terralith (오버월드)** | **완전 대체 (상호 배타)** | 오버월드 `noise_settings`·`density_function`·`material_rule`을 Clean-room 프리셋이 전담한다. Terralith(커스텀 바이옴 95·피처 505·구조물 29)와 JJThunder(ARR 라이선스)를 모두 사용하지 않으므로 저작권·코드젠 부채를 해소한다. |
| **Incendium (네더)** | **공존 가능 (차원 분리)** | 네더(`minecraft:the_nether`, `0..256`, `logical_height=192`)는 오버월드와 완전히 분리되어 있다. Clean-room 오버월드 프리셋은 네더를 건드리지 않으므로, 향후 Incendium 네더 통합(또는 바닐라 네더 유지)과 충돌 없이 조합 가능하다. 단, 현재 `worldgen_profile.rs`처럼 단일 스테이징에 `Terralith + Incendium`을 강제 결합하지 말고 **오버월드 프로파일과 네더 옵션을 분리**해야 한다. |
| **일반 데이터팩** (함수·레시피·루트테이블·태그·발전과제 등) | **독립 공존 (런타임 `DatapackManager`)** | `<world>/datapacks/<pack_dir>`에서 `DatapackManager::load_all`이 계속 로드한다. 단, Phase 2 정적 감사에서 확인된 Pumpkin 제약(ZIP 팩 미지원, `#minecraft:load` 객체 엔트리 무시, 동적 worldgen JSON은 생성기에 미연결 등)은 그대로 적용된다. |
| **런타임 worldgen 데이터팩 덮어쓰기** | **무시됨 (의도된 분리)** | 사용자가 `<world>/datapacks/`에 Terralith 등을 넣더라도 Pumpkin 생성기는 컴파일된 Clean-room/바닐라 상수만 사용한다(`docs/PHASE2.md` §1). 단, `CRegistryData`로 비동기화 레지스트리가 클라이언트로 전송되는 부작용을 막기 위해 Clean-room 운영 시 월드 `datapacks/`에 타 오버월드 지형 팩을 동시 배치하지 않도록 문서화·경고한다. |

---

## 6. Phase 5b (`5e87898` / 워크트리 `613259c2...`) 정합성 검토

`git worktree list` 및 `git log` 확인 결과, Codex 세션 `613259c2fd744fa39ee749476530a052`의 작업 결과는 커밋 `5e87898` (`Phase 5 (partial): Y=-512..511 64-section chunk support...`)로 저장소 `main` 히스토리에 이미 반영되어 있다. Clean-room 프리셋 설계가 이 기반 위에 충돌 없이 올라타는지 정적 검토한 결과는 다음과 같다.

### 6.1 이미 반영되어 그대로 활용 가능한 기반 (`5e87898` 정합 PASS)

1. **`i16` 최소 Y 전달 경로 (`noise_settings.rs`, `proto_chunk.rs`, `carver/*`, `feature/*`, `height_provider.rs`, `aquifer_sampler.rs`):**
   - `GenerationShapeConfigStruct.min_y` 및 `GenerationShapeConfig.min_y`가 `i8` → `i16`으로 확장되었고 `trim_height`도 `i16`을 받는다. `-512` 표현이 완전히 가능하다.
2. **64섹션 청크·11비트 하이트맵·직렬화 (`chunk/mod.rs`, `chunk/format/mod.rs`):**
   - `height = 1024`일 때 `Y - min_y + 1` 인코딩(`0..1024`)을 11비트(long당 5값, 총 52 `i64` long)로 패킹하며, `PCHK` 버전 2 포맷으로 비트 폭을 저장하고 기존 버전 1(37 long)과 양립한다.
   - 랜덤 틱 마스크와 `ChunkSections::from_palettes`도 64섹션을 수용한다.
3. **Java 64섹션 조명 인코딩 (`net/java/chunk_data/light.rs`):**
   - `encode_light_layer`가 `(section_count + 2).div_ceil(64)` = 2개 `u64` 워드(비트 0..65)를 할당하여 섹션 63(비트 64)과 상한 경계(비트 65)를 왕복 인코딩한다(오프라인 유닛 테스트 통과 기록 존재).
4. **안전 가드 설정 틀 (`crates/pumpkin-config/src/lib.rs`, `crates/pumpkin-core/src/server/mod.rs`):**
   - `ExperimentalWorldgen { enabled: bool, profile: String }` (기본값 `enabled = false`)과 `Server::new` 시작 전 실패 가드(`WorldInfoError::UnavailableWorldgenProfile`)가 이미 구현되어 있다. Clean-room 프리셋은 이 가드에 프로파일 분기만 추가하면 된다.

### 6.2 Phase 5b와 Clean-room 설계 간 조정·보완이 필요한 지점

| 항목 | 현재 Phase 5b(`5e87898`) 상태 | Clean-room 정합을 위한 조정 사항 |
|---|---|---|
| **`worldgen_profile.rs` 스테이징 결합** | `PROFILE = "terralith_incendium_extreme_v1"` 단일 고정, `Terralith`·`Incendium` 폴더 및 해당 기후 바이옴 존재 여부를 필수 검증(`worldgen_profile.rs:10, 14-18, 63-71`) | `cleanroom_extreme_v1` 모드 추가 또는 서브커맨드 인자(`--profile cleanroom_extreme_v1`)로 분리해 **Terralith/Incendium 없이** 바닐라 + Clean-room JSON만으로 스테이징/코드젠 가능하게 변경 |
| **`ExperimentalWorldgen::default().profile`** | 기본 문자열이 `"terralith_incendium_extreme_v1"` (`pumpkin-config/src/lib.rs:210`) | 기본 프로파일 문자열을 `"cleanroom_extreme_v1"`로 전환하거나, 지원 프로파일 목록을 문서·검증기에 명시 (`enabled=false`이므로 즉시 영향은 없음) |
| **용암 수위·대수층 Y 경계** | `aquifer_sampler.rs:91,129`, `proto_chunk.rs:816`에 `FluidLevel::new(-54, &Block::LAVA)` 및 `aquifer_sampler.rs:614`의 `level <= -10` 하드코딩 잔존 | 프로파일별 `lava_level`(`-502`) 주입 필요 (바닐라 모드와 분리) |
| **광맥·광물·카버 절대 Y** | `ore_sampler.rs:71-86`, `surface/mod.rs:88-102`, `proto_chunk.rs:1217-1219`에 `-60..-8`, `0..50` 하드코딩; `placed_features_generated.rs`와 `carver.rs`가 바닐라 절대 Y 고정 | Clean-room 프로파일에서 `-512..-400` 지하 구간용 광맥/광물/카버 Y 오프셋 또는 오버라이드 필수 |
| **차원/네트워크 동기화** | `Dimension::OVERWORLD`가 `-64/384/384` 고정, `World::load`(`world/mod.rs:377`)가 `NoiseSettings::from_dimension` 직접 호출, `GlobalStructureCache`(`placement.rs:381,530`)가 `BaseNoiseRouters::from_dimension` 직접 호출 | 프로파일 활성 시 `Dimension`, `NoiseSettings`, `BaseNoiseRouters`, `MaterialRule`, `DynamicRegistryManager`를 모두 프로파일 선택값으로 일원화해야 함 |
| **빌드 시 바닐라 추출 데이터 의존** | `5e87898` 워크트리는 바닐라 추출 데이터(`assets/datapack`)를 외부 체크아웃에서 복사해 스테이징(`PHASE5.md:62`). 오프라인 샌드박스에는 `assets/datapack` 미포함 | Clean-room 코드젠 구현 단계에서는 추출된 바닐라 `assets/datapack` 확보(오프라인 캐시 또는 사전 추출본)가 선행 조건 |

---

## 7. 구현 단계별 체크리스트 및 검증 범위 (모두 `NOT TESTED`)

1. **Step 1 — Clean-room 데이터 자산 작성:**
   - `dimension_type/overworld.json`, `noise_settings/cleanroom_extreme.json`, `density_function/cleanroom_extreme/*`, `material_rule/cleanroom_extreme.json` 작성.
   - 수학 단위 검증: `depth=0` 교차점이 평원 `-400`, 극단 산맥 `+400`, 심해 `-465` 범위에 들어오는지 오프라인 density 계산기로 수치 점검 (`NOT TESTED`).
2. **Step 2 — `pumpkin-codegen` & 스테이징 확장:**
   - `worldgen_profile.rs`, `noise_settings.rs`, `noise_router.rs`, `material_rule.rs`, `dimension.rs`에 오버레이 프로파일 생성 경로 추가 (`NOT TESTED`).
3. **Step 3 — 하드코딩 Y 경로 파라미터화:**
   - `AquiferSampler` 용암 수위(`-502`), `OreVeinSampler`/`VeinContext`/`proto_chunk` 광맥 범위, `World::load`, `GlobalStructureCache`, `shipwreck.rs` fallback, `badlands_pillar` 기준 고도, 광물/카버 Y 범위 보정 (`NOT TESTED`).
4. **Step 4 — 프로파일 영속화 & 가드 연결:**
   - `Server::new`에서 `cleanroom_extreme_v1` 허용, `pumpkin_worldgen_profile.json` 저장·검증, 기존 월드 충돌 차단, `DynamicRegistryManager` 동기화 (`NOT TESTED`).
5. **Step 5 — 오프라인 + 실서버 통합 검증 (`NOT TESTED`):**
   - `tools/world-verify`로 생성 청크 샘플링: `min_y=-512`, `max_y<=511`, 평원 중심 `-400 ± 25`, 산 정상 분포 `+300..+400`, 지하 동굴 `-507..-420`, 용암/딥슬레이트/광물 Y 분포, 재시작 후 청크·하이트맵 무결성 확인 (`NOT TESTED`).
   - Java 26.3 및 최신 Bedrock(내장 Bedrock + 외부 Geyser 프록시 A/B) 실접속, `-512..511` 렌더링·조명·블록 설치/파괴·이동·구름 높이 확인 (`NOT TESTED`).

> **최종 상태 표기:** 본 문서는 정적 코드 분석 기반의 설계 계획안 및 정합성 검토서이다. 서버 빌드·실서버 기동·실제 지형 생성·Java/Bedrock 클라이언트 접속 검증은 모두 수행하지 않았으며 **`NOT TESTED`** 이다.
