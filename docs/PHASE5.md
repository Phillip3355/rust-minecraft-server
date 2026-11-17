# Phase 5 — Extreme Worldgen

**상태: 부분 구현.** Pumpkin의 64섹션 청크 처리와 빌드 프로파일 입력 스테이징을 추가했다. Extreme 생성기와 클라이언트 레지스트리 연동은 미완료다. 실서버 검증은 **NOT TESTED**다.

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

## 빌드 입력 스테이징

로컬에 추출된 바닐라 26.3 데이터와 `reference/datapacks/`의 Terralith 및 Incendium을 사용한다. 저장소 루트에서 다음 두 명령을 실행하면 지정 디렉터리에 프로파일 입력이 생성된다.

```sh
mkdir -p .work/phase5
CARGO_NET_OFFLINE=true CARGO_PROFILE_DEV_LTO=false CARGO_PROFILE_DEV_CODEGEN_UNITS=16 \
  cargo run --manifest-path reference/pumpkin-src/Cargo.toml --offline -j 2 \
  -p pumpkin-codegen -- stage-worldgen-profile .work/phase5/profile
```

스테이징은 바닐라 → Terralith → Incendium 순서로 전체 네임스페이스를 유지해 합친다. 충돌한 일반 리소스는 마지막 팩을 사용한다. 태그는 값을 이어 붙이고 `replace: true`에서 교체한다. `manifest.json`에는 입력·출력의 파일별 SHA-256, 충돌 출처, 목표 좌표가 기록된다. 양쪽 차원의 dimension type과 noise settings 참조, 인라인 기후 목록의 바이옴 참조, Incendium 네더의 차원 경계를 검사한다. 산출물의 상태는 `codegen_input`이며 바이옴 검색 트리나 생성기 실행 코드를 포함하지 않는다.

```toml
[experimental.worldgen]
enabled = false
profile = "terralith_incendium_extreme_v1"
```

기존 설정에서 이 항목을 생략해도 비활성화 상태다. 현재 `enabled = true`는 컴파일된 생성기가 없다는 오류를 반환한다. 이 검사는 `level.dat` 조회, 백업 및 쓰기 전에 실행된다.

## 남은 범위

- 팩의 namespaced density/noise/surface rule을 생성 코드에 연결하고, 기후 목록에서 Terralith 및 Incendium 바이옴을 선택하는 검색 트리를 생성해야 한다. 서버 선택 결과와 클라이언트 동기화에 동일한 바이옴 ID가 필요하다.
- 지면과 정상 목표를 실제 밀도 라우터에 반영하고 대수층, 지표 규칙, 광물, 피처, 구조물 높이를 함께 변환해야 한다. 팩의 참조를 끝까지 해석하고 카버, 피처, processor, template pool, 구조물 실행 경로를 완성해야 한다. 현재 스테이징 검사는 진입점과 기후 바이옴 참조를 다룬다.
- 완성된 프로파일을 차원·레지스트리와 함께 등록하고 새 월드의 프로파일 ID, 버전, 입력 해시, 차원 경계를 영속화해야 한다. 기존 월드 적용 및 불일치 상태에서의 재시작을 거부하는 검증도 이 연결에 포함된다.

## 검증 범위

| 오프라인 검증 | 결과 |
|---|---|
| `tools/world-verify` | 유닛 24개 및 회귀 26개 통과 |
| `tools/datapack-audit` | 20개 통과 |
| `pumpkin-codegen`, `pumpkin-config` | 각각 8개, 24개 통과 |
| `pumpkin-world --lib` | 302개 통과. 임시 복사본에서 벤치마크 전용 Criterion 의존성만 제외했다. 제출 manifest는 수정하지 않았다. |
| `pumpkin-core --lib net::java::chunk_data` | 3개 통과. 26.3 조명 왕복에서 비트 64와 65, skylight와 blocklight 배열을 확인했다. |
| 빌드 입력 스테이징 | 두 번 실행해 manifest 및 전체 15,839개 출력 SHA-256 일치 확인. 충돌 경로 303개, 기본 기후 목록의 Terralith 바이옴 참조 90종 및 Incendium 바이옴 참조 8종을 확인했다. |
| Pumpkin 전체 `cargo test --offline -j 2` | 캐시에 `alloca 0.4.0`이 없어 테스트 실행 전 종료 코드 101 |

Cargo 실행은 LTO 비활성화 및 codegen-units 16을 환경 변수로 지정했다. 작업 워크트리에 없던 바닐라 추출 데이터는 같은 저장소의 기존 체크아웃에서 가져왔으며, 테스트와 빌드 산출물은 Git에서 제외했다.

연결 워크트리의 Git 메타데이터가 읽기 전용이므로 원래 HEAD는 유지했다. 같은 기준 커밋을 부모로 하는 커밋 이력과 diff는 워크트리 내 별도 Git 저장소와 패치로 제출한다.

실서버 기동·종료 후 재시작, Java/Bedrock 플레이, 접속 및 렌더링, 인게임 조명과 블록 상호작용, 실제 지형 높이 분포와 팩 바이옴·구조물의 출현은 모두 **NOT TESTED**다. 서버는 기동하지 않았다.
