# Phase 2 구현 계획: Playable MUD + Persistence

## Context

Phase 0(엔진 코어 골격)과 Phase 1(WASM Runtime 통합)이 완료되었다.
42개 테스트가 전부 통과하며, ECS + Command Stream + Event Bus + Space Model + WASM Plugin Runtime이 동작한다.

Phase 2의 목표는 **텍스트 기반 MUD를 실제로 플레이할 수 있는 상태**로 만드는 것이다.
Telnet으로 접속하여 이동, 전투, 아이템 획득이 가능하고, 서버 재시작 후 캐릭터 상태가 복구되어야 한다.

## 신규 Crate 구조

```
rust_mud_engine/
├── crates/
│   ├── (기존) ecs_adapter/
│   ├── (기존) engine_core/
│   ├── (기존) space/
│   ├── (기존) observability/
│   ├── (기존) plugin_abi/
│   ├── (기존) plugin_runtime/
│   ├── net/                     ← [신규] 네트워크 + 세션 계층
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── transport.rs     ← Transport trait 추상화
│   │       ├── telnet.rs        ← TelnetTransport (IAC 핸들링)
│   │       ├── session.rs       ← Session, SessionManager, AuthState
│   │       ├── line_buffer.rs   ← LineBuffer (분할 패킷 누적)
│   │       └── channel.rs       ← SessionInput/SessionOutput, 채널 타입 정의
│   ├── persistence/             ← [신규] Snapshot 저장/복원
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── snapshot.rs      ← WorldSnapshot, EntitySnapshot, RoomSnapshot
│   │       └── manager.rs       ← SnapshotManager (저장 주기, 로테이션)
│   └── mud/                     ← [신규] MUD 게임 로직
│       └── src/
│           ├── lib.rs
│           ├── components.rs    ← Health, Name, Description, Inventory 등 게임 Component
│           ├── commands.rs      ← GameCommand enum, parse_command()
│           ├── systems.rs       ← MovementSystem, CombatSystem, LookSystem
│           ├── output.rs        ← 텍스트 출력 포매팅 (Room 묘사, 전투 메시지)
│           └── world_builder.rs ← 초기 월드 생성 (테스트용 맵)
├── src/
│   └── main.rs                  ← [신규] 서버 바이너리 진입점
└── plugins/
    └── npc_ai/                  ← [신규] NPC AI WASM 플러그인
        └── src/lib.rs
```

## 의존 관계 (Phase 2 신규 crate 포함)

```
서버 binary (main.rs)
  → engine_core → ecs_adapter, space, observability, plugin_abi, plugin_runtime
  → net → (tokio, 독립)
  → persistence → ecs_adapter, space, mud (직렬화를 위해)
  → mud → ecs_adapter, space, plugin_abi
```

```
net: tokio, tracing
persistence: ecs_adapter, space, mud, serde, bincode, tracing
mud: ecs_adapter, space, plugin_abi, serde, tracing
```

핵심 원칙:
- `net`은 게임 로직에 무관 (SessionInput/SessionOutput만 주고받음)
- `mud`는 네트워크에 무관 (입력 문자열 → GameCommand 변환, 출력 문자열 생성)
- `persistence`는 ECS 상태를 통째로 직렬화/역직렬화
- Tick thread만 World 상태 수정 (async에서 직접 접근 금지)

## 기술 스택 추가

| 영역 | 선택 | 근거 |
|------|------|------|
| Async Runtime | tokio 1.x (rt-multi-thread) | 업계 표준, 네트워크 I/O |
| Telnet | 직접 구현 (최소 IAC) | 라이브러리 불필요한 수준의 단순 프로토콜 |
| Channel | tokio::sync::mpsc | Tick thread ↔ Async 간 통신 |
| UUID | uuid 1.x (v4) | SessionId 생성 |

---

## 구현 단계 (16 Steps)

### Step 1: Cargo workspace 확장 + 신규 crate 골격

