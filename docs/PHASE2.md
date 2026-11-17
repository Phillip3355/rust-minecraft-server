# Phase 2 — 데이터팩 런타임 점검 (진행 기록)

- 기준 문서: `Minecraft-Rust-Server-Plan.md` §7 Phase 2
- 대상: Pumpkin `0.2.2+26.3-26.60` @ `eda69440` — 저장소에 벤더링된 `reference/pumpkin-src` (956857a)
- 대상 데이터팩: `reference/datapacks/Terralith` (2.6.5+26.3), `reference/datapacks/Incendium` (5.5.4+26.3)
- 작성일: 2026-11-17 · 환경: Linux 7.0.0-28-generic, rustc/cargo 1.100.0 (offline)
- 산출물: 이 문서, `docs/phase2/MISSING.md`(누락·파싱 실패 목록), `docs/phase2/audit.json`(전체 위치 포함 기계 판독본), `docs/phase2/smoke-audit.md`, `tools/datapack-audit/`, `tests/datapacks/phase2-smoke/`, `docs/validation/phase2-datapack-audit-tests.log`

## 1. 상태 요약 (정직 표기)

| 항목 | 상태 | 근거 |
|---|---|---|
| 1. `assets/datapack`·Dynamic Registry Manager·런타임 로더 범위 점검/문서화 | **완료 (소스 정적 분석)** | §2, §3. 모든 주장에 소스 위치 표기 |
| 2. ZIP/폴더 로딩, 로드 우선순위, 오류 진단 로그 확인 | **완료 (소스 정적 분석)** | §2.3–§2.5 |
| 3. function·tag·recipe·loot table·advancement 지원 여부 | **소스 기준 판정 완료 / 런타임 미검증** | §3 표. 실서버 실행 결과 아님 |
| 4. Terralith/Incendium 파싱 실패·누락 항목 목록화 | **완료 (정적 감사 도구로 전수 검사)** | §4, `docs/phase2/MISSING.md`, `audit.json` |
| 감사 도구 `tools/datapack-audit` | **구현 + 테스트 통과** | `cargo test --offline`: 단위 4 + 통합 16 = **20/20 통과** (`docs/validation/phase2-datapack-audit-tests.log`) |
| 대표 일반 데이터팩 검증 절차 | **절차·테스트 팩 작성 완료 / 실행 미검증** | §5. 스모크 팩은 정적 감사에서 FAIL/WARN 0 (`docs/phase2/smoke-audit.md`) |
| Pumpkin 서버에서 데이터팩 실제 로드/실행 | **미검증** | 이 워커 세션은 `~/.hermes/cache/scratch/pumpkin-src`와 빌드된 바이너리에 접근 권한이 없음(경로 접근 차단). 사용자 보고(빌드·기동 성공)는 이 문서에서 재확인하지 않았음 |

핵심 결론:

1. **Pumpkin의 월드 생성은 컴파일 타임에 고정된다.** density function·noise·noise_settings·biome·configured/placed feature·carver·structure·structure_set·world_preset·dimension(_type)은 `tools/pumpkin-codegen`이 빌드 시 바닐라 JSON에서 Rust 코드로 생성한다. 런타임 데이터팩의 해당 JSON은 원시 NBT로 동적 레지스트리에 저장되어 **클라이언트로 전송될 뿐 생성기에 쓰이지 않는다.** 따라서 Terralith/Incendium의 지형·바이옴·피처·구조물은 현재 Pumpkin에서 **전혀 적용되지 않는다.** (실패 로그 없이 조용히 무시됨)
2. 런타임에 실제로 동작하는 데이터팩 월드젠 요소는 **structure NBT 템플릿, template_pool, processor_list** 세 가지뿐이며, 이것도 컴파일된 바닐라 구조물이 참조할 때만 쓰인다(바닐라 ID 오버라이드 용도).
3. **두 팩 모두 함수 진입점이 Pumpkin에서 끊긴다.** Terralith의 `#minecraft:load`는 `{id, required}` 객체 항목만 가지며(Pumpkin은 문자열 항목만 보존), Incendium은 Lantern Load 규약의 중첩 함수 태그(`#load:_private/load`)를 쓴다(Pumpkin은 중첩 태그를 펼치지 않음). 정적 도달성 분석: **Incendium 2,314개 함수 중 바닐라 기준 2,117개가 load/tick에서 도달 가능, Pumpkin 기준 0개.** Terralith 3개 중 바닐라 2개, Pumpkin 0개.
4. JSON 문법 오류·NBT 손상은 0건. 즉 "파싱 실패"의 실체는 **문법이 아니라 Pumpkin 쪽 미지원 타입·닫힌(enum) 레지스트리·미구현 소비 경로**다. 전 항목은 §4와 `MISSING.md`에 있다.

