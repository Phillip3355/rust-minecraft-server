# Phase 3 — 플러그인 호환성 정적 분석

기준: 벤더링된 Pumpkin `eda69440e6369c5f8a05c3f69fc809c40dab93a6`, 조사일 2026-11-18. 이 문서는 준비 보고서다. **이번 작업에서 Java, Paper, PatchBukkit 또는 플러그인을 실행하지 않았다. 모든 실행 결과는 NOT TESTED(미검증)**다. WIT 선언은 API 계약의 증거이지 구현 완성도나 동작의 증거가 아니다.

## 결론과 경계

현재 트리의 Pumpkin은 자체 Rust 네이티브 및 WASM 플러그인 시스템을 제공한다. Bukkit/Paper API 구현이나 JAR 로더는 발견하지 못했다. [Pumpkin 자체 설명](../reference/pumpkin-src/AGENTS.md)도 Bukkit 플러그인을 직접 로드할 수 없다고 명시한다. [매니저](../reference/pumpkin-src/crates/pumpkin-core/src/plugin/mod.rs)는 네이티브 로더와 추가 로더를 사용하고, [네이티브 로더](../reference/pumpkin-src/crates/pumpkin-core/src/plugin/loader/native.rs)는 운영체제별 `.so`/`.dylib`/`.dll` 및 Pumpkin API 버전 심볼을 요구한다. WASM 인터페이스는 [WIT v0.1/v0.2](../reference/pumpkin-src/crates/pumpkin-plugin-wit/)와 `pumpkin-wasm-host*`에 있다. 이 시스템의 API가 Bukkit과 이름이나 역할이 비슷해도 JAR 바이너리 호환을 뜻하지 않는다.

