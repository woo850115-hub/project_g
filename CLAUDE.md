# Project G — Rust MUD/2D MMORPG 엔진

## 프로젝트 개요

Rust 기반 MUD/2D MMORPG 겸용 게임 엔진. 공유 엔진 코어 위에 Text MUD(project_mud)와 2D MMO(project_2d) 두 게임 프로젝트가 분리.
게임 로직은 WASM 플러그인 + Lua 스크립트로 분리, 결정론적 시뮬레이션 루프가 핵심.

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
- **Phase 3.5 (Lua 게임 로직 마이그레이션): 완료**
- **Phase 4a (GridSpace — 2D 좌표 기반 공간 모델): 완료**
- **Phase 4b (Lua 스크립팅 GridSpace 통합): 완료**
- **Phase 4c (WebSocket 서버 + Grid 모드 네트워킹 MVP): 완료**
- **Phase 4d (AOI + Delta Snapshot): 완료**
- **Phase 5a (Web Client MVP): 완료**
- **Phase 6a (ContentRegistry): 완료**
- **Phase 7 (Server Configuration): 완료**
- **Phase 8 (Graceful Shutdown): 완료**
- **Phase 9 (Rate Limiting & Connection Security): 완료**
- **Phase 10 (Player Database): 완료**
- **Phase 11 (Enhanced Login Flow & Session States): 완료**
- **Phase 12 (Character Auto-Save & Reconnection): 완료**
- **Phase 13 (Admin System): 완료**
- **Phase 14 (Telnet Enhancement — ANSI Colors + GMCP): 완료**
- **Phase 15 (프로젝트 분리): 완료**
  - 단일 rust_mud_engine/ → engine/ + project_mud/ + project_2d/ 분리
  - 루트 가상 워크스페이스 (단일 Cargo.lock, workspace.dependencies 공유)
  - 엔진 crate 10개: engine/crates/ 이동
  - project_mud: MUD 전용 바이너리 + mud/player_db crate + MUD 테스트 10개
  - project_2d: Grid 전용 바이너리 + 독립 Name 컴포넌트 + Grid 테스트 4개

**현재 테스트: 337개 전체 통과**

## 문서 위치

문서는 프로젝트 루트 `docs/` 디렉토리에 위치:

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
project_g/
├── Cargo.toml                  가상 워크스페이스 (단일 Cargo.lock 공유)
├── Cargo.lock
├── engine/crates/              공유 엔진 crate 10개
│   ├── ecs_adapter/            ECS 백엔드 격리 (bevy_ecs 래핑)
│   ├── engine_core/            TickLoop<S: SpaceModel>, CommandStream(LWW), EventBus
│   ├── space/                  SpaceModel trait, RoomGraphSpace, GridSpace, SpaceSnapshotData
│   ├── observability/          init_logging(), TickMetrics
│   ├── plugin_abi/             WASM ABI 공유 타입 (no_std, WasmCommand)
│   ├── plugin_runtime/         WASM 플러그인 런타임 (wasmtime, Fuel, quarantine)
│   ├── session/                SessionId, SessionOutput, SessionManager, PlayerSession, LingeringEntity, PermissionLevel
│   ├── scripting/              Lua 스크립팅 엔진 (mlua/Luau, 샌드박스, Hook 시스템, on_admin 훅)
│   │   └── src/api/            Lua API 모듈별 분리 (ecs, space, session, output, log)
│   ├── persistence/            PersistenceRegistry, PersistenceManager, Snapshot capture/restore, 디스크 I/O
│   └── net/                    Telnet, WebSocket, axum 웹 서버, ANSI, GMCP, rate limiter, 채널
├── project_mud/                MUD 게임 프로젝트
│   ├── Cargo.toml              바이너리 패키지 (mud_server)
│   ├── src/
│   │   ├── main.rs             MUD 전용 서버 (tokio + tick thread, 로그인 상태머신, 자동저장)
│   │   ├── config.rs           MUD ServerConfig (net, tick, persistence, scripting, database, security, character)
│   │   └── shutdown.rs         ShutdownTx/ShutdownRx — watch 채널 기반 안전 종료
│   ├── crates/
│   │   ├── mud/                MUD 게임 로직 (components, parser, systems, persistence_setup, script_setup)
│   │   └── player_db/          SQLite 계정/캐릭터 DB (rusqlite bundled, argon2 해싱)
│   ├── scripts/                Lua 게임 스크립트
│   │   ├── 00_utils.lua        공용 헬퍼 (format_room, broadcast_room, HELP_TEXT, colors 테이블)
│   │   ├── 01_world_setup.lua  on_init 월드 생성 (6개 방 + 고블린 + 물약)
│   │   ├── 02_commands.lua     on_action 명령어 처리 (look/move/attack/get/drop/say/who/help)
│   │   ├── 03_combat.lua       on_tick 전투 해결 시스템 (ANSI 색상 적용)
│   │   └── 04_admin.lua        on_admin GM 도구 (kick/announce/teleport/stats/help)
│   ├── server.toml             MUD 서버 설정
│   ├── test_fixtures/          사전 빌드된 .wasm 바이너리
│   ├── data/                   런타임 데이터 (snapshots, player.db)
│   └── tests/                  MUD + 엔진 통합 테스트 (10개)
├── project_2d/                 2D Grid 게임 프로젝트
│   ├── Cargo.toml              바이너리 + 라이브러리 패키지 (grid_server, project_2d)
│   ├── src/
│   │   ├── main.rs             Grid 전용 서버 (WebSocket, AOI, Delta Snapshot)
│   │   ├── lib.rs              pub mod components
│   │   ├── components.rs       Name 컴포넌트 (독립 정의)
│   │   ├── config.rs           Grid ServerConfig (net, tick, scripting, grid, security)
│   │   └── shutdown.rs         ShutdownTx/ShutdownRx
│   ├── web_client/             TypeScript + Vite + PixiJS 웹 클라이언트
│   ├── web_dist/               빌드된 클라이언트 정적 파일
│   ├── server.toml             Grid 서버 설정
│   └── tests/                  Grid 통합 테스트 (4개)
├── plugins/                    테스트용 WASM 플러그인 소스 (workspace exclude)
│   ├── test_movement/          3틱마다 MoveEntity 명령 발행
│   ├── test_infinite_loop/     무한루프 (fuel exhaustion 테스트)
│   └── test_panic/             즉시 trap (quarantine 테스트)
├── docs/                       설계 문서
└── README.md
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
project_mud → 엔진 crate 전체 + mud + player_db
project_2d → 엔진 crate (persistence 제외) + bevy_ecs(derive only)
```

### PersistenceRegistry 패턴

persistence crate는 `PersistentComponent` trait과 `PersistenceRegistry`를 제공.
게임 레이어(mud)에서 `register_mud_components()`로 12개 컴포넌트를 등록.
새 게임에서는 자체 컴포넌트를 같은 방식으로 등록하면 됨.

```rust
// project_mud/crates/mud/src/persistence_setup.rs
pub fn register_mud_components(registry: &mut PersistenceRegistry) { ... }