## 2. Pumpkin 데이터팩 런타임 구조 (소스 점검 결과)

경로는 `reference/pumpkin-src/crates/` 기준.

### 2.1 `assets/datapack` (요청서의 `assets/datapacks/`)

- 실제 경로는 단수형 **`assets/datapack/`** 이다. 저장소에 커밋돼 있지 않고 `pumpkin-data/build.rs`가 빌드 시 26.3 서버 jar를 내려받아 `assets/datapack/data/minecraft/...`로 추출한다(`ensure_datapack`/`extract_datapack`, conventional tags를 `data/c`로 복사). 벤더링 사본 `reference/pumpkin-src/assets/`에는 이 디렉터리가 **없다**(빌드 전 상태).
- 이 바닐라 데이터는 (a) `tools/pumpkin-codegen`이 월드젠/레지스트리/루트 테이블 등을 Rust로 생성하는 입력, (b) `pumpkin-world/src/world_info/mod.rs`의 world preset `include_str!`, (c) 번들 기능 팩 `assets/datapack/data/minecraft/datapacks/{trade_rebalance,minecart_improvements,redstone_experiments}`(`pumpkin-core/src/data/datapack/mod.rs::find_bundled_datapack_dir`, CWD·실행 파일 상위 5단계까지 탐색)로 쓰인다.

### 2.2 로드 흐름 — `DatapackManager::load_all` (`pumpkin-core/src/data/datapack/mod.rs`)

1. 서버 시작 시 **모든 월드 로드 후** 호출(`server/mod.rs` 466–474행), 이어서 `#minecraft:load` 실행. `/reload`, `/datapack enable|disable` 시 `Server::reload_datapacks`로 재호출 후 `#minecraft:load` 재실행 및 레시피북 재전송.
2. `structure_loader::clear_dynamic_worldgen_data()`로 동적 템플릿/풀/프로세서 초기화.
3. 내장 test instance·context provider 로드 → `scan_datapacks_dir(<world>/datapacks)` → `load_bundled_datapacks`.
4. 팩별 `load_pack_contents`: 네임스페이스마다 `recipe(s)`, `function(s)`, `tags`(함수 태그 + 일반 태그 맵), `test_instance`, `damage_type(s)`, `context_{int,float}_provider(s)`, `loot_table(s)`, `advancement(s)`, `predicate(s)`, `item_modifier(s)`, `slot_source(s)`, `structure(s)`(.nbt), `worldgen/template_pool`, `worldgen/processor_list`, 동적 레지스트리, `trade_set(s)`, `villager_trade(s)`, `tags/villager_trade(s)`.
5. 누적 결과를 각 `RwLock`에 교체 저장, `dynamic_registry_manager.apply_custom_registries` 호출.

### 2.3 Dynamic Registry Manager (`dynamic_registry_loader.rs`, `dynamic_registry_manager.rs`)

- `DYNAMIC_REGISTRIES` 표에 있는 폴더와, 그 밖의 **모든 미지정 폴더를 자동 발견**(`EXCLUDED_REGISTRY_DIRS` 제외, `worldgen/` 하위는 `<ns>:worldgen/<dir>`로)하여 JSON → NBT 바이트(`json_to_nbt_tag`)로 저장한다. 내용 검증은 없다(파싱 불가 JSON은 조용히 건너뜀).
- 병합: 바닐라 엔트리 순서·네트워크 ID 유지, 같은 ID는 덮어쓰기, 신규는 사전순으로 뒤에 추가. **기반 동기화 집합(`REGISTRY_V_26_3`)에 없는 레지스트리는 새 동기화 레지스트리로 추가**되며 `net/java/login/known_packs.rs::handle_known_packs`가 **모든 Java 클라이언트에 `CRegistryData`로 전송**한다. 즉 Terralith를 켜면 `minecraft:worldgen/density_function`, `…/feature`, `…/noise`, `…/structure` 등 14개 비동기화 레지스트리의 전체 JSON이 접속 시마다 클라이언트로 간다(클라이언트 반응·패킷 크기 영향은 **미검증**).
- 태그(`CUpdateTags`)는 `pumpkin_data` 정적 태그만 보낸다 → 데이터팩 태그는 클라이언트에 반영되지 않음.
- 서버 측 소비: 동적 레지스트리 raw ID 조회(엔티티 변형 등)와 damage_type. **worldgen 레지스트리는 서버에서 소비처가 없다.**