- workspace members에 `net`, `persistence`, `mud` 추가
- 각 crate의 Cargo.toml + 빈 lib.rs 생성
- root Cargo.toml의 [dependencies]에 신규 crate 추가
- workspace.dependencies에 `tokio`, `uuid` 추가
- **검증:** `cargo build --workspace` 성공

### Step 2: 게임 Component 정의 (mud/components.rs)

ECS Component로 사용할 게임 데이터 타입을 정의한다.

```rust
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Name(pub String);

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Description(pub String);

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Attack {
    pub damage_min: i32,
    pub damage_max: i32,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Defense {
    pub armor: i32,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    pub items: Vec<u64>,       // EntityId를 u64로 저장
    pub capacity: usize,
}

/// Player를 표시하는 마커 Component
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct PlayerTag;

/// NPC를 표시하는 마커 Component
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct NpcTag {
    pub ai_type: String,       // AI 유형 (e.g., "aggressive", "passive")
}

/// Room 설명 Component (RoomGraphSpace의 EntityId에 부착)
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct RoomDescription {
    pub name: String,
    pub description: String,
}

/// Item 데이터 Component
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ItemData {
    pub name: String,
    pub description: String,
    pub item_type: ItemType,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ItemType {
    Weapon { damage_bonus: i32 },
    Armor { defense_bonus: i32 },
    Consumable { heal_amount: i32 },
    Misc,
}

/// 세션 바인딩 Component (어떤 Session이 이 Entity를 제어하는지)
#[derive(Component, Clone, Debug)]
pub struct SessionBinding {
    pub session_id: u64,
}
```

- 모든 게임 Component에 `Serialize`/`Deserialize` derive (Snapshot용)
- `ecs_adapter::Component` derive로 bevy_ecs Component 자동 등록
- **검증:** Component CRUD 단위 테스트, bincode round-trip

### Step 3: 텍스트 명령 파서 (mud/commands.rs)

```rust
pub enum Direction {
    North, South, East, West,
    Custom(String),
}

pub enum GameCommand {
    Move(Direction),
    Look,
    Say(String),
    Attack(String),        // 대상 이름
    Inventory,
    Get(String),           // 아이템 이름
    Drop(String),
    Use(String),
    Quit,
    Help,
    Who,                   // 접속자 목록
    Unknown(String),       // 인식 불가 명령
}

pub fn parse_command(input: &str) -> GameCommand {
    let input = input.trim();
    let parts: Vec<&str> = input.split_whitespace().collect();
    match parts.first().map(|s| s.to_lowercase()).as_deref() {
        Some("north" | "n") => GameCommand::Move(Direction::North),
        Some("south" | "s") => GameCommand::Move(Direction::South),
        // ... 이하 생략
        _ => GameCommand::Unknown(input.to_string()),
    }
}
```

- 대소문자 무관 매칭
- 축약어 지원 (n/s/e/w, l, i, etc.)
- **검증:** 모든 명령의 파싱 단위 테스트

### Step 4: 채널 타입 정의 (net/channel.rs)

Tick thread와 Async 네트워크 간 통신에 사용하는 메시지 타입을 정의한다.

```rust
pub type SessionId = u64;

/// Async → Tick Thread 방향 메시지
pub enum SessionInput {
    /// 새 연결 (세션 생성)
    Connected { session_id: SessionId },
    /// 텍스트 라인 입력 (LineBuffer에서 완성된 줄)
    Line { session_id: SessionId, line: String },
    /// 연결 종료
    Disconnected { session_id: SessionId },
}

/// Tick Thread → Async 방향 메시지
pub enum SessionOutput {
    /// 특정 세션에 텍스트 전송
    SendText { session_id: SessionId, text: String },
    /// 세션 강제 종료
    Disconnect { session_id: SessionId },
}
```

- Unbounded channel 사용 (tick thread가 blocking되면 안 됨)
- **검증:** 타입 정의 + 컴파일

