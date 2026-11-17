# Phase 4 — Pumpkin 월드젠 구조와 데이터팩 적용 설계

상태: **정적 분석, 실행 미검증**. 대상은 vendored Pumpkin `eda69440`, Terralith 2.6.5, Incendium 5.5.4. 이 문서는 설계 초안이며 코드·설정 변경이나 데이터팩 적용을 수행하지 않았다. Phase 2의 [MISSING.md](phase2/MISSING.md)는 소스 경로를 모사한 정적 감사이지 서버 실행 결과가 아니다. 실제 적용과 검증은 Phase 5 범위다. 아래 경로는 저장소 루트 기준이다.

## 먼저 해결할 좌표 충돌

`min_y=-512, height=1024`는 블록 Y **-512..511**(상한 배타 512)이다. 따라서 **월드 좌표 Y≈800의 산은 이 범위에 들어갈 수 없다**. 현재 `height=384`도 최고 Y가 아니라 높이이며 `min_y=-64`이면 최고 블록 Y=319다. 지면 -300과 최고 산 800의 차이는 1100블록으로 전체 폭 1024보다 크다. 다음 두 선택지는 서로 배타적이며 Phase 5 구현 전에 결정해야 한다.

1. 범위 -512..511을 유지: 지면 -300을 목표로 하고 산 정상을 511 이하로 제한·재설계한다.
2. 지면 -300과 산 800을 유지: 상한을 800보다 높게 확장한다. 예를 들어 `min_y=-512, height=1328`이면 최고 Y=815이며 16블록 정렬된다. 이는 사용자 결정인 1024 폭을 변경하는 **대안**이지 승인된 값이 아니다. 클라이언트·프로토콜 허용 여부도 미검증이다.

## 현재 데이터 흐름과 고정 지점

아래 `reference/pumpkin-src/`를 P라 부른다. 파일과 줄은 고정 커밋의 소스 기준이다.

1. `P/crates/pumpkin-data/build.rs:8`은 바닐라 데이터가 없으면 서버 jar에서 `assets/datapack/data`를 확보한다. **이 build.rs 자체가 JSON을 Rust로 변환하지 않는다.** 별도 `P/tools/pumpkin-codegen/src/main.rs:111`의 codegen 실행이 `crates/pumpkin-data/src/generated`에 Rust 소스를 쓴다. 등록 목록은 같은 파일 140–207행이고 필터 실행은 240행이다. 이후 컴파일된 생성 코드가 런타임에 쓰인다. 단순히 서버의 데이터팩을 바꿔도 codegen이 재실행되는 경로는 아니다.
2. 지형: `P/tools/pumpkin-codegen/src/noise_settings.rs:250`은 바닐라 noise settings를 읽고, `noise_router.rs:2983`은 고정된 이름들의 noise settings 및 density function을 읽어 router 상수를 만든다. `noise_router.rs:2229`의 참조 해석은 `minecraft:`만 제거하고 바닐라 디렉터리에서 찾는다. `noise_parameter.rs:63,128`은 바닐라 noise 목록과 닫힌 ID 조회를 생성한다. `noise_router.rs:3423`의 이름 매핑은 제한적이다. 생성된 `P/crates/pumpkin-data/src/generated/noise_settings.rs:18`과 `noise_router.rs`가 런타임 입력이다. surface material rule도 settings별 정적 선택이다 (`P/crates/pumpkin-world/src/generation/generator/mod.rs:198`).
3. 바이옴: `P/tools/pumpkin-codegen/src/biome.rs:310`은 바닐라 biome JSON을 읽고 334행에서 별도 `assets/multi_noise_biome_tree.json`을 읽는다. 팩의 multi-noise parameter list가 자동으로 이 트리가 되지 않는다. `P/crates/pumpkin-world/src/generation/generator/mod.rs:204`에서 supplier를 선택하고 `proto_chunk.rs:795`에서 바이옴 단계를 진행한다. `proto_chunk.rs:780`은 생성된 Biome을 조회하고 ID를 `u8`로 취급한다. 팩 ID와 클라이언트 레지스트리 ID의 정합성이 필요하다.
4. feature/carver: `P/tools/pumpkin-codegen/src/configured_feature.rs:15`, `placed_feature.rs:9`, `carver.rs:7`은 바닐라 디렉터리에서 생성한다. 알 수 없는 placement modifier는 `placed_feature.rs:349`에서 compile error를 낸다. 바이옴의 carver가 `generator/mod.rs:225`에서 사용되고 `proto_chunk.rs:1003`에서 단계가 진행된다. `proto_chunk.rs:1356,1417`은 feature 순서와 생성된 `PLACED_FEATURES`를 사용한다. `generation/feature_order.rs:106,121`은 차원별 정적 순서와 u8 바이옴 ID에 의존한다.
5. 구조물: `P/tools/pumpkin-codegen/src/structures.rs:608,712,730`은 고정 structure key 및 바닐라 structure/structure_set을 생성한다. `generator/mod.rs:217`은 정적 `StructureSet::ALL`을 사용하고 `proto_chunk.rs:1439`에서 구조물 단계를 진행한다. NBT 템플릿/풀 로드가 가능하다는 사실은 구조물 선택·배치가 팩 정의를 사용한다는 뜻이 아니다. Phase 2는 template pool 및 processor list도 부분 지원으로 분류했다.
6. 차원과 프리셋: `P/tools/pumpkin-codegen/src/dimension.rs:58`은 바닐라 dimension_type을 생성한다. `P/crates/pumpkin-data/src/generated/dimension.rs:84`의 오버월드는 -64/384/384, 211행의 네더는 0/256/128 (`min_y/height/logical_height`)이다. `P/crates/pumpkin-world/src/world_info/mod.rs:252` 부근의 world preset은 내장 JSON을 사용한다. Incendium의 `reference/datapacks/Incendium/data/minecraft/dimension_type/the_nether.json:31`은 0/256/192다. logical height와 실제 저장·생성 높이는 별개다.
7. 런타임 팩 경로는 `P/crates/pumpkin-core/src/data/datapack/mod.rs:250`에서 동적 레지스트리를 합치고 `dynamic_registry_manager.rs:140`, `net/java/login/known_packs.rs:5`를 통해 클라이언트 동기화에 사용한다. 반면 위 generator는 생성된 상수와 제한된 이름 조회를 사용한다. 레지스트리 전송 자체는 서버 지형 생성에 연결되지 않는다. Phase 2의 각 registry별 `CLIENT_SYNC_ONLY`, `IGNORED_BY_WORLDGEN`, `PARTIAL` 구분을 유지한다. 구조물 NBT가 로드된다는 항목도 자동 생성 보장은 아니다.