### 2.4 ZIP/폴더 로딩과 활성화

| 동작 | Pumpkin | 바닐라 대비 |
|---|---|---|
| 폴더 팩 | 로드 | 동일 |
| ZIP 팩 | **로드 안 됨.** `get_all_known_packs`는 `.zip`을 `file/<name>.zip`으로 목록에 올리지만 `scan_datapacks_dir`는 `!pack_path.is_dir()`면 건너뜀 → 활성화해도 내용 0, 경고 없음 | 차이(치명) |
| 새 팩 자동 활성화 | **없음.** `level.dat`의 `DataPacks.enabled`에 있는 것만 로드(기본값 `["vanilla"]`) → `datapack enable "file/<폴더명>"` 필요 | 바닐라는 시작 시 새 팩 자동 활성화 |
| `pack.mcmeta` | description(문자열이 아니면 JSON 문자열 그대로), `pack_format` → `max_format` → `min_format` 순, 정수가 아니면 61. `overlays`·`supported_formats`·포맷 호환성 검사 없음 | 포맷 검사·오버레이 없음 |
| 동시 다중 팩 | 지원 | — |

### 2.5 로드 우선순위

- `datapack enable … first|last|before|after`는 `level.dat`의 순서만 바꾼다. 실제 로드는 `scan_datapacks_dir`가 **`fs::read_dir` 반환 순서(파일시스템 순서, 정의되지 않음)** 로 돌며 `enabled`에 포함 여부만 본다 → **바닐라의 "목록 뒤쪽 팩이 이긴다" 규칙이 적용되지 않는다.** 같은 ID 충돌 시 결과는 맵 종류별로 다르다:
  - HashMap 덮어쓰기(함수, 루트 테이블, 발전과제, predicate, 동적 레지스트리, 템플릿 풀, 프로세서): 나중에 읽힌 팩이 이김.
  - 레시피: `Vec`에 누적 → **같은 ID가 중복 등록**.
  - 태그: `replace:false`는 합집합, `replace:true`는 그 시점까지만 교체(순서 의존). 함수 태그는 `replace` 자체를 무시.
  - 구조물 NBT 조회(`load_structure`)는 로드 역순 탐색.
- 번들 기능 팩은 항상 디렉터리 팩 뒤에 로드.

### 2.6 오류 진단 로그 정비 상태

| 대상 | 실패 시 로그 | 비고 |
|---|---|---|
| `datapacks/` 디렉터리 읽기 | `warn!` | |
| 팩 단위 요약 | `info!` "Loaded datapack '<name>': N recipe(s) …" | item_modifier·slot_source·trade_tag 수는 출력 안 됨 |
| pack.mcmeta | **없음** (기본값 대체) | |
| recipe / function / tag / loot_table / predicate / item_modifier / slot_source / damage_type / 동적 레지스트리 | **없음** — JSON 오류·미지원 타입 모두 조용히 건너뜀 | 진단 공백의 핵심 |
| advancement | `warn!` (JSON 파싱 실패만) | |
| context provider, trade | `warn!` | |
| structure .nbt | 읽기 실패 `error!`; NBT 파싱 오류는 사용 시점까지 지연 | |
| template_pool / processor_list | `error!` + serde 메시지 | 런타임 월드젠 중 유일하게 원인 로그 있음 |
| 함수 내 명령 실패 | 디스패처가 콘솔 소스로 오류 반환(로그 형식 **미검증**) | `return`은 이후 줄을 멈추지 않음, 재귀 깊이 제한 없음(`visit_function_lines`) |