### Step 5: LineBuffer (net/line_buffer.rs)

```rust
pub struct LineBuffer {
    buffer: Vec<u8>,
    max_size: usize,  // 기본 4096, DoS 방지
}

impl LineBuffer {
    pub fn new(max_size: usize) -> Self;
    /// raw bytes를 누적, 완성된 줄(개행 기준)을 반환
    pub fn feed(&mut self, data: &[u8]) -> Vec<String>;
    pub fn is_empty(&self) -> bool;
}
```

- `\n` 기준 줄 분리 (MUD 표준)
- `\r\n` 처리 (Telnet은 CR+LF)
- buffer overflow 시 clear (DoS 방지)
- **검증:** 분할 패킷 테스트, overflow 테스트, CR+LF 처리

### Step 6: Telnet Transport (net/telnet.rs)

```rust
/// 최소 Telnet 구현.
/// IAC 바이트(0xFF) 필터링 + WILL/WONT/DO/DONT 기본 응답.
pub struct TelnetCodec {
    state: TelnetState,
}

enum TelnetState {
    Data,
    Iac,
    Will,
    Wont,
    Do,
    Dont,
    SubNeg,
    SubNegIac,
}

impl TelnetCodec {
    pub fn new() -> Self;
    /// raw bytes에서 IAC 시퀀스를 제거하고 순수 데이터만 반환.
    /// 필요한 IAC 응답(WONT/DONT)이 있으면 함께 반환.
    pub fn process(&mut self, data: &[u8]) -> TelnetResult;
}

pub struct TelnetResult {
    pub data: Vec<u8>,           // IAC 제거 후 순수 데이터
    pub responses: Vec<Vec<u8>>, // IAC 응답 (WONT/DONT 등)
}
```

Phase 2 범위:
- IAC WILL/WONT/DO/DONT 기본 핸들링 (모든 옵션에 WONT/DONT 응답)
- IAC 바이트 이스케이프 (0xFF 0xFF → 0xFF)
- Line Mode 동작 (클라이언트가 줄 단위로 전송)
- Sub-negotiation 무시 (IAC SB ... IAC SE)

Phase 2 범위 밖:
- NAWS, MCCP, MSDP, GMCP → Phase 3+
- Character Mode → Phase 3+

- **검증:** IAC 필터링 단위 테스트, 이스케이프 테스트, 순수 텍스트 통과 테스트

### Step 7: 네트워크 서버 + Session Manager (net/lib.rs, net/session.rs)

```rust
/// Async TCP 서버. Telnet 연결을 수락하고 세션을 관리한다.
pub async fn run_server(
    addr: SocketAddr,
    input_tx: UnboundedSender<SessionInput>,
    output_rx: UnboundedReceiver<SessionOutput>,
) -> Result<(), NetError>;
```

내부 동작:
1. `TcpListener::bind(addr)` → accept loop
2. 새 연결 시:
   - SessionId 할당 (atomic counter)
   - `input_tx.send(SessionInput::Connected { session_id })`
   - spawn `handle_client(socket, session_id, input_tx, output_tx)`
3. `handle_client`:
   - read loop: `socket.read()` → TelnetCodec → LineBuffer → 완성된 줄마다 `input_tx.send(Line)`
   - write task: output_rx에서 해당 session_id의 메시지 수신 → socket.write()
   - 연결 종료 시: `input_tx.send(Disconnected)`

Output 라우팅:
- 별도 task가 `output_rx`에서 메시지를 수신
- `SessionOutput::SendText { session_id, text }` → 해당 session의 write half로 전달
- session별 write channel(oneshot 또는 mpsc)로 라우팅

```rust
pub struct SessionManager {
    sessions: HashMap<SessionId, SessionState>,
    next_id: AtomicU64,
}

pub struct SessionState {
    pub player_entity: Option<EntityId>,
    pub auth_state: AuthState,
}

pub enum AuthState {
    AwaitingName,
    AwaitingPassword { name: String },
    Authenticated { name: String },
}
```

