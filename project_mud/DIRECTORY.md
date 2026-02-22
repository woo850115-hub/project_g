# project_mud/ 디렉토리 구조

```
project_mud/
├── Cargo.toml                          # 바이너리 패키지 (mud_server)
├── server.toml                         # MUD 서버 설정 파일 (TOML)
├── src/
│   ├── main.rs                         # MUD 서버 진입점 (tokio + tick 스레드, 로그인 상태머신, 자동저장)
│   ├── config.rs                       # 서버 설정 — TOML 파싱, CLI 오버라이드, 기본값
│   └── shutdown.rs                     # 안전 종료 — watch 채널 기반 ShutdownTx/ShutdownRx
├── crates/
│   ├── mud/                            # MUD 게임 로직 crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # 공개 모듈 재export (components, parser, systems 등)
│   │       ├── components.rs           # ECS 컴포넌트 (Health, Attack, Defense, Name, InRoom, Inventory 등)
│   │       ├── parser.rs               # 플레이어 입력 파서 (PlayerAction 열거형, 방향 매핑)
│   │       ├── output.rs               # 출력 헬퍼 재export
│   │       ├── session.rs              # 세션 헬퍼 재export
│   │       ├── persistence_setup.rs    # MUD 컴포넌트 영속성 등록 (PersistenceRegistry)
│   │       ├── script_setup.rs         # MUD 컴포넌트 스크립트 등록 (ScriptComponentRegistry)
│   │       └── systems/
│   │           └── mod.rs              # GameContext, PlayerInput 정의
│   └── player_db/                      # SQLite 플레이어 데이터베이스 crate
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                  # 공개 API (PlayerDb, AccountRepo, CharacterRepo)
│           ├── db.rs                   # 데이터베이스 연결 관리
│           ├── schema.rs              # SQL 스키마 (accounts, characters 테이블)
│           ├── account.rs              # 계정 CRUD (생성, 인증, 권한 설정)
│           ├── character.rs            # 캐릭터 CRUD (생성, 상태 저장, 로드, 삭제)
│           └── error.rs               # 에러 타입 정의
├── scripts/                            # Lua 게임 스크립트
│   ├── 00_utils.lua                    # 공용 헬퍼 (format_room, broadcast_room, HELP_TEXT, colors)
│   ├── 01_world_setup.lua              # on_init 월드 생성 (방, NPC, 아이템)
│   ├── 02_commands.lua                 # on_action 명령어 처리 (look/move/attack/get/drop/say/who/help)
│   ├── 03_combat.lua                   # on_tick 전투 해결 시스템
│   └── 04_admin.lua                    # on_admin GM 도구 (kick/announce/teleport/stats/help)
├── data/
│   └── snapshots/                      # 영속성 스냅샷 바이너리 (자동 생성)
│       ├── latest.bin
│       └── snapshot_tick_*.bin
├── test_fixtures/                      # 테스트용 사전 빌드 WASM 플러그인 바이너리
│   ├── test_movement.wasm
│   ├── test_infinite_loop.wasm
│   └── test_panic.wasm
└── tests/                              # 통합 테스트 (10개)
    ├── tick_simulation.rs              # 100 엔티티 × 5 방 × 300틱 시뮬레이션
    ├── tick_determinism.rs             # 동일 시드 → 동일 결과 결정론 검증
    ├── wasm_plugin_test.rs             # WASM 플러그인 로드/실행/격리
    ├── fuel_determinism.rs             # Fuel 기반 결정론적 실행 검증
    ├── memory_grow_stress.rs           # WASM 메모리 확장 안전성
    ├── space_test.rs                   # RoomGraphSpace 생명주기
    ├── content_registry_test.rs        # JSON 콘텐츠 → Lua 로딩 검증
    ├── game_systems_integration.rs     # 전체 게임 흐름 (이동/전투/인벤토리/대화)
    ├── snapshot_integration.rs         # 스냅샷 캡처/복원/디스크 영속성
    └── server_integration.rs           # TCP 로그인 + 이동 종단간 테스트
```