→ Phase 2 결론: **로드 실패 진단은 대부분 부재**하므로 서버 로그만으로는 Terralith/Incendium의 실패를 알 수 없다. 이 공백을 `tools/datapack-audit`로 대신 메운다.

## 3. 콘텐츠 종류별 지원 현황 (요청 3)

판정은 소스 기준(정적). "런타임"은 실제 서버 실행 결과가 아니라 소스상 소비 경로 유무다. **실서버 실행 검증은 전 항목 미검증.**

| 종류 | 로드 | 런타임 소비 | 주요 제한 (소스 근거) |
|---|---|---|---|
| function | O | O — `#minecraft:load`(시작·리로드), `#minecraft:tick`(매 틱, `server/mod.rs:1276`), `/function`, `/schedule function`, enchantment `run_function` | 매크로(`$`) 미지원, `\` 줄 이어쓰기 미지원, `return`으로 조기 종료 안 됨, 재귀 제한 없음 |
| tag (function) | O | 부분 | 문자열 항목만 보존(`{id,required}` 객체 버림), 중첩 `#tag` 미전개, `replace` 무시 |
| tag (block/item/entity/biome …) | O | **X** | `DatapackManager::is_in_tag/get_tag_values`의 게임플레이 호출처 없음 → 게임 로직은 정적 태그 사용, 클라이언트도 정적 태그만 수신 |
| recipe | O | O (제작대·석재절단기·대장장이·화로·양조·지식의 책) | 지원 14종 + `crafting_special_*`; 그 외 조용히 무시; 같은 ID 중복 가능 |
| loot_table | O | 부분 — `World::get_loot_table`이 데이터팩 우선 | 함수 8종만 지원(`set_count`, `apply_bonus`, `enchanted_count_increase`, `limit_count`, `explosion_decay`, `furnace_smelt`, `set_stew_effect`, `looting_enchant`) — 나머지(`set_components`, `set_name`, `enchant_*`, `set_damage` 등)는 조용히 제거; 미지원 조건 → 절대 드롭 안 함; 미지원 엔트리/롤 provider → 테이블 전체 폐기(정적 테이블로 대체); 일부 경로(`block/blocks/chests.rs:202`, `entity/vehicle/vehicle.rs:177`)는 데이터팩을 무시하고 정적 테이블 직접 조회 |
| predicate | O | 루트 테이블 조건에서만 | |
| advancement | O (정의 저장) | **X** | 플레이어 진행도·`/advancement`는 `pumpkin_data` 정적 발전과제만 사용 → 데이터팩 발전과제는 달성·보상 함수 실행 안 됨 |
| item_modifier / slot_source | O | **X** | getter 호출처 없음 |
| damage_type | O | O | `/damage` 인자 등록 |
| structure (.nbt) | O | O | `/place template`, jigsaw |
| worldgen/template_pool, processor_list | O | 부분 | 바닐라 ID 오버라이드만 의미 있음; 미지원 processor/predicate 타입 → 목록 전체 거부(`error!`) |
| 그 밖의 worldgen, dimension, dimension_type, biome | 원시 NBT로만 | **X (클라이언트 동기화만)** | §1 결론 1 |

## 4. Terralith / Incendium 감사 결과 (요청 4)

`tools/datapack-audit`가 두 팩의 모든 파일(JSON 2,860·mcfunction 2,317·NBT 564)을 Pumpkin 소스에서 추출한 지원 집합과 대조했다. 지원 집합은 하드코딩이 아니라 고정 커밋 소스의 match arm / serde rename에서 매번 추출한다(어디서 읽었는지 `MISSING.md` 말미에 기재).

### 4.1 요약

