# Project G — Rust MUD / 2D MMORPG Engine

Rust 기반 **Text MUD**와 **2D MMORPG**를 단일 서버 코어로 동시 지원하는 게임 엔진.

- **MUD 모드**: Telnet 접속, 방 기반 탐험/전투, ANSI 컬러, GMCP
- **Grid 모드**: WebSocket + 웹 클라이언트, 2D 좌표 이동, AOI 기반 Delta Snapshot

게임 로직은 **Lua 스크립트**로 작성하고, 엔진은 ECS + 결정론적 틱 루프로 동작합니다.

## Features

| 영역 | 설명 |
|------|------|
| ECS | bevy_ecs 기반 Entity-Component-System, 결정론적 시뮬레이션 |
| Lua Scripting | Luau 샌드박스 (메모리/명령어 제한), Hook 시스템 (on_tick, on_action, on_admin 등) |
| WASM Plugins | wasmtime 기반 플러그인, Fuel 제한, 자동 quarantine |
| Dual Space Model | RoomGraphSpace (MUD) + GridSpace (2D MMO), SpaceModel trait으로 추상화 |
| Networking | Telnet (MUD) + WebSocket/HTTP (Grid), ANSI 색상, GMCP 프로토콜 |
| Persistence | 월드 스냅샷 (bincode), 캐릭터 자동 저장 (SQLite JSON blob) |
| Account System | SQLite + argon2 비밀번호 해싱, 다단계 로그인, 권한 관리 (Player/Builder/Admin/Owner) |
| Admin Tools | Lua 기반 GM 명령 (/kick, /announce, /teleport 등), 권한 검증 |
| Security | IP별 접속 제한, 명령어 Rate Limiting, 입력 길이 제한, Graceful Shutdown |
| Web Client | TypeScript + PixiJS 8, WASD 이동, 위치 보간, AOI Delta 수신 |
| Configuration | TOML 설정 파일 + CLI 오버라이드 |

## Quick Start

### Requirements

- Rust 1.75+ (2021 edition)
- Node.js 18+ (웹 클라이언트 빌드 시)

### MUD Mode (Telnet)

```bash
cd rust_mud_engine
cargo run -- --config server.toml --mode mud
```

```bash
# 다른 터미널에서 접속
telnet localhost 4000
```

### Grid Mode (Web)

```bash
# 웹 클라이언트 빌드
cd web_client
npm install && npm run build

# 서버 실행
cd ../rust_mud_engine
cargo run -- --config server.toml --mode grid
```

브라우저에서 http://localhost:4001/ 접속.

### Development (HMR)

```bash
# 터미널 1: 서버
cd rust_mud_engine && cargo run -- --mode grid

# 터미널 2: Vite dev server (HMR)
cd web_client && npm run dev
# → http://localhost:5173/
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                    main.rs                       │
│         (tokio async + tick thread)              │
├──────────┬──────────┬──────────┬────────────────┤
│   net    │ session  │ scripting│   player_db    │
│ Telnet   │ Sessions │ Lua VM   │ SQLite+argon2  │
│ WebSocket│ States   │ Hooks    │ Accounts       │
│ HTTP     │ Linger   │ Sandbox  │ Characters     │
├──────────┴──────────┴──────────┴────────────────┤
│                 engine_core                       │
│    TickLoop<S: SpaceModel> + CommandStream        │
├──────────┬──────────┬───────────────────────────┤
│   space  │   mud    │      persistence           │
│ RoomGraph│ Comps    │  Snapshot capture/restore   │
│ GridSpace│ Parser   │  PersistenceRegistry        │
├──────────┴──────────┴───────────────────────────┤
│              ecs_adapter (bevy_ecs)               │
└─────────────────────────────────────────────────┘
```

### Crate 구조 (12 crates)

| Crate | 역할 |
|-------|------|
| `ecs_adapter` | bevy_ecs 래핑, 외부 노출 차단 |
| `engine_core` | TickLoop, CommandStream (LWW), EventBus |
| `space` | SpaceModel trait, RoomGraphSpace, GridSpace |
| `session` | SessionManager, PlayerSession, LingeringEntity, PermissionLevel |
| `scripting` | ScriptEngine (mlua/Luau), Hook 시스템, ContentRegistry |
| `mud` | MUD 게임 컴포넌트, 파서, 시스템 |
| `persistence` | PersistenceRegistry, Snapshot I/O |
| `player_db` | SQLite 계정/캐릭터 DB, argon2 해싱 |
| `net` | Telnet, WebSocket, axum, ANSI, GMCP, Rate Limiter |
| `plugin_abi` | WASM ABI 공유 타입 (no_std) |
| `plugin_runtime` | wasmtime WASM 런타임, Fuel, quarantine |
| `observability` | tracing 로깅, TickMetrics |