// project_mud/src/main.rs
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
// project_mud/crates/mud/src/script_setup.rs
pub fn register_mud_script_components(registry: &mut ScriptComponentRegistry) { ... }

// project_mud/src/main.rs
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
// project_mud/src/main.rs
let player_db = PlayerDb::open(&config.database.path)?;
let account = player_db.account().authenticate("user", "pass")?;
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

# 개별 엔진 crate
cargo test -p ecs_adapter
cargo test -p engine_core
cargo test -p space
cargo test -p plugin_abi
cargo test -p plugin_runtime
cargo test -p session
cargo test -p persistence
cargo test -p net
cargo test -p scripting

# 게임 프로젝트별
cargo test -p project_mud
cargo test -p project_2d
cargo test -p mud
cargo test -p player_db

# 통합 테스트 (상세 출력, -p로 프로젝트 지정)
cargo test -p project_mud --test tick_simulation -- --nocapture
cargo test -p project_mud --test tick_determinism -- --nocapture
cargo test -p project_mud --test wasm_plugin_test -- --nocapture
cargo test -p project_mud --test fuel_determinism -- --nocapture
cargo test -p project_mud --test game_systems_integration -- --nocapture
cargo test -p project_mud --test snapshot_integration -- --nocapture
cargo test -p project_mud --test server_integration -- --nocapture
cargo test -p project_mud --test content_registry_test -- --nocapture
cargo test -p project_mud --test space_test -- --nocapture
cargo test -p project_mud --test memory_grow_stress -- --nocapture
cargo test -p project_2d --test grid_space_test -- --nocapture
cargo test -p project_2d --test grid_tick_integration -- --nocapture
cargo test -p project_2d --test grid_scripting_test -- --nocapture
cargo test -p project_2d --test ws_grid_integration -- --nocapture

# WASM 플러그인 빌드 (test_fixtures 업데이트 시)
cargo build --target wasm32-unknown-unknown --release --manifest-path plugins/test_movement/Cargo.toml
# 빌드 후 project_mud/test_fixtures/로 복사 필요

# 웹 클라이언트
cd /home/genos/workspace/project_g/project_2d/web_client
npm install          # 의존성 설치
npm run build        # 프로덕션 빌드 → project_2d/web_dist/
npm run dev          # 개발 서버 (Vite HMR, :5173, /ws proxy → :4001)

# 서버 실행
cargo run -p project_mud -- --config project_mud/server.toml    # MUD 서버 (telnet localhost 4000)
cargo run -p project_2d -- --config project_2d/server.toml      # Grid 서버 (http://localhost:4001/)
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
| Telnet 포트 | 0.0.0.0:4000 (project_mud/server.toml에서 설정 가능) |
| 웹/WS 포트 | 0.0.0.0:4001 (project_2d/server.toml에서 설정 가능) |

## 코딩 컨벤션

- 한국어로 소통, 코드/주석은 영어
- 결정론 보장: 정렬된 순서로 iteration (Vec sort, BTreeMap)
- HashMap iteration 결과에 의존하는 로직 금지 (정렬 후 사용)
- 모든 public API는 Result 반환 (unwrap 금지, 테스트 제외)
- Component는 순수 데이터 (로직 없음)
