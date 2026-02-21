# Project G — Rust MUD/2D MMORPG 엔진

## 프로젝트 개요

Rust 기반 MUD/2D MMORPG 겸용 게임 엔진. 단일 서버 코어로 Text MUD와 2D MMO를 동시 지원.
게임 로직은 WASM 플러그인으로 분리, 결정론적 시뮬레이션 루프가 핵심.

## 현재 진행 상태

- **Phase 0 (엔진 코어 골격): 완료**
- **Phase 1 (WASM Runtime 통합): 완료**
- **Phase 2 (Playable MUD + Persistence): 완료**
- **Phase 2.5 (엔진-게임 계층 분리): 완료**
  - session crate 분리 (SessionId, SessionOutput, SessionManager)
  - TickLoop\<S: SpaceModel\> 제네릭화
  - PersistenceRegistry 도입 (trait-object 기반 컴포넌트 등록)
  - persistence, net, engine_core → mud 역방향 의존 제거 완료
- **Phase 3 (Lua 스크립팅 엔진): 완료**
  - scripting crate 신규 (mlua/Luau 기반)
  - ScriptEngine: Lua VM 관리, 샌드박스 (메모리/명령어 제한)
  - ScriptComponentRegistry: Lua table ↔ Rust Component 변환
  - Lua API: ecs.*/space.*/output.*/log.*/hooks.* 전체 구현
  - Hook 시스템: on_tick, on_action, on_enter_room, on_connect
  - MUD 통합: 12개 컴포넌트 스크립트 등록, on_action 훅으로 커스텀 명령 지원
  - 게임 템플릿: game.toml + scripts/ 디렉토리 자동 로드
- **Phase 3.5 (Lua 게임 로직 마이그레이션): 완료**
  - Lua API 확장: space:register_room/room_exists/room_count/all_rooms, sessions:session_for/playing_list, hooks.on_init
  - TagComponentHandler: PlayerTag/NpcTag/ItemTag/Dead → Lua에서 true/false로 접근
  - EntityId 참조 컴포넌트 커스텀 핸들러: CombatTarget, InRoom, Inventory (u64 ↔ EntityId 변환)
  - 게임 로직 Lua 이전: 월드 생성, 명령어 처리 (look/move/attack/get/drop/inventory/say/who/help), 전투 시스템
  - Rust 코드 정리: systems/{look,movement,combat,inventory}.rs, world_setup.rs, output.rs 삭제/축소
  - main.rs: on_init 호출로 월드 생성, 틱 순서 변경 (on_action → on_tick)
- **Phase 4a (GridSpace — 2D 좌표 기반 공간 모델): 완료**
  - GridSpace: 정수 좌표 기반 2D 공간 모델 (BTreeMap/BTreeSet, 결정론적)
  - SpaceModel trait 구현 (셀 좌표 ↔ 합성 EntityId 인코딩, generation=u32::MAX)
  - GridSpace 전용 메서드: move_to, get_position, set_position, entities_in_radius, in_bounds
  - SpaceSnapshotData enum + SpaceSnapshotCapture trait (다형성 스냅샷)
  - persistence::snapshot 제네릭화: capture/restore<S: SpaceSnapshotCapture>
  - main.rs: --mode mud|grid 플래그 (grid 모드는 빈 TickLoop<GridSpace> 실행)
- **Phase 4b (Lua 스크립팅 GridSpace 통합): 완료**
  - SpaceProxy enum 리팩터링: SpaceKind(RoomGraph|Grid) + IntoSpaceKind trait
  - ScriptContext<'a, S: SpaceModel> 제네릭화, run_* 메서드에 IntoSpaceKind 바운드
  - Grid 전용 Lua API 7개: get_position, set_position, move_to, entities_in_radius, in_bounds, grid_config, entity_count
  - 공용 SpaceModel 메서드(entity_room, move_entity, place_entity, remove_entity) 양쪽 모두 동작
  - Grid 모드 main.rs: ScriptEngine 초기화, scripts_grid/ 디렉토리 로드, on_init/on_tick 훅 실행