## Lua Scripting

게임 로직은 `scripts/` 디렉토리의 Lua 파일로 작성합니다.

```lua
-- Hook: 매 틱마다 실행
hooks.on_tick(function(tick)
    local combatants = ecs:query("CombatTarget")
    for _, attacker in ipairs(combatants) do
        -- 전투 처리 로직
    end
end)

-- Hook: 플레이어 명령어 처리
hooks.on_action(function(info)
    if info.action == "look" then
        local room = space:entity_room(info.entity)
        output:send(info.session_id, format_room(room, info.entity))
    end
end)

-- Hook: 관리자 명령 (권한 검증은 Rust에서 처리)
hooks.on_admin("kick", 2, function(ctx)
    -- Admin 전용 명령
end)
```

### Lua API

| 네임스페이스 | API |
|-------------|-----|
| `ecs` | `get`, `set`, `has`, `remove`, `spawn`, `despawn`, `query` |
| `space` | `entity_room`, `move_entity`, `place_entity`, `room_occupants`, `exits` (MUD) |
| `space` | `get_position`, `set_position`, `move_to`, `entities_in_radius` (Grid) |
| `output` | `send`, `broadcast_room` |
| `sessions` | `session_for`, `playing_list` |
| `hooks` | `on_init`, `on_tick`, `on_action`, `on_enter_room`, `on_connect`, `on_admin` |
| `log` | `info`, `warn`, `error`, `debug` |
| `colors` | ANSI 색상 테이블 (`reset`, `bold`, `red`, `green`, `cyan`, `yellow` 등) |

## Configuration

`server.toml`로 서버 설정을 관리합니다. 모든 항목에 기본값이 있어 파일이 없어도 동작합니다.

```toml
mode = "mud"                        # mud | grid

[net]
telnet_addr = "0.0.0.0:4000"
ws_addr = "0.0.0.0:4001"
max_connections = 1000

[tick]
tps = 10

[database]
path = "data/player.db"
auth_required = false               # true: 계정 로그인, false: quick-play

[security]
max_connections_per_ip = 5
max_commands_per_second = 20
```

CLI로 오버라이드 가능:

```bash
cargo run -- --config server.toml --mode grid
```

## Testing

```bash
cd rust_mud_engine

# 전체 테스트 (328개)
cargo test --workspace

# 개별 crate
cargo test -p player_db
cargo test -p scripting
cargo test -p net

# 통합 테스트
cargo test --test server_integration -- --nocapture
cargo test --test ws_grid_integration -- --nocapture
```

## Tech Stack

| 영역 | 기술 |
|------|------|
| Language | Rust 2021 edition |
| ECS | bevy_ecs 0.15 |
| Scripting | mlua 0.10 (Luau) |
| WASM | wasmtime 41 |
| Async | tokio 1 |
| Web Server | axum 0.8 + tower-http 0.6 |
| WebSocket | tokio-tungstenite 0.24 |
| Database | rusqlite 0.32 (bundled SQLite) |
| Password | argon2 0.5 |
| Config | toml 0.8 |
| Serialization | serde + bincode / serde_json |
| Web Client | TypeScript 5.7 + Vite 6 + PixiJS 8 |

## Project Structure

```
project_g/
├── rust_mud_engine/           Rust 서버 엔진
│   ├── src/                   서버 바이너리 (main, config, shutdown)
│   ├── crates/                12개 워크스페이스 crate
│   ├── scripts/               MUD Lua 스크립트 (월드, 명령어, 전투, 관리자)
│   ├── server.toml            서버 설정 파일
│   └── tests/                 통합 테스트 (14개)
├── web_client/                TypeScript + PixiJS 웹 클라이언트
│   └── src/                   (main, protocol, state, ws, input, renderer)
└── docs/                      설계 문서
```

## License

Private — All rights reserved.