| 구분 | Terralith | Incendium |
|---|---|---|
| JSON 문법 오류 / NBT 손상 | 0 / 0 | 0 / 0 |
| 커스텀 바이옴 (컴파일 집합 밖) | 95 (+바닐라 39개 오버라이드: 서버 무시) | 8 (+5 오버라이드) |
| 커스텀 density function / 참조 깨짐 | 87 / 비-minecraft DF 참조 122 | 18 / 61 |
| 커스텀 noise / 참조 | 123 (terralith 116 + minecraft 네임스페이스 신규 7) / 39 | 54 / 34 |
| 커스텀 noise_settings | 2 (`all_skylands`, `amplified_large_biomes`) + 3 오버라이드 무효 | nether 오버라이드 무효 |
| configured / placed feature (닫힌 enum 밖) | 505 / 567 | 33 / 70 |
| carver | 1 (`deep_ravine`) | 1 (`big_cave`, 바이옴 10곳 참조) |
| structure / structure_set (닫힌 enum 밖) | 29 / 8 | 9 / 2 |
| template_pool 파싱 | 87 전부 통과(단, 커스텀 구조물이 없어 미사용) | 160 통과, 9개 `feature_pool_element`는 미컴파일 feature라 요소 제거 |
| processor_list 파싱 | 7 통과 | 50 중 **1 거부** (`heavy_damage`: `incendium:air_above` pos 규칙) |
| dimension / dimension_type 오버라이드 | overworld(무효) / `amplified_large_biomes`(클라이언트만) | the_nether(무효) / the_nether `min_y=0,height=256`(클라이언트만) |
| world_preset | 2 (컴파일 고정, 무효) | — |
| 함수 도달성 (load/tick 기준, 바닐라→Pumpkin) | 2 → **0** | 2,117 → **0** |
| 명령 루트 미등록 | 0 | 0 (35종 사용, 전부 등록됨 — 하위 인자 호환성은 미검증) |
| 매크로 줄 | 0 | 5 |
| 루트 테이블 | 함수 제거 159건(enchant_with_levels·set_damage 등) | 함수 제거 469건, 테이블 1개 폐기(`hovering_inferno/extras`: 롤 provider `div`) |
| 발전과제 | — | 60개 미소비 |
| 클라이언트로 새로 동기화될 비바닐라 레지스트리 | 14 | 13 |

### 4.2 Pumpkin에 없는 "타입" (Phase 4 구현 대상 중 코드 추가가 필요한 것)

닫힌 enum/컴파일 고정 문제와 별개로, 런타임 월드젠을 도입했을 때 **해석기 자체에 없는 타입**:

| 범주 | 누락 타입 | 위치 |
|---|---|---|
| Density Function | **없음** — 두 팩이 쓰는 DF 타입은 모두 `noise_router.rs::parse_vanilla_df`가 지원 | — |
| Carver | **없음** (두 팩 carver 타입은 `cave`/`canyon` 안; 참고로 codegen은 미지원 타입을 경고 없이 기본 cave로 대체) | — |
| Feature (최상위) | **없음** — 두 팩의 configured feature 타입은 75종 지원 목록 안 | — |
| Placement modifier | `minecraft:random_chance` | Terralith `placed_feature/highlands/forest/red_shrub.json` |
| 피처 내부 중첩 타입 | `minecraft:randomly_selected` | Terralith `feature/cave/fungal/floor.json` |
| Structure type / placement | **없음** (모두 지원 집합 16종/2종 안) — 문제는 ID enum이 닫혀 있다는 점 | — |
| Structure processor 규칙 | `incendium:air_above` (팩 고유 pos_rule_test — 바닐라에도 없음, 확인 필요) | Incendium `processor_list/heavy_damage.json` |
| Loot number provider | `div` | Incendium `loot_table/hovering_inferno/extras.json` |
| Loot function | `enchant_randomly`, `enchant_with_levels`, `set_components`, `set_damage`, `set_instrument`, `set_lore`, `set_name`, `set_potion`, `fill_player_head`, `set_attributes`, `set_custom_data`, `set_custom_model_data`, `set_enchantments`, `set_written_book_pages` | `MISSING.md` 표 |
| Surface rule / condition | **없음** | — |

즉 Phase 4의 주 작업은 "타입 추가"보다 **아키텍처 전환**이다: (1) DF·noise·noise_settings·biome·feature·carver·structure 레지스트리를 런타임 데이터 구동으로(또는 Terralith/Incendium을 codegen 입력에 추가해 빌드 타임 포함), (2) 바이옴 ID(`u8`, 현재 67개 → +103이면 170개로 범위 내), `StructureKeys`, `PlacedFeature`/`ConfiguredFeature` enum을 개방형으로, (3) multi-noise 파라미터 목록과 dimension/dimension_type을 데이터 기반으로. 사용자 결정(Extreme Worldgen −512..511, 지면 Y=−300, 산 최대 800)도 **dimension 높이가 컴파일 상수(`pumpkin-data/src/generated/dimension.rs`: overworld `min_y=-64, logical_height=384`)** 라 같은 경로의 변경이 필요하다.