- **Phase 4c (WebSocket 서버 + Grid 모드 네트워킹 MVP): 완료**
  - JSON 프로토콜: ClientMessage (Connect/Move/Action/Ping), ServerMessage (Welcome/EntityUpdate/EntityRemove/Error/Pong)
  - WebSocket 서버 (포트 4001, tokio-tungstenite): 세션 관리, reader/writer 분리
  - net crate 확장: protocol.rs, ws_server.rs 추가 (ecs_adapter/space 의존 없이 순수 wire 타입)
  - Grid 모드 채널 통합: MUD 모드와 동일한 채널 패턴 (PlayerTx/OutputTx/RegisterTx/UnregisterTx)
- **Phase 4d (AOI + Delta Snapshot): 완료**
  - AoiTracker: 세션별 AOI 상태 추적 (known 엔티티 BTreeMap)
  - AOI 필터링: Chebyshev 반경 32 내 엔티티만 전송 (entities_in_radius 재사용)
  - Delta Snapshot: StateDelta (entered/moved/left) — 전체 상태 대신 변경분만 전송
- **Phase 5a (Web Client MVP): 완료**
  - axum 0.8 + tower-http 0.6 기반 웹 서버 (단일 포트 4001: WS + 정적 파일 서빙)
  - TypeScript + Vite 6 + PixiJS v8 웹 클라이언트 (web_client/)
  - WASD/화살표 입력 (100ms 쓰로틀), 위치 보간 (lerp 0.18), 카메라 추적
  - Production: `cargo run -- --mode grid` → http://localhost:4001/
  - Dev: Vite proxy (`npm run dev`) → http://localhost:5173/