- **검증:** 단위 테스트 (mock TCP), 수동 Telnet 접속 테스트

### Step 8: Tick Thread 통합 — 입력 처리 파이프라인

기존 TickLoop.step()을 확장하여 네트워크 입력을 처리한다.

```
기존 step() 흐름:
  1. WASM Plugins 실행 → WasmCommand 수집
  2. WasmCommand → EngineCommand 변환 → CommandStream push
  3. CommandStream resolve (LWW)
  4. Apply commands
  5. Event drain

확장된 step() 흐름:
  0. [신규] input_rx drain → SessionInput 수집
  1. [신규] SessionInput 처리 (Connected/Disconnected/Line)
     - Line → parse_command() → GameCommand
     - GameCommand → EngineCommand 변환 → CommandStream push
  2. WASM Plugins 실행 → WasmCommand 수집 → CommandStream push
  3. CommandStream resolve (LWW)
  4. Apply commands
  5. [신규] 출력 생성 (Room description, 전투 결과 등)
  6. [신규] output_tx.send(SessionOutput) → 클라이언트에 텍스트 전송
  7. Event drain
```

TickLoop에 추가되는 필드:
```rust
pub struct TickLoop {
    // 기존 필드...
    pub input_rx: Option<UnboundedReceiver<SessionInput>>,
    pub output_tx: Option<UnboundedSender<SessionOutput>>,
    pub session_manager: SessionManager,   // Tick thread 내에서 세션 상태 관리
}
```

핵심: SessionManager는 Tick thread 내에 존재한다 (async가 아님).
- SessionInput::Connected → session_manager에 새 세션 등록
- SessionInput::Line → session_manager에서 auth 상태 확인 → GameCommand 변환
- SessionInput::Disconnected → player entity 정리

- **검증:** mock input channel로 TickLoop 통합 테스트

### Step 9: 인증 흐름 (최소 구현)

Phase 2에서는 최소한의 인증만 구현한다 (DB 없이 메모리 기반).

흐름:
```
[연결] → AwaitingName → "Enter your name: " 전송
[이름 입력] → AwaitingPassword → "Enter password: " 전송
[비밀번호] → Authenticated
  ├─ 기존 캐릭터 → Snapshot에서 복원 (EntityId 매칭)
  └─ 신규 캐릭터 → spawn_entity + 기본 Component 부착 + 시작 Room 배치
```

Phase 2 범위:
- 이름만으로 식별 (비밀번호는 stub: 아무 값이든 통과)
- 이름이 Snapshot에 있으면 기존 캐릭터 복원
- 없으면 신규 생성

Phase 2 범위 밖:
- 해시된 비밀번호 저장 → Phase 3
- 중복 로그인 방지 → Phase 3
- 계정 시스템 → Phase 3

- **검증:** 연결 → 이름 입력 → 인증 → 게임 진입 통합 테스트

### Step 10: Look 시스템 + Room 출력

인증 완료 시 그리고 `look` 명령 시 Room 정보를 출력한다.

```
=== 시장 광장 ===
활기찬 시장 광장이다. 상인들이 물건을 팔고 있다.

[출구: 북쪽, 남쪽, 동쪽]

여기 있는 사람:
  고블린 (NPC)
  용사1 (Player)
```

구현:
- RoomDescription Component 조회 → Room 이름 + 설명
- RoomExits 조회 → 출구 목록
- `space.entities_in_same_area()` → 같은 방 Entity 목록
- Entity별 Name Component 조회 → 이름 표시
- NpcTag/PlayerTag로 구분

- **검증:** Look 출력 포맷 단위 테스트

### Step 11: 이동 시스템

GameCommand::Move(direction) 처리:

