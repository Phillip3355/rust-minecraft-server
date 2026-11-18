# Phase 5 — Extreme Worldgen

**상태: 실험용 오프라인 통합.** Terralith·Incendium 스테이징 데이터를 밀도 라우터, 바이옴, 장식 피처, 카버 및 지그소 경로에 연결했다. Java와 Bedrock의 프로파일 전용 구조물 패킷 생성 경로를 추가했다. 실서버 검증은 **NOT TESTED**이며 시작 가드는 유지한다.

## 확정 좌표

| 항목 | Extreme 오버월드 | Incendium 네더 |
|---|---:|---:|
| 최소 블록 Y | -512 | 0 |
| 높이 | 1,024 | 256 |
| 최고 블록 Y | 511 | 255 |
| 논리 높이 | 1,024 | 192 |
| 지면 목표 Y | -400 | — |
| 산 높이 / 정상 목표 Y | 지면 위 800 / 400 | — |

## 청크와 저장

- 노이즈 형상, 프로토 청크, 지표 규칙, 대수층, 카버, 피처와 구조물의 최소 Y 전달 경로를 `i16`으로 확장했다. 노이즈 설정 및 구조물 Rust 상수는 수정된 코드 생성기로 재생성했다.
- 높이맵의 값은 `Y - min_y + 1`이다. 빈 열은 0이며, 확장 청크의 Y=511은 1,024로 인코딩한다. 64섹션의 높이맵은 11비트씩 long 하나에 5값을 담아 총 52 long을 사용한다. 기존 9비트 높이맵은 37 long을 유지한다. 블록 변경 후 높이맵에 전달하는 Y와 Z 인수의 순서도 바로잡았다.
- Anvil은 기존 NBT 태그를 사용한다. `.pump` 지역 파일 헤더는 그대로 사용하고, 넓은 높이맵을 쓰는 청크의 `PCHK` 페이로드에는 버전 2와 높이맵 비트 폭을 기록한다. 기존 버전 1 청크를 읽을 수 있으며, 9비트만 필요한 청크는 버전 1로 쓴다.
- 랜덤 틱 마스크는 섹션 수에 맞춰 64비트 워드를 할당한다. Java의 전체 청크와 조명 갱신 패킷은 같은 조명 인코더를 사용한다. 확장 청크의 최상단 섹션 비트는 64, 상한 바깥 조명 비트는 65다. 균일한 비영 조명은 저장과 전송 시 니블 배열로 변환한다.

## 프로파일 빌드

저장소 루트에서 실행한다. 바닐라 26.3 데이터는 미리 추출된 로컬 `reference/pumpkin-src/assets/datapack`을 사용한다.

```sh
mkdir -p .work/phase5
CARGO_NET_OFFLINE=true cargo run --manifest-path reference/pumpkin-src/Cargo.toml \
  --offline -j 2 -p pumpkin-codegen -- stage-worldgen-profile .work/phase5/staged
CARGO_NET_OFFLINE=true cargo run --manifest-path reference/pumpkin-src/Cargo.toml \
  --offline -j 2 -p pumpkin-codegen -- compile-worldgen-profile \
  .work/phase5/staged .work/phase5/compiled
```

스테이징은 바닐라 → Terralith → Incendium 순으로 네임스페이스를 유지해 합친다. 마지막 팩이 일반 리소스의 충돌을 결정한다. 태그는 값을 이어 붙이고 `replace: true`에서 교체한다. 컴파일 단계는 manifest의 출력 SHA-256과 실제 파일 목록을 대조한 뒤 밀도·노이즈·지표 규칙, 기후 목록, 배치·구성 피처, 카버, 구조물 집합·지그소 풀·프로세서 및 NBT 템플릿을 빌드 입력으로 생성한다. `PUMPKIN_COMPILED_WORLDGEN_PROFILE`을 컴파일 출력 디렉터리의 절대 경로로 설정하면 빌드에 이 입력이 포함된다.

오버월드 라우터는 지형 기준점 `-64 → -512`, `63 → -400`, `319 → 400`으로 수직 기울기와 높이 앵커를 변환한다. 밀도 라우터의 정상 상한은 Y=400이다. 해수면, 대수층의 하층 용암 및 광맥의 Y 범위도 변환한다. 지표 고정 구조물은 새 해수면과 지형 높이에서 배치하며 지면 투영 구조물은 팩의 상대 배치 오프셋을 사용한다. Incendium 생성 높이는 192, 청크 저장 높이는 256, 논리 높이는 192다. 두 팩의 기후 목록은 동일한 샘플러를 사용하는 차원별 검색 트리에서 처리한다.