[PatchBukkit](https://github.com/Pumpkin-MC/PatchBukkit)은 별도의 Pumpkin 네이티브 플러그인이다. 현재 저장소에 그 소스나 바이너리는 포함되지 않았고, Pumpkin 고정 커밋과의 ABI 호환성도 확인되지 않았다. 따라서 계획서의 PatchBukkit 및 헤드리스 Paper 구성은 **목표 아키텍처**이지 현재 체크아웃의 기능이 아니다. 외부 프로젝트의 지원 주장도 이 조합의 검증 결과가 아니다.

## JVM 및 실행 경로

고정 Pumpkin 소스의 Rust/TOML/WIT/Markdown에서 `BJVM`, `PatchBukkit`, `org.bukkit`, `libjvm`을 검색했으며 해당 통합을 발견하지 못했다. 따라서 여기서 BJVM의 Java 버전이나 기동 명령을 소스로 확정할 수 없다. **BJVM이 이 트리에 내장되어 있다는 근거도 없다.**

별도 PatchBukkit의 [README](https://github.com/Pumpkin-MC/PatchBukkit)는 Java 25 이상과 네이티브 라이브러리를 Pumpkin `plugins/`에 배치할 것을 요구한다. README의 설치 절과 개발 절은 JAR 디렉터리를 각각 `patchbukkit/patchbukkit-plugins/`와 `plugins/data/patchbukkit/patchbukkit-plugins/`로 다르게 기술하므로 고정할 릴리스의 실제 코드와 생성 로그로 확인해야 한다. 공식 [아키텍처 문서](https://github.com/Pumpkin-MC/PatchBukkit/blob/master/ARCHITECTURE.md)는 JVM 스레드, JNI 및 Protobuf 브리지를 설명하고, [2026-10 발표](https://blog.pumpkinmc.org/2026-10-23-patchbukkit-nms-and-headless-paper/)는 헤드리스 Paper 접근을 설명한다. 두 문서의 API 구현 설명은 서로 다를 수 있으며 둘 다 이동하는 외부 자료다. 특정 커밋의 실행 코드, JVM 옵션, Paper 버전, 리스너/틱 비활성화 및 CLI 기동 절차는 아직 확인되지 않았다. 독립 `java -jar ... --nogui` 명령을 이 통합의 절차로 가정하지 않는다. Java 25 요구는 PatchBukkit README 기준이며 Pumpkin 자체 요구가 아니다.

## 기능별 소스 판정

정적 판정의 PARTIAL은 Pumpkin 고유 API에 관련 선언이 있다는 뜻이며 Bukkit 동등성은 주장하지 않는다. FAIL은 현재 트리에 해당 브리지가 없다는 범위의 판정이다. PASS는 동등성과 실행이 확인된 경우에만 사용한다. 실행 열은 모두 NOT TESTED다.

| 영역 | 정적 판정 | 근거 및 제한 | 실행 |
|---|---|---|---|
| 이벤트 | PARTIAL | [context.wit](../reference/pumpkin-src/crates/pumpkin-plugin-wit/v0.2/context.wit)에 우선순위·blocking 등록, [event.wit](../reference/pumpkin-src/crates/pumpkin-plugin-wit/v0.2/event.wit)에 이벤트 타입 선언. Bukkit 이벤트 매핑/취소 의미는 미확인 | NOT TESTED |
| 명령 | PARTIAL | context의 명령 등록 및 [command.wit](../reference/pumpkin-src/crates/pumpkin-plugin-wit/v0.2/command.wit)의 노드·발신자 선언. Bukkit 명령 등록과 동일성 미확인 | NOT TESTED |
| 권한 | PARTIAL | context 권한 등록 및 command의 `has-permission` 선언. [plugins 설정](../reference/pumpkin-src/crates/pumpkin-config/src/plugins.rs)의 플러그인 권한 승인과 플레이어 권한은 구분해야 함. Bukkit 권한 프로바이더 연동 미확인 | NOT TESTED |
| GUI | PARTIAL | [gui.wit](../reference/pumpkin-src/crates/pumpkin-plugin-wit/v0.2/gui.wit)와 [Rust GUI API](../reference/pumpkin-src/crates/pumpkin-core/src/plugin/api/gui.rs). Bukkit 인벤토리 이벤트 및 클릭 동기화 미확인 | NOT TESTED |
| 월드 조작 | PARTIAL | [world.wit](../reference/pumpkin-src/crates/pumpkin-plugin-wit/v0.2/world.wit)에 블록 읽기/쓰기와 엔티티 생성 선언. 대량 편집, 저장 및 Bukkit 의미 동일성 미확인 | NOT TESTED |
| 엔티티 동기화 | FAIL | Pumpkin 쪽 엔티티/텔레포트 선언은 있으나 Java 객체와 Rust 엔티티 간 브리지가 이 트리에 없다. 클라이언트 패킷 동기화와도 별개 | NOT TESTED |
| Bukkit/Paper JAR 로딩 | FAIL | 현재 로더 경로에 Bukkit/JVM 로더가 없다. 외부 PatchBukkit 추가 시 결과는 별도 평가 | NOT TESTED |

## 주요 플러그인 대조 초안

버전을 고정하지 않았으므로 아래의 API 버전은 확인된 범위만 표시한다. `api-version`은 플러그인 메타데이터의 선언이며 지원 Minecraft 범위 전체나 런타임 호환성을 뜻하지 않는다. 기본 Pumpkin이 제공하는 Bukkit/Paper API 버전은 **없음**이고, PatchBukkit의 API 버전과 해당 Pumpkin 핀과의 호환성은 미확정이다. 각 행의 실제 로딩 및 기능은 **NOT TESTED**다.

| 플러그인 | 공식 자료의 요구/버전 | 현재 Pumpkin 대조 및 정적 판정 | 실행 |
|---|---|---|---|
| LuckPerms | [공식 저장소](https://github.com/LuckPerms/LuckPerms)에 Bukkit 플랫폼 모듈. 특정 빌드의 최소 Bukkit API/26.3 지원은 미확인 | Bukkit JAR 로더 및 권한 브리지 부재: FAIL | NOT TESTED |
| EssentialsX | [plugin.yml](https://github.com/EssentialsX/Essentials/blob/2.x/Essentials/src/main/resources/plugin.yml) `api-version: 1.13` (이동 브랜치); 실제 릴리스의 서버 지원 범위는 별도 확인 | Bukkit API/로더 부재: FAIL | NOT TESTED |
| WorldEdit | [공식 설치 문서](https://worldedit.enginehub.org/en/latest/install/)는 Paper/Spigot용 빌드와 Minecraft 버전에 맞는 다운로드 요구. 정확한 API 버전 미확인 | Bukkit API/로더 및 월드 편집 브리지 부재: FAIL | NOT TESTED |
| WorldGuard | [공식 설치 문서](https://worldguard.enginehub.org/en/latest/install/)는 Bukkit API와 Bukkit판 WorldEdit 요구. 정확한 API 버전 미확인 | Bukkit API/로더 및 보호 이벤트 브리지 부재: FAIL | NOT TESTED |
| PlaceholderAPI | [공식 소스](https://github.com/PlaceholderAPI/PlaceholderAPI/blob/master/src/main/java/me/clip/placeholderapi/PlaceholderAPIPlugin.java)는 `JavaPlugin` 사용. [문서](https://wiki.placeholderapi.com/developers/using-placeholderapi/)의 일부 Component 기능은 Paper 기반 서버 요구. 정확한 기본 API 버전 미확인 | Bukkit API/로더 및 확장 연동 부재: FAIL | NOT TESTED |

FAIL은 현재 **PatchBukkit 미통합 구성**에 대한 정적 장애이며, 외부 브리지 통합 후 각 플러그인의 실패를 실측했다는 의미가 아니다.

## 후속 실행 준비 및 판정

1. 사용자 승인 후에만 Java 설치 및 실제 실행을 진행한다. 격리된 테스트 디렉터리와 복사본 월드 사용. PatchBukkit 커밋, Pumpkin 커밋, Java 배포판/버전, Paper 및 플러그인 빌드와 해시를 고정한다. 우선 PatchBukkit의 `PUMPKIN_API_VERSION` 및 빌드 의존성이 이 핀과 맞는지 소스에서 확인한다.
2. 고정한 PatchBukkit의 JVM 로딩 방식, Java 요구, Paper 아티팩트·기동 옵션, JAR 디렉터리, 네트워크/틱 동작 및 라이선스를 소스로 확인하고, 로그로 JVM 초기화와 Paper/브리지 상태를 검증한다. README 경로 충돌을 해결하기 전 경로를 확정하지 않는다.
3. 동일 버전 Paper 기준선에서 각 플러그인 단독 로딩을 먼저 확인하고, Pumpkin+PatchBukkit에서 로딩·종료 로그를 수집한다. WorldGuard는 호환 WorldEdit을 함께 설치한다. LuckPerms 권한 허용/거부, EssentialsX 명령·GUI, WorldEdit 블록 수정/저장/재접속, WorldGuard 보호/취소, PlaceholderAPI 확장 및 치환을 각각 확인한다.
4. 이벤트 순서·취소, 비동기 호출, GUI 클릭, 엔티티 생성/이동/제거 및 재접속 후 상태를 Paper와 비교한다. 테스트별 버전·입력·기대/실제 결과·로그를 기록하고 그때만 PASS/PARTIAL/FAIL을 실행 결과로 부여한다. 메모리는 Rust와 JVM의 전체 프로세스 범위를 명시해 측정한다.

이번 검증은 위 소스 검색과 공식 문서 열람에 한정된다. 런타임 호환성, 성능 및 안정성은 미검증이다.