1. 해당 session의 player entity 조회
2. player가 있는 Room의 RoomExits 조회
3. direction에 해당하는 출구가 있는지 확인
4. 있으면: `space.move_entity(player, target_room)`
5. 이전 Room 점유자에게: "용사1이(가) 북쪽으로 떠났다." 전송
6. 새 Room 점유자에게: "용사1이(가) 도착했다." 전송
7. player에게: 새 Room의 look 출력

없으면: "그 방향으로는 갈 수 없다." 전송

- **검증:** 이동 성공/실패 테스트, 이동 시 메시지 브로드캐스트 테스트

### Step 12: 전투 시스템

#### Core 측 (Rust)
- `attack <target>` → 같은 Room에서 target 이름 검색
- 매 tick (또는 2 tick마다) 전투 상태인 Entity의 공격 처리
- HP ≤ 0 → 사망 처리 (Entity 유지, Health.current = 0, 일정 시간 후 부활)

#### WASM Plugin 측 (npc_ai)
- 데미지 계산 공식을 WASM Plugin이 결정
- `WasmCommand::EmitEvent` 로 데미지 수치 전달
- Core가 HP 변경 적용 (SetComponent)

#### 최소 구현 (Phase 2)
- 정수 기반 데미지: `damage = attacker.attack.damage_min + (seed % (max - min + 1))`
- 방어력 적용: `final_damage = max(1, damage - defender.defense.armor)`
- HP 0 이하 → 사망 메시지 + 간단한 전리품(경험치 없이 item drop)
- 사망 후 10 tick 뒤 시작 Room에서 부활

결정론 보장:
- 데미지 RNG는 `deterministic_seed(tick, "combat")` 기반
- 동일 tick + 동일 상태 → 동일 데미지

- **검증:** 전투 → 데미지 → 사망 → 부활 통합 테스트, 결정론 검증

### Step 13: 인벤토리 시스템

```
get <item>   - 같은 Room 바닥의 아이템 획득
drop <item>  - 인벤토리의 아이템을 Room 바닥에 놓기
inventory    - 인벤토리 목록 표시
use <item>   - 소비 아이템 사용 (회복 등)
```

구현:
- Room 바닥 아이템: Room에 있는 ItemData Entity 중 Inventory에 속하지 않는 것
- 획득: ItemData Entity의 EntityId를 player의 Inventory.items에 추가
- 드롭: Inventory에서 제거 + Room에 배치
- 사용: Consumable → HP 회복 + Entity 제거

- **검증:** get/drop/use 사이클 테스트

### Step 14: NPC AI WASM 플러그인 (plugins/npc_ai/)

기존 test_movement 플러그인을 확장한 실전 NPC AI 플러그인.

동작:
- `on_tick(tick)`:
  - `host_get_tick()`으로 현재 tick 조회
  - tick 기반 RNG로 행동 결정
  - 50% 확률로 랜덤 방향 이동: `WasmCommand::MoveEntity`
  - 같은 방에 Player가 있으면: `WasmCommand::EmitEvent` (attack intent)
  - 그 외: 대기

Phase 2 범위:
- 단순 랜덤 이동
- 공격적 NPC: 같은 방 Player에게 공격 시도
- 수동적 NPC: 이동만

- **검증:** NPC 이동 + 공격 통합 테스트

### Step 15: Snapshot Persistence (persistence/)

#### WorldSnapshot 구조

```rust
#[derive(Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub schema_version: u32,       // 현재 1
    pub tick: u64,
    pub timestamp_secs: u64,       // SystemTime → epoch seconds
    pub entities: Vec<EntitySnapshot>,
    pub rooms: Vec<RoomSnapshot>,
    pub room_graph: RoomGraphSnapshot,
    pub entity_allocator: EntityAllocatorSnapshot,
}

#[derive(Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u64,            // EntityId.to_u64()
    pub components: Vec<ComponentSnapshot>,
}

#[derive(Serialize, Deserialize)]
pub struct ComponentSnapshot {
    pub component_id: u32,
    pub data: Vec<u8>,             // bincode-serialized component
}

#[derive(Serialize, Deserialize)]
pub struct RoomSnapshot {
    pub room_id: u64,
    pub name: String,
    pub description: String,
    pub exits: RoomExitsSnapshot,
    pub occupants: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct RoomGraphSnapshot {
    pub rooms: Vec<RoomSnapshot>,
}

#[derive(Serialize, Deserialize)]
pub struct EntityAllocatorSnapshot {
    pub generations: Vec<u32>,
    pub free_list: Vec<u32>,
    pub next_index: u32,
}
```