청크 카버 단계는 팩 바이옴에 연결된 카버와 팩이 덮어쓴 바닐라 카버를 사용한다. 피처 단계는 해당 차원의 기후 목록으로 피처 순서를 계산하고 청크 이웃의 바이옴에 해당하는 피처를 기존 생성기에서 실행한다. 팩 블록 태그는 빌드에서 해석한다. 오버월드 피처와 카버의 절대 높이 앵커는 프로파일 좌표로 변환하고 네더 앵커는 팩 좌표를 사용한다.

Terralith 95종과 Incendium 8종의 바이옴 ID는 바닐라 ID 뒤에 배정한다. Anvil 바이옴 팔레트에 네임스페이스를 보존한다. Java 프로파일 레지스트리에는 바이옴과 차원 높이를 넣고 해당 레지스트리 크기에 맞춰 직접 팔레트를 인코딩한다. Bedrock 프로파일 바이옴 정의에는 기존 ID와 겹치지 않는 ID를 배정한다. 프로파일이 선택된 경우 Java 설정 단계에 구조물, 구조물 집합, 템플릿 풀 및 프로세서 목록을 포함한다. Bedrock 지그소 패킷의 네 목록에는 스테이징된 정의와 네임스페이스 ID를 Bedrock 네트워크 NBT로 넣는다. 이 경로는 패킷 인코딩까지 포함한다. Java 클라이언트의 추가 레지스트리 수락, Bedrock의 네이티브 지그소 정의 해석 및 렌더링은 **NOT TESTED**다.

## 설정 및 저장

```toml
[experimental.worldgen]
enabled = false
profile = "terralith_incendium_extreme_v1"
```

설정을 생략하면 비활성화된다. 현재 활성화 요청은 빌드 입력 유무에 따른 오류를 반환하고 월드 열기 전에 종료한다. 시험용 프리셋의 생성기 참조는 `data/minecraft/world_gen_settings.dat`에 저장한다. `level.dat`의 `PumpkinWorldgenProfile`은 프로파일 ID, 버전, 스테이징 입력 해시 및 차원 경계를 기록한다. 기본 월드의 저장에는 이 태그가 없다. 기록이 손상됐거나 선택과 불일치하면 열기 또는 덮어쓰기를 거부한다.

## 남은 범위

- 지그소 프로세서의 미지원 타입, 팩 템플릿 변형, 구조물별 실제 출현과 배치 높이는 검증이 남아 있다.
- 정상 목표 Y=400은 높이 변환 기준점이다. 여러 시드에서 지면과 정상 분포를 실측하는 작업은 **NOT TESTED**다.
- 기본 Java 26.3 동기화 목록에는 이 구조물 레지스트리가 없다. Bedrock 패킷에 담긴 정의는 Java 팩 스키마를 사용하며 Bedrock 네이티브 구조물 포맷과 대조할 로컬 기준 데이터가 없다. 클라이언트 구조물 동기화는 미완료다.
- 중첩 피처 배치 중 바닐라 해양 식생의 `cuboid`는 기존 생성기와 같은 제한이 있다.

## 검증 범위

| 오프라인 검증 | 결과 |
|---|---|
| `tools/world-verify` | 유닛 24개 및 회귀 26개 통과 |
| `tools/datapack-audit` | 20개 통과 |
| `pumpkin-world` 기본 프로파일 | 유닛 304개 및 통합 테스트 2개 통과 |
| `pumpkin-codegen` | 오버월드 높이 앵커 변환 포함 9개 통과 |
| 팩 포함 프로파일 | 두 차원 청크 단계와 블록 변경, 프로파일 메타데이터 왕복·손상 거부, Java 레지스트리와 Bedrock 지그소 NBT 인코딩 검증 통과 |
| 컴파일 재현성 | 동일 스테이징 입력에서 `worldgen_profile.rs`, `decoration.rs`, `templates.bin` 바이트 일치 |
| 워크스페이스 회귀 | TCP·UDP 루프백 테스트 3개 제외, 1,553개 통과 |
| 실서버 기동 및 재시작 | **NOT TESTED** |
| Java/Bedrock 플레이와 렌더링 | **NOT TESTED** |
| 인게임 조명 | **NOT TESTED** |
| 인게임 블록 상호작용 | **NOT TESTED** |

오프라인 회귀는 아래 명령으로 실행한다. TCP·UDP 루프백을 사용하는 Pumpkin 테스트 세 개를 제외한다. 프로파일 통합 테스트는 메모리에서 청크를 생성하고 패킷을 인코딩하며, 임시 디렉터리의 프로파일 메타데이터를 왕복 검증한다.

```sh
cd reference/pumpkin-src
cargo test --offline -j 2 -- \
  --skip telemetry_client_send_heartbeat_and_shutdown \
  --skip check_for_updates_http \
  --skip ice_socket_sends_and_receives
PUMPKIN_COMPILED_WORLDGEN_PROFILE=/absolute/path/to/compiled \
  cargo test --offline -j 2 -p pumpkin-world -p pumpkin-core -- profile
```
