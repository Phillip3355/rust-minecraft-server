# Golden Snapshot 규약 및 생성 절차

`tests/performance/snapshots/`에는 대용량 월드 바이너리(`.mca` 등)를 Git에 커밋하지 않고,
**체크섬 매니페스트(`*.sha256`, `*.json`)와 생성 절차**만 추적한다.
실제 스냅샷 월드 데이터는 `.work/benchmark/snapshots/vanilla_overworld_r2048/`에 생성·보관한다.

## 1. Golden Snapshot 생성 절차 (Paper 26.3 + Chunky)

1. Paper 26.3 및 Chunky 플러그인을 준비한다:
   ```bash
   ./scripts/benchmark/fetch_artifacts.sh --paper --chunky
   ```
2. 시드 `1790155153199293024` (`scripts/benchmark/bench_manifest.toml`의 `vanilla_overworld`)로
   임시 Paper 서버를 기동한 뒤 콘솔에서 아래 명령을 실행한다:
   ```text
   chunky world world
   chunky center 0 0
   chunky radius 2048
   chunky start
   ```
3. Chunky 완료 로그가 출력되면:
   ```text
   save-all
   stop
   ```
4. 생성된 `world/` 디렉터리에서 `session.lock` 및 임시 캐시를 제거한 뒤
   `.work/benchmark/snapshots/vanilla_overworld_r2048/`로 이동하고 읽기 전용 권한(`chmod -R a-w`)을 부여한다.
5. 체크섬 매니페스트를 기록한다:
   ```bash
   (cd .work/benchmark/snapshots/vanilla_overworld_r2048 && find . -type f | sort | xargs sha256sum) \
     > tests/performance/snapshots/vanilla_overworld_r2048.sha256
   ```

## 2. Pumpkin 호환성 사전 검증

`S1`, `S2` 벤치마크 시작 전 Golden Snapshot 사본을 Pumpkin `0.2.2+26.3-26.60` (`[world.chunk] type = "anvil"`)으로
1회 로드·저장하여 청크 파싱 오류나 빈 청크 현상이 없는지 확인해야 한다.