#### SnapshotManager

```rust
pub struct SnapshotManager {
    save_interval: Duration,       // 기본 60초
    max_snapshots: usize,          // 기본 5개 유지
    save_path: PathBuf,
    last_save: Instant,
}

impl SnapshotManager {
    pub fn new(save_path: PathBuf, interval_secs: u64, max_snapshots: usize) -> Self;
    pub fn maybe_save(&mut self, snapshot: &WorldSnapshot) -> Result<(), PersistenceError>;
    pub fn load_latest(&self) -> Result<Option<WorldSnapshot>, PersistenceError>;
    fn rotate_old_snapshots(&self) -> Result<(), PersistenceError>;
}
```

#### 저장/복원 흐름

저장 (Tick thread에서):
1. 매 tick 종료 시 `snapshot_manager.maybe_save()` 호출
2. save_interval 경과 시 → ECS 상태 + Space 상태 + Allocator 상태를 WorldSnapshot으로 변환
3. bincode 직렬화 → 파일 기록 (`snapshot_{tick}.bin`)
4. 오래된 Snapshot 삭제 (최근 N개만 유지)

복원 (서버 시작 시):
1. `snapshot_manager.load_latest()` 호출
2. WorldSnapshot 역직렬화
3. EntityAllocator 상태 복원
4. Entity + Component 복원 (ECS에 재삽입)
5. RoomGraphSpace 복원
6. 복원된 tick 번호부터 시뮬레이션 재개

- **검증:** save → load round-trip, schema_version 체크, 로테이션 테스트

### Step 16: 서버 바이너리 + 초기 월드 + 통합 테스트

#### main.rs (서버 진입점)

```rust
#[tokio::main]
async fn main() {
    observability::init_logging();

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();

    // Tick thread (동기)
    let tick_handle = std::thread::spawn(move || {
        let mut tick_loop = create_tick_loop(input_rx, output_tx);
        tick_loop.run();
    });

    // Network server (비동기)
    let addr = "0.0.0.0:4000".parse().unwrap();
    net::run_server(addr, input_tx, output_rx).await.unwrap();
}
```

#### 초기 월드 (world_builder.rs)

테스트용 최소 맵:
```
[시작의 방] ←→ [시장 광장] ←→ [어두운 골목]
                  ↕
              [무기 상점]
                  ↕
              [던전 입구] ←→ [던전 1층]
```

- 6개 Room, 양방향 연결
- 시작의 방에 환영 메시지
- 던전 1층에 고블린 NPC 배치
- 시장 광장에 아이템 배치

#### 통합 테스트

```
tests/
├── mud_session_test.rs      ← 세션 연결/인증/이동 통합 테스트
├── combat_test.rs           ← 전투 시스템 통합 테스트
├── persistence_test.rs      ← Snapshot save/load round-trip
├── load_test.rs             ← 동접 100 부하 테스트
└── (기존 Phase 0/1 테스트 유지)
```

- **검증:** 전체 플레이 흐름 (접속 → 인증 → 이동 → 전투 → 아이템 → 종료 → 재접속 복원)

---

## 의존성 순서