- **Phase 6a (ContentRegistry): 완료** — 275개 테스트 통과
  - JSON 기반 콘텐츠 정의 시스템 (content/*.json)
- **Phase 7 (Server Configuration): 완료**
  - ServerConfig 구조체 (TOML 파싱, serde default, CLI 오버라이드)
  - server.toml 설정 파일 (mode, net, tick, persistence, scripting, grid, database, security 섹션)
  - main.rs 하드코딩 상수 제거 → config 참조로 전환
  - `--config server.toml --mode mud|grid` CLI 지원
- **Phase 8 (Graceful Shutdown): 완료**
  - ShutdownTx/ShutdownRx (tokio watch 채널 기반)
  - SIGINT/SIGTERM 시그널 핸들링 (wait_for_signal)
  - 종료 시 최종 스냅샷 저장 + 전 세션 종료 메시지 + 리소스 정리
- **Phase 9 (Rate Limiting & Connection Security): 완료**
  - ConnectionLimiter: 전체/IP당 접속 수 제한 (Arc<Mutex> 공유)
  - CommandThrottle: 세션별 토큰 버킷 명령어 쓰로틀링
  - 최대 입력 길이 제한 (4096 bytes)
  - RateLimitConfig → ServerConfig [security] 섹션 연동
- **Phase 10 (Player Database): 완료**
  - player_db crate 신규 (rusqlite bundled + argon2 비밀번호 해싱)
  - SQL 스키마: accounts (username, password_hash, permission) + characters (components JSON, room_id, position)
  - AccountRepo: create, authenticate, get_by_username, set_permission
  - CharacterRepo: create, list_for_account, save_state, load, delete
  - PermissionLevel: Player(0) < Builder(1) < Admin(2) < Owner(3)
- **Phase 11 (Enhanced Login Flow & Session States): 완료**
  - 다단계 인증 흐름: AwaitingLogin → AwaitingPassword → AwaitingPasswordConfirm → SelectingCharacter → Playing
  - 신규 계정 생성 + 기존 계정 로그인 + 캐릭터 선택/생성
  - DB 캐릭터 로드 → ECS 엔티티 스폰 (Health, Attack, Defense, Name 복원)
  - 하위 호환: config.database.auth_required = false → 기존 quick-play 모드 유지
- **Phase 12 (Character Auto-Save & Reconnection): 완료**
  - LingeringEntity: 접속 해제 시 엔티티 월드 잔류 (linger_timeout 기반 타임아웃)
  - 재접속 시 linger 엔티티 재바인딩 (심리스 복원)
  - 주기적 캐릭터 자동 저장 (character_save_interval 틱 간격)
  - 종료 시 전체 캐릭터 + lingering 엔티티 DB 저장
- **Phase 13 (Admin System): 완료**
  - `hooks.on_admin(command, min_permission, fn)` Lua 훅 타입
  - Rust에서 permission >= min_permission 검증 후 Lua 콜백 호출 (보안 보장)
  - `/` 접두사 관리자 명령 파싱 (PlayerAction::Admin variant)
  - 04_admin.lua: kick, announce, teleport, stats, help 기본 GM 도구
- **Phase 14 (Telnet Enhancement — ANSI Colors + GMCP): 완료**
  - ANSI 색상 상수 + strip_ansi() + colorize() (net/src/ansi.rs)
  - GMCP 패키지: Char.Vitals, Room.Info + Telnet 서브네고시에이션 (net/src/gmcp.rs)
  - Lua `colors` 글로벌 테이블 (scripts/00_utils.lua)
  - 방 이름 bold cyan, 출구 green, 전투 메시지 yellow/red 색상 적용

**현재 테스트: 328개 전체 통과**

## 문서 위치

문서는 프로젝트 루트 `docs/` 디렉토리에 위치 (rust_mud_engine/ 외부):

- 아키텍처 설계: `docs/rust_mud_2d_engine_architecture_20260219.md`
- 전체 구현 계획: `docs/rust_mud_2d_engine_implementation_plan_20260219.md`
- Phase 1 구현 계획: `docs/phase1_implementation_plan.md`
- Phase 2 구현 계획: `docs/phase2_implementation_plan.md`
- Phase 3 구현 계획: `docs/phase3_implementation_plan.md`
- 데이터 설계: `docs/database_design.md` (콘텐츠=JSON 파일, 플레이어=SQLite, 엔진-게임 분리 현황)
- 엔티티 정의서: `docs/entity_definition.md` (계층별 [C]/[R]/[P] 구분, 공유 정의 분리)
- 속성 카탈로그: `docs/attribute_catalog.md` (열거형 속성값 + 공통 속성 사전)

## 코드 구조

```
rust_mud_engine/
├── Cargo.toml              (workspace + root package)
├── server.toml             서버 설정 파일 (TOML, Phase 7)
├── src/
│   ├── lib.rs              re-export 주요 crate (ecs_adapter, engine_core, mud, net, observability, persistence, session, space)
│   ├── main.rs             서버 바이너리 (tokio + tick thread, 로그인 상태머신, 자동저장)
│   ├── config.rs           ServerConfig — TOML 파싱, CLI 오버라이드, 기본값
│   └── shutdown.rs         ShutdownTx/ShutdownRx — watch 채널 기반 안전 종료
├── crates/
│   ├── ecs_adapter/        ECS 백엔드 격리 (bevy_ecs 래핑)
│   ├── engine_core/        TickLoop<S: SpaceModel>, CommandStream(LWW), EventBus
│   ├── space/              SpaceModel trait, RoomGraphSpace, GridSpace, SpaceSnapshotData
│   ├── observability/      init_logging(), TickMetrics
│   ├── plugin_abi/         WASM ABI 공유 타입 (no_std, WasmCommand)
│   ├── plugin_runtime/     WASM 플러그인 런타임 (wasmtime, Fuel, quarantine)
│   ├── session/            SessionId, SessionOutput, SessionManager, PlayerSession, LingeringEntity, PermissionLevel
│   ├── scripting/          Lua 스크립팅 엔진 (mlua/Luau, 샌드박스, Hook 시스템, on_admin 훅)
│   │   └── src/api/        Lua API 모듈별 분리 (ecs, space, session, output, log)
│   ├── mud/                MUD 게임 로직 (components, parser, systems, persistence_setup, script_setup, output/session re-exports)
│   ├── persistence/        PersistenceRegistry, PersistenceManager, Snapshot capture/restore, 디스크 I/O
│   ├── player_db/          SQLite 계정/캐릭터 DB (rusqlite bundled, argon2 해싱)
│   └── net/                Telnet, WebSocket, axum 웹 서버, ANSI, GMCP, rate limiter, 채널
├── plugins/                테스트용 WASM 플러그인 소스 (workspace exclude)
│   ├── test_movement/      3틱마다 MoveEntity 명령 발행
│   ├── test_infinite_loop/ 무한루프 (fuel exhaustion 테스트)
│   └── test_panic/         즉시 trap (quarantine 테스트)
├── scripts/                Lua 게임 스크립트
│   ├── 00_utils.lua        공용 헬퍼 (format_room, broadcast_room, HELP_TEXT, colors 테이블)
│   ├── 01_world_setup.lua  on_init 월드 생성 (6개 방 + 고블린 + 물약)
│   ├── 02_commands.lua     on_action 명령어 처리 (look/move/attack/get/drop/say/who/help)
│   ├── 03_combat.lua       on_tick 전투 해결 시스템 (ANSI 색상 적용)
│   └── 04_admin.lua        on_admin GM 도구 (kick/announce/teleport/stats/help)
├── web_dist/               웹 클라이언트 빌드 산출물 (vite build 결과)
├── test_fixtures/          사전 빌드된 .wasm 바이너리
└── tests/                  통합 테스트 (14개 파일)

web_client/                 웹 클라이언트 소스 (TypeScript + Vite + PixiJS)
├── package.json            pixi.js v8, vite v6, typescript v5
├── tsconfig.json           strict, ES2020, bundler moduleResolution
├── vite.config.ts          proxy /ws → :4001, build → rust_mud_engine/web_dist/
├── index.html              로그인 오버레이 + canvas 컨테이너
└── src/
    ├── main.ts             진입점 — 모듈 조립, 생명주기 관리
    ├── protocol.ts         서버 프로토콜 TypeScript 미러 (타입만)
    ├── state.ts            엔티티 상태 Map + delta 적용 로직
    ├── ws.ts               WebSocket 연결 관리 (connect/send/close)
    ├── input.ts            WASD 키보드 → Move 메시지 (100ms 쓰로틀)
    └── renderer.ts         PixiJS: 그리드 배경, 엔티티 원형, 이름 라벨, 카메라 추적
```

### Crate 의존 관계

```
engine_core → ecs_adapter, space(SpaceModel trait), observability, plugin_abi, plugin_runtime
plugin_runtime → plugin_abi, ecs_adapter, wasmtime
scripting → ecs_adapter, space, session, mlua (Luau VM)
session → ecs_adapter (엔진 레이어, 게임 비의존)
mud → ecs_adapter, space, session, persistence, scripting, bevy_ecs(derive only)
persistence → ecs_adapter, space (mud 의존 없음, PersistenceRegistry로 분리)
net → session, tokio, axum, tower-http (mud 의존 없음)
player_db → rusqlite(bundled), argon2, password-hash, serde_json, thiserror, tracing
space → ecs_adapter
observability → (독립)
plugin_abi → (독립, no_std)
ecs_adapter → bevy_ecs (내부만, 외부 노출 금지)
root package → 모든 crate + toml(설정 파싱)
```

### PersistenceRegistry 패턴

persistence crate는 `PersistentComponent` trait과 `PersistenceRegistry`를 제공.
게임 레이어(mud)에서 `register_mud_components()`로 12개 컴포넌트를 등록.
새 게임에서는 자체 컴포넌트를 같은 방식으로 등록하면 됨.

```rust
// mud/src/persistence_setup.rs
pub fn register_mud_components(registry: &mut PersistenceRegistry) { ... }

// main.rs
let mut registry = PersistenceRegistry::new();
register_mud_components(&mut registry);
snapshot::capture(&ecs, &space, tick, &registry);
snapshot::restore(snap, &mut ecs, &mut space, &registry)?;
```

### ScriptComponentRegistry 패턴

scripting crate는 `ScriptComponent` trait과 `ScriptComponentRegistry`를 제공.
PersistenceRegistry와 동일한 패턴으로, Lua table ↔ Rust Component 변환을 위한 trait-object 레지스트리.
게임 레이어(mud)에서 `register_mud_script_components()`로 12개 컴포넌트를 등록.

```rust
// mud/src/script_setup.rs
pub fn register_mud_script_components(registry: &mut ScriptComponentRegistry) { ... }

// main.rs
let mut script_engine = ScriptEngine::new(ScriptConfig::default())?;
register_mud_script_components(script_engine.component_registry_mut());
script_engine.load_directory(Path::new("scripts"))?;
```

### Lua 스크립팅 API

Lua 스크립트는 tick 스레드에서 직접 실행, ECS/Space에 직접 읽기/쓰기 가능:
- `ecs:get/set/has/remove/spawn/despawn/query` — ECS 컴포넌트 접근
- `space:entity_room/move_entity/place_entity/remove_entity` — 공용 SpaceModel (양쪽 모드)
- `space:room_occupants/register_room/room_exists/room_count/all_rooms/exits` — RoomGraph 전용 (Grid에서 Lua error)
- `space:get_position/set_position/move_to/entities_in_radius/in_bounds/grid_config/entity_count` — Grid 전용 (RoomGraph에서 Lua error)
- `output:send/broadcast_room` — 세션 출력
- `sessions:session_for/playing_list` — 세션 매핑 쿼리
- `hooks.on_init/on_tick/on_action/on_enter_room/on_connect` — 이벤트 훅 등록
- `hooks.on_admin(command, min_permission, fn)` — 관리자 명령 훅 (Rust에서 권한 검증 후 호출)
- `hooks.fire_enter_room(entity, room)` — Lua에서 on_enter_room 훅 직접 트리거
- `log.info/warn/error/debug` — tracing 연결
- `colors.*` — ANSI 색상 글로벌 테이블 (reset, bold, red, green, cyan, yellow 등)

### Player Database 패턴

player_db crate는 SQLite 기반 계정/캐릭터 영속성을 제공.
캐릭터 상태는 JSON blob(components 컬럼)으로 저장, ECS ↔ JSON 변환.

```rust
// main.rs
let player_db = PlayerDb::open(&config.database.path)?;
// 계정: create, authenticate, set_permission
let account = player_db.account().authenticate("user", "pass")?;
// 캐릭터: create, list_for_account, save_state, load, delete
let chars = player_db.character().list_for_account(account.id)?;
player_db.character().save_state(char_id, &components_json, room_id, position)?;
```

### Session State Machine

MUD 모드 로그인 흐름 (auth_required = true):

```
접속 → AwaitingLogin → 이름 입력
  → 기존 계정: AwaitingPassword → 인증 성공 → SelectingCharacter
  → 신규 계정: AwaitingPassword(is_new) → AwaitingPasswordConfirm → SelectingCharacter
  → 캐릭터 선택/생성 → Playing
  → 접속 해제 → LingeringEntity (linger_timeout 후 DB 저장 + despawn)
  → 재접속 + 같은 캐릭터 → rebind_lingering (심리스 복원)
```

auth_required = false (기본값): 기존 quick-play 모드 유지 (이름만 입력 → Playing)

## 핵심 설계 원칙 (위반 금지)

1. **bevy_ecs 타입 미노출**: bevy_ecs는 ecs_adapter 내부에만 존재. 다른 crate에서 직접 의존 금지
2. **Plugin Stateless**: WASM 플러그인은 내부 상태 저장 금지. 모든 게임 상태는 ECS에 저장
3. **Command Stream 간접 수정**: Plugin이 ECS를 직접 수정 금지. EngineCommand로만 상태 변경
4. **Last Writer Wins (LWW)**: 같은 Entity+Component에 대한 마지막 Command가 승리
5. **Fuel = 결정론적 파라미터**: 동일 입력 + 동일 Fuel = 동일 결과
6. **단일 쓰기 스레드**: Tick thread만 World 상태 수정 가능. async에서 직접 접근 금지
7. **EntityId = generation + index**: 단순 u64 증가 아닌 세대 기반 (Snapshot 복원 안전)
8. **Lua 스크립트 = 샌드박스**: 메모리 제한(16MB), 명령어 제한(1M), require 금지. 게임메이커 보안 보장
9. **엔진-게임 완전 분리**: 엔진 crate(engine_core, scripting, persistence, net, space, session)는 게임별 스키마(MonsterDef, ItemDef 등)를 모름. 게임 데이터는 동적 처리(serde_json::Value), 게임 로직은 Lua
10. **콘텐츠 = JSON 파일, DB = 플레이어만**: 게임 정의 데이터(몬스터/아이템/스킬 등)는 content/*.json (ContentRegistry로 로드, 디렉토리는 필요 시 생성), SQLite는 계정/캐릭터/길드 영속성 전용

## 빌드 & 테스트

Rust가 기본 PATH에 없으므로 반드시 PATH 설정 필요:

```bash
export PATH="/home/genos/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH"

# 전체 빌드
cargo build --workspace

# 전체 테스트
cargo test --workspace

# 개별 crate
cargo test -p ecs_adapter
cargo test -p engine_core
cargo test -p space
cargo test -p plugin_abi
cargo test -p plugin_runtime
cargo test -p session
cargo test -p mud
cargo test -p persistence
cargo test -p net
cargo test -p scripting
cargo test -p player_db

# 통합 테스트 (상세 출력)
cargo test --test tick_simulation -- --nocapture
cargo test --test tick_determinism -- --nocapture
cargo test --test wasm_plugin_test -- --nocapture
cargo test --test fuel_determinism -- --nocapture
cargo test --test game_systems_integration -- --nocapture
cargo test --test snapshot_integration -- --nocapture
cargo test --test server_integration -- --nocapture
cargo test --test grid_space_test -- --nocapture
cargo test --test grid_tick_integration -- --nocapture
cargo test --test grid_scripting_test -- --nocapture
cargo test --test ws_grid_integration -- --nocapture
cargo test --test content_registry_test -- --nocapture
cargo test --test space_test -- --nocapture
cargo test --test memory_grow_stress -- --nocapture

# WASM 플러그인 빌드 (test_fixtures 업데이트 시)
cargo build --target wasm32-unknown-unknown --release --manifest-path plugins/test_movement/Cargo.toml
# 빌드 후 test_fixtures/로 복사 필요

# 웹 클라이언트
cd /home/genos/workspace/project_g/web_client
npm install          # 의존성 설치
npm run build        # 프로덕션 빌드 → rust_mud_engine/web_dist/
npm run dev          # 개발 서버 (Vite HMR, :5173, /ws proxy → :4001)

# 서버 실행
cd /home/genos/workspace/project_g/rust_mud_engine
cargo run -- --config server.toml --mode grid   # Grid 모드 (http://localhost:4001/)
cargo run -- --config server.toml --mode mud    # MUD 모드 (telnet localhost 4000)
```

## 기술 스택

| 영역 | 선택 |
|------|------|
| Rust edition | 2021 |
| ECS | bevy_ecs 0.15 (default-features = false) |
| 직렬화 (내부) | serde + bincode |
| 직렬화 (WASM ABI) | serde + postcard |
| WASM Runtime | wasmtime 41 |
| WASM Target | wasm32-unknown-unknown (no WASI) |
| Lua 스크립팅 | mlua 0.10 (Luau, vendored, send, serialize) |
| 설정 파싱 | toml 0.8 |
| 데이터베이스 | rusqlite 0.32 (bundled SQLite) |
| 비밀번호 해싱 | argon2 0.5 + password-hash 0.5 |
| 로깅 | tracing + tracing-subscriber |
| 에러 타입 | thiserror |
| 네트워크 | tokio 1 (full features) |
| WebSocket | tokio-tungstenite 0.24 + futures-util 0.3 |
| 웹 서버 | axum 0.8 (ws) + tower-http 0.6 (fs) |
| JSON 프로토콜 | serde_json 1 |
| 웹 클라이언트 | TypeScript 5.7 + Vite 6 + PixiJS 8 |
| Telnet 포트 | 0.0.0.0:4000 (server.toml에서 설정 가능) |
| 웹/WS 포트 | 0.0.0.0:4001 (Grid 모드, WS + 정적 파일, 설정 가능) |

## 코딩 컨벤션

- 한국어로 소통, 코드/주석은 영어
- 결정론 보장: 정렬된 순서로 iteration (Vec sort, BTreeMap)
- HashMap iteration 결과에 의존하는 로직 금지 (정렬 후 사용)
- 모든 public API는 Result 반환 (unwrap 금지, 테스트 제외)
- Component는 순수 데이터 (로직 없음)