## 적용 설계 — 제안, 실행 미검증

**권장: 결정적인 빌드 프로파일 생성과 월드별 고정 선택.** 바닐라 → Terralith → Incendium → 프로젝트 좌표 변환 순으로 전체 네임스페이스를 가진 리소스 맵을 합친다. 같은 리소스 ID는 뒤쪽 팩이 우선하도록 하고 충돌 목록과 파일 해시, 팩 순서를 manifest에 남긴다. 실제 순서·충돌은 빌드에서 검사해야 한다. 입력을 원본 디렉터리에 덮어쓰지 말고 별도 staging으로 만들고, 참조를 완전히 해석한 뒤 profile별 생성 Rust를 만든다. 기본 바닐라 결과와 별도 산출물로 만들어 플래그가 꺼져 있으면 기존 경로를 사용한다. 현재 codegen의 바닐라 전용 경로를 staging 경로로 바꾸는 것만으로는 충분하지 않다.

- 리소스 로더: namespace/path와 inline/reference를 모두 처리하고 순환·누락·중복·지원하지 않는 타입은 조용히 무시하거나 fallback하지 말고 ID와 경로를 출력하며 실패한다. dimension → generator → noise settings/biome source, 참조되는 density/noise/material rule, biome → carver/placed feature → configured feature, structure set → structure → pool/processor/template 및 tag까지 닫힌 의존 그래프를 검증한다. 미지원 동작은 Phase 2 감사 목록에서 출발해 실제 parser와 executor 둘 다 확인한다.
- codegen: 위 noise router의 고정 이름·namespace 가정, noise ID 목록, feature/carver/structure의 닫힌 enum과 참조 변환을 확장한다. biome parameter list를 현재 별도 tree 형식으로 정확하게 변환하거나 동등한 검색 구조를 구현한다. 바이옴별 feature 순서, 구조물의 biome tag와 배치, template pool/processor, surface rule까지 한 프로파일에서 생성한다. Terralith의 바이옴 보존은 ID 목록만 수록하는 것이 아니라 climate 매핑, 각 바이옴 데이터와 feature/carver 및 생성 결과를 검증해야 한다.
- 서버/클라이언트: 프로파일에서 동일한 biome 및 dimension type ID 순서와 내용을 서버 생성기와 클라이언트 동기화에 공급한다. `VanillaGenerator::new_with_all_settings` (`generator/mod.rs:191`)의 정적 lookup/fallback을 명시적 profile 선택으로 대체하고, 알 수 없는 프로파일은 오류로 처리한다. structure placement의 별도 overworld fallback (`generation/structure/placement.rs:381,530`)도 동일 프로파일로 연결한다. 월드 생성 시 profile ID, 버전, 입력 해시와 차원별 경계를 저장하고 재시작 때 불일치하면 중단한다. 새 월드를 기본 대상으로 한다.
- 다른 경로: 런타임 로더와 서버용 typed registry를 만들어 시작 시 해석하면 재빌드 없이 팩을 변경할 수 있지만, 현재 `'static` 참조·닫힌 enum·캐시와 전송 ID를 전면 개편해야 하고 재현성 관리가 더 어렵다. 기존 런타임 동기화 registry만 재사용하는 방식은 generator 연결이 없어 불충분하다. 빌드 프로파일도 미지원 타입과 생성 알고리즘을 새로 구현해야 하며 호환성을 자동으로 보장하지 않는다.