```
Step 1 (workspace 확장)
  │
  ├─ Step 2 (게임 Component) ─────────────┐
  ├─ Step 3 (명령 파서) ──────────────────┤
  ├─ Step 4 (채널 타입) ───┐              │
  │                         │              │
  │  Step 5 (LineBuffer) ──┤ 병렬 가능     │
  │  Step 6 (Telnet) ──────┘              │
  │                                        │
  ├─ Step 7 (네트워크 서버 + Session) ←── Step 4, 5, 6
  │                                        │
  ├─ Step 8 (Tick 통합 — 입력 처리) ←──── Step 2, 3, 4, 7
  │                                        │
  ├─ Step 9 (인증 흐름) ←──────────────── Step 8
  │                                        │
  ├─ Step 10 (Look 시스템) ←───────────── Step 9
  │                                        │
  ├─ Step 11 (이동 시스템) ←───────────── Step 10
  │                                        │
  ├─ Step 12 (전투 시스템) ←───────────── Step 11   ─┐
  │                                                    │ 병렬 가능
  ├─ Step 13 (인벤토리) ←──────────────── Step 11   ─┘
  │
  ├─ Step 14 (NPC AI Plugin) ←─────────── Step 12
  │
  ├─ Step 15 (Snapshot Persistence) ←──── Step 2
  │
  └─ Step 16 (서버 바이너리 + 통합 테스트) ←── 전체
```

---

## Phase 2 완료 조건

| 조건 | 검증 방법 |
|------|-----------|
| Telnet 클라이언트로 접속 가능 | 수동 telnet 접속 + 통합 테스트 |
| 캐릭터 이름 입력 → 게임 진입 | session_test 통합 테스트 |
| Room 이동 (n/s/e/w) + 방 묘사 출력 | mud_session_test |
| 같은 방 플레이어에게 메시지 전파 | 이동/전투 시 broadcast 테스트 |
| 전투 (attack → 데미지 → 사망 → 부활) | combat_test |
| 인벤토리 (get/drop/use) | inventory 단위 테스트 |
| NPC AI가 WASM Plugin으로 동작 | NPC 이동/공격 통합 테스트 |
| 서버 종료 → 재시작 후 캐릭터 복원 | persistence_test |
| Snapshot 로테이션 (최근 N개 유지) | persistence_test |
| 동접 100명 테스트 성공 | load_test (mock client) |
| Phase 0/1 기존 42개 테스트 전부 통과 | cargo test --workspace |
| say 명령으로 같은 방 채팅 | 채팅 테스트 |

---

## 리스크 및 완화 전략

| 리스크 | 완화 |
|--------|------|
| Telnet IAC 엣지 케이스 | 최소 IAC 핸들링만 구현, 고급 기능은 Phase 3 |
| Snapshot 파일 손상 | 복수 Snapshot 유지 + schema_version 검증 |
| Snapshot 호환성 깨짐 | schema_version 필드로 버전 분기, Migration은 Phase 3 |
| Tick thread blocking | Unbounded channel + input drain 방식, await 금지 |
| 동접 100명 시 성능 | LineBuffer max_size 방어, 부하 테스트 스크립트 |
| 전투 결정론 깨짐 | 정수 기반 데미지 + deterministic_seed |
| ECS Component 직렬화 실패 | Snapshot에 schema_version, 테스트로 round-trip 검증 |
| 세션 누수 (Disconnect 미처리) | SessionManager에서 주기적 정리 + timeout |

---

## 검증 방법

```bash
source "$HOME/.cargo/env"

# 전체 빌드
cargo build --workspace

# 전체 테스트
cargo test --workspace

# 개별 crate 테스트
cargo test -p net
cargo test -p persistence
cargo test -p mud

# 통합 테스트
cargo test --test mud_session_test -- --nocapture
cargo test --test combat_test -- --nocapture
cargo test --test persistence_test -- --nocapture

# NPC AI 플러그인 빌드
cd plugins/npc_ai && cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/npc_ai.wasm ../../test_fixtures/

# 서버 실행
cargo run

# 수동 접속 테스트
telnet localhost 4000
```