### 4.3 데이터팩 외 즉시 수정 가능한 런타임 결함 (Phase 2 후속 후보, 구현 안 함)

1. 함수 태그: `{id, required}` 객체 항목 보존 + 중첩 `#tag` 재귀 전개 (두 팩의 진입점 복구; 바닐라 의미 확인 필요)
2. `scan_datapacks_dir`: ZIP 로드(또는 최소한 경고), `enabled` 목록 순서대로 로드
3. 로더 전반의 실패 로그(`warn!`) 추가
4. 비동기화 worldgen 레지스트리를 클라이언트 동기화 대상에서 제외

## 5. 대표 일반 데이터팩 검증 절차 (완료 조건 1)

### 5.1 테스트 팩 `tests/datapacks/phase2-smoke`

내용 종류당 관측 가능한 프로브 하나씩. 정적 감사 결과 **FAIL 0 / WARN 0** (INFO 1 = 발전과제 미소비, 의도된 대조군) — 따라서 런타임에서 실패하면 팩이 아니라 Pumpkin 쪽 원인이다. 회귀 테스트 `phase2_smoke_pack_is_statically_clean`이 이 상태를 고정한다.

### 5.2 실행 절차 (미검증 — 서버 접근 가능한 환경에서 수행)

1. 스크래치 디렉터리에서 Pumpkin 1회 기동→`stop` (월드 생성). 같은 시드로 Paper 26.3도 준비(Phase 0 `docs/BENCHMARK_EXECUTION.md` 러너 재사용) — 바닐라 기준선.
2. `cp -r tests/datapacks/phase2-smoke <world>/datapacks/phase2-smoke` (**폴더로**, ZIP 금지).
3. 서버 기동 후 콘솔: `datapack list` → `file/phase2-smoke`가 available에 있어야 함. `datapack enable "file/phase2-smoke"`.
4. 로그에서 `Loaded datapack 'phase2-smoke': 1 recipe(s), 6 function(s), … 1 damage type(s) … 1 loot table(s) … 1 advancement(s), 1 predicate(s), N tag(s)` 확인.
5. 아래 체크리스트 수행 후 `stop`, 로그 보관(`docs/validation/phase2-runtime-<date>.log`).
6. 같은 절차를 Paper에서 반복하고 결과 열을 비교. 이어서 `tools/world-verify verify <world>`로 월드 무결성(Phase 1 도구) 확인.

### 5.3 체크리스트

| ID | 조작 | 바닐라 기대 | Pumpkin 소스 기반 예측 | 실제 |
|---|---|---|---|---|
| P2-F1 | enable 직후/재시작 시 로그 | `[phase2] P2-F1 load ok` | 출력됨 | 미검증 |
| P2-F2 | 5초 후 `scoreboard players get #tick p2` | ≈100 | 증가 | 미검증 |
| P2-F3 | `function phase2:probe` | start/end 출력 | 출력됨 | 미검증 |
| P2-T1 | (probe 내부) `function #phase2:chain` | `P2-T1 function tag ok` | 출력됨(문자열 항목) | 미검증 |
| P2-F4 | (probe 내부) `execute run function phase2:nested` | `P2-F4 … ok` | 출력됨 | 미검증 |
| P2-R1 | 플레이어가 흙 3개 무형 조합 | 다이아몬드 1 | 성공 | 미검증 |
| P2-L1 | `loot give <player> loot phase2:probe` | 에메랄드 3 (`reference`→`phase2:always` predicate) | 에메랄드 3 | 미검증 |
| P2-D1 | `damage <player> 1 phase2:probe` | 피해 1 | 성공 | 미검증 |
| P2-G1 | 돌 위에서 `execute as <player> at @s if block ~ ~-1 ~ #phase2:probe_blocks run say hit` | `hit` | **실패 예상**(데이터팩 블록 태그 미소비) | 미검증 |
| P2-A1 | 플레이어 접속 | `P2-A1 advancement reward ran` + 토스트 | **출력 안 됨 예상**(발전과제 미소비) | 미검증 |
| P2-X1 | `reload` | P2-F1 재출력, 레시피 유지 | 재출력 | 미검증 |
| P2-X2 | 팩을 ZIP으로 바꿔 enable | 동작 | **로드 0 예상, 경고 없음** | 미검증 |