## 프리셋 초안

아래는 **제안된 설정 스키마**이지 현재 Pumpkin에서 읽는 설정이 아니다. `enabled=false`가 기본이며 false에서는 아래 값을 적용하지 않는다. 높이 충돌이 남아 있으므로 true여도 검증 실패로 시작을 막는다.

```toml
[experimental.worldgen]
enabled = false
profile = "terralith_incendium_extreme_v1"

[experimental.worldgen.overworld]
min_y = -512
height = 1024                 # 최대 블록 Y = 511
logical_height = 1024
ground_y_target = -300        # 지형 보정의 목표치, 보장값 아님
mountain_peak_y_target = 800  # 범위 밖: 유효성 검사 실패
preserve_terralith_biomes = true

[experimental.worldgen.nether]
min_y = 0
height = 256                  # Incendium dimension_type 기준
logical_height = 192
```

높이 필드만 바꾸면 지면 -300이나 산 800이 만들어지지 않는다. 해수면·density 함수의 수직 좌표 및 gradient, surface rule, aquifer, 동굴·광석·feature와 구조물의 절대/상대 높이, spawn을 함께 변환해야 한다. 일률적인 이동이나 배율은 Terralith 특유 지형과 바이옴의 관계를 바꿀 수 있으므로 변환 규칙과 성공 기준은 Phase 5에서 정하고 샘플 시드로 측정한다. 산 높이는 절대 보장보다 분포 목표로 정의할 필요가 있다. 네더도 logical height 변경만으로 Incendium 생성이 적용되지 않는다.

## 영향과 위험

- `P/crates/pumpkin-data/src/generated/noise_settings.rs:19`의 `min_y: i8`, `P/crates/pumpkin-world/src/generation/proto_chunk.rs:216`의 `as i8`, `generation/height_provider.rs:16`의 i8은 -512를 표현하지 못한다. codegen과 소비자 전체에서 좌표 타입·캐스팅·경계 검사를 확장해야 한다.
- `P/crates/pumpkin-world/src/chunk/mod.rs:199`의 heightmap은 9비트와 37 long으로 고정되어 `height - min_y + 1`을 마스킹한다. 높이 1024에서 0..1024를 담으려면 11비트가 필요하다. 저장·로드·네트워크·클라이언트 해석을 함께 검증해야 한다. 청크 section 수, 바이옴 팔레트, 조명, 구조물 탐색 및 메모리/CPU 비용도 감사 대상이다.
- 기존 월드에서는 청크 경계의 지형 단절, 높이맵/section 형식 및 생성기 변경으로 인한 의미 불일치, ID 재배치 및 되돌리기 손상이 발생할 수 있다. 기존 월드에 자동 적용하지 않고 백업·별도 새 월드로 시작하며, 마이그레이션과 롤백은 별도 설계·검증 전까지 지원하지 않는다.
- Java와 Bedrock의 높이 제한, 레지스트리 동기화, 위치 직렬화 및 클라이언트 수용 여부는 여기서 판정하지 않았다. 프로파일이 내부에서 표현 가능해도 접속·렌더링이 가능하다는 뜻은 아니다.
- Phase 2 감사의 미지원 modifier, custom ID, 중첩 태그, loot/function 및 processor 제한은 결과물에 영향을 준다. 생성만 성공해도 팩 전체 게임플레이가 완전하다는 근거는 아니다.

## Phase 5 검증 계획 — 모두 미실행

프로파일 생성 시 전체 참조와 지원 타입을 검사하고 결정적 산출물/ID 매핑을 비교한다. 높이 경계와 heightmap round-trip, 저장·재시작, 바이옴 및 feature/structure 선택, 차원별 동기화에 대한 테스트를 추가한다. 고정 시드에서 지면·산 높이 분포와 Terralith 바이옴 출현, Incendium 구조물 및 네더 logical height를 실제 생성으로 확인한다. Java/Bedrock 접속 및 극단 높이의 청크·조명·이동을 각각 확인하고, 기존 월드 호환성과 성능을 별도 평가한다. 충돌하는 최고 Y 결정과 클라이언트 한계가 해결되기 전에는 목표 달성을 주장하지 않는다.