Terralith/Incendium 실측도 같은 절차로: 팩 enable → 로그의 개수 요약을 `docs/phase2/audit.json` inventory와 대조 → 새 청크에서 `locate biome terralith:…`(실패 예상), F3 바이옴 표시(클라이언트는 커스텀 바이옴 등록을 받지만 서버 청크는 바닐라 바이옴) → `function incendium:load` 수동 실행(진입점 우회 시 실제 명령 호환성 확인용).

## 6. 수행한 검증 / 미검증

수행(실제 실행):
- `cargo test --offline` (`tools/datapack-audit`): 20/20 통과 — JSON 파서 엄격성(serde_json과 동일 거부 사례), Rust 소스 추출기, 고정 소스에서의 지원 집합 추출(DF·구조물·명령·루트·레지스트리 표), 합성 팩 10종(각 Pumpkin 동작 하나씩), 스모크 팩 청정성. 로그: `docs/validation/phase2-datapack-audit-tests.log`.
- `datapack-audit` 실행: Terralith, Incendium, phase2-smoke → `docs/phase2/{MISSING.md,audit.json,smoke-audit.md}`.
- 감사 중 발견·수정한 도구 결함: (1) `data/c/worldgen/*.json`(worldgen 바로 아래 파일)을 동기화 레지스트리로 오탐 → Pumpkin은 하위 폴더만 읽으므로 제외; (2) 인라인 placed feature(`{feature, placement}`)의 `feature`를 placed 참조로 오분류해 "미해결 참조" 74건 오탐 → configured 참조로 수정; (3) 루트 provider `{"type":"constant","value":n}`를 미지원으로 오판 → `parser.rs::scalar`대로 허용. 각각 회귀 테스트 추가.

미검증:
- Pumpkin 실서버에서의 로드·실행 전부(§5.3 "실제" 열). 이 세션에서는 Python 실행과 저장소 밖 경로(`~/.hermes/...`, 빌드된 바이너리) 접근이 허용되지 않았다.
- 비동기화 레지스트리 전송에 대한 26.3 클라이언트 반응, 함수 명령의 하위 인자 단위 호환성(`execute` 하위 명령, `data modify storage`, `item modify` 등).
- 감사 도구의 한계: (a) 피처 내부 중첩 타입은 여러 범주 합집합으로 판정(휴리스틱); (b) 정적 분석이므로 Pumpkin이 지원 타입을 "잘못" 처리하는 경우는 잡지 못함; (c) 팩 간 상호 참조(두 팩 동시 활성화)는 팩 단위로만 분석.
- `incendium:air_above`, 롤 provider `div`, placement `random_chance`가 바닐라 26.3에 존재하는지는 바닐라 jar를 열지 못해 확인하지 않음(팩 쪽 고유/신규 타입일 수 있음).

## 7. 재현

```bash
cargo test --offline --manifest-path tools/datapack-audit/Cargo.toml
cargo run --offline --release --manifest-path tools/datapack-audit/Cargo.toml -- \
  --pumpkin-src reference/pumpkin-src \
  --pack reference/datapacks/Terralith --pack reference/datapacks/Incendium \
  --json-out docs/phase2/audit.json --md-out docs/phase2/MISSING.md
cargo run --offline --release --manifest-path tools/datapack-audit/Cargo.toml -- \
  --pumpkin-src reference/pumpkin-src --pack tests/datapacks/phase2-smoke --md-out docs/phase2/smoke-audit.md
```

`--pumpkin-src`를 다른 Pumpkin 커밋으로 바꾸면 지원 집합이 그 커밋에서 다시 추출되므로, 업스트림 갱신 시 같은 명령으로 누락 목록을 재생성할 수 있다(추출 대상 함수/enum 이름이 바뀌면 exit 2로 실패).
