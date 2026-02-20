# Rust 기반 MUD/2D 겸용 엔진 구현 계획서 (v3)

---

# 변경 이력

| 버전 | 날짜 | 주요 변경 |
|------|------|-----------|
| v1 | 2026-02-19 | 초안 작성 |
| v2 | 2026-02-19 | 피드백 반영 요약본 |
| v3 | 2026-02-19 | v1 상세 내용 유지 + v2 수정사항 + 구조적 결함 보완 통합본 |
| v3.1 | 2026-02-19 | EntityId generation 정책, Snapshot versioning, Determinism Hash 정렬 기준 확정 |

### v3.1 변경 사항

- EntityId를 `(generation: u32, index: u32)` 구조로 변경 (Phase 0)
- WorldSnapshot에 `schema_version` 및 `EntityAllocatorSnapshot` 추가 (Phase 2)
- Determinism Hash 정렬 기준 명시 (Phase 3)
- Component Registry 개념 추가 (Phase 1)
- Backpressure 전략 Phase 3 리스크 테이블에 추가

### v3 주요 변경 사항

- Phase 번호와 섹션 번호 체계 통일
- TPS 30 기본, configurable
- ECS 모듈 격리 전략 반영
- Command Stream 상세 설계 포함
- Persistence를 Phase 2에 포함
- WASM Memory 안정성 검증 항목 강화
- Fuel을 결정론적 파라미터로 재정의
- MUD 공간 모델(RoomGraphSpace) 명시
- 일정 현실화 (총 10~12개월)
- 리스크 검증 일정표 완성
- 번호 중복/누락 수정

---

# 1. 구현 전략 개요

## 1.1 핵심 원칙

1. **Deterministic Core를 먼저 완성한다.** Tick Loop + ECS + Command Stream이 안정적으로 동작해야 다음 단계로 간다.
2. **WASM은 최소 기능부터 통합한다.** 첫 Plugin은 "이동 방향 결정"처럼 단순한 것.
3. **"플레이 가능한 상태"를 가능한 빨리 만든다.** Phase 2 완료 시점에서 실제 Telnet 접속 + 이동 + 전투가 가능해야 한다.
4. **최적화는 구조가 안정된 이후에 진행한다.** Phase 0~2에서 premature optimization 금지.
5. **DB, 분산, 샤딩은 가장 마지막 단계에서 도입한다.** 단, Snapshot 기반 최소 Persistence는 Phase 2에 포함.

## 1.2 모든 Phase 공통 규칙

- 각 Phase 시작 전 이전 Phase의 완료 조건을 100% 충족해야 한다
- 완료 조건은 자동화된 테스트로 검증 가능해야 한다
- Phase 완료 시 아키텍처 문서와의 정합성 리뷰 수행

---

# 2. PHASE 0 — 엔진 코어 골격 구축

**예상 기간: 4~6주**

## 2.1 목표

플러그인 없이도 독립적으로 동작하는 Tick 기반 서버 코어 완성.
이 단계의 산출물은 "WASM 없이 동작하는 결정론적 시뮬레이션 루프"다.

## 2.2 프로젝트 구조

```
rust_mud_engine/
├── Cargo.toml
├── crates/
│   ├── engine_core/         ← Tick Loop, Event Bus, Command Stream
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tick.rs      ← Tick 루프 메인
│   │       ├── events.rs    ← Event Bus
│   │       └── command.rs   ← EngineCommand, CommandStream
│   │
│   ├── ecs_adapter/         ← ECS 백엔드 격리 계층
│   │   └── src/
│   │       ├── mod.rs       ← 공개 API
│   │       ├── bevy_backend.rs
│   │       └── types.rs     ← EntityId, ComponentId 등
│   │
│   ├── space/               ← SpaceModel trait + 구현체
│   │   └── src/
│   │       ├── mod.rs       ← SpaceModel trait
│   │       ├── room_graph.rs ← MUD RoomGraphSpace
│   │       └── grid.rs      ← 2D GridSpace (Phase 4)
│   │
│   ├── plugin_runtime/      ← WASM Runtime (Phase 1)
│   │   └── src/
│   │
│   ├── net/                 ← 네트워크 계층 (Phase 2)
│   │   └── src/
│   │       ├── transport.rs
│   │       ├── telnet.rs
│   │       ├── codec.rs
│   │       └── session.rs
│   │
│   ├── persistence/         ← Snapshot/DB (Phase 2)
│   │   └── src/
│   │
│   └── observability/       ← Logging, Metrics
│       └── src/
│
├── plugins/                 ← WASM Plugin 소스 (Phase 1)
│   └── combat/
│
└── tests/
    ├── tick_determinism.rs
    ├── ecs_adapter_test.rs
    └── command_stream_test.rs
```

> **설계 근거:** Cargo workspace로 crate를 분리하여 각 모듈의 의존성을 명시적으로 관리한다.
> ecs_adapter가 bevy_ecs에 의존하더라도, engine_core는 ecs_adapter의 공개 API에만 의존하므로
> bevy_ecs 교체 시 engine_core를 수정할 필요가 없다.

## 2.3 ECS 모듈 격리 구현

```rust
// crates/ecs_adapter/src/types.rs

/// 엔진 전용 Entity ID.
/// 단순 u64 단조 증가가 아닌 generation+index 방식.
/// Snapshot restore 후 ID 충돌, Entity 삭제 후 dangling reference를 방지한다.
///
/// index: Entity 슬롯 번호 (재사용 가능)
/// generation: 해당 슬롯의 재사용 횟수 (재사용 시 증가)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId {
    pub index: u32,
    pub generation: u32,
}

impl EntityId {
    /// WASM ABI용 u64 변환. 상위 32bit = generation, 하위 32bit = index.
    pub fn to_u64(&self) -> u64 {
        ((self.generation as u64) << 32) | (self.index as u64)
    }
    pub fn from_u64(val: u64) -> Self {
        Self { index: val as u32, generation: (val >> 32) as u32 }
    }
}

/// Component 종류 식별자.
/// WASM ABI에서도 이 ID로 Component를 참조한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32);
```

### EntityAllocator (Phase 0 필수)

```rust
/// Entity 슬롯 관리자.
/// 삭제된 Entity의 index를 재사용하되 generation을 증가시켜 구분한다.
/// Snapshot 저장/복원 시 이 구조체의 상태도 함께 직렬화한다.
struct EntityAllocator {
    generations: Vec<u32>,
    free_list: Vec<u32>,
    next_index: u32,
}

impl EntityAllocator {
    fn allocate(&mut self) -> EntityId {
        if let Some(index) = self.free_list.pop() {
            self.generations[index as usize] += 1;
            EntityId { index, generation: self.generations[index as usize] }
        } else {
            let index = self.next_index;
            self.next_index += 1;
            self.generations.push(0);
            EntityId { index, generation: 0 }
        }
    }

    fn deallocate(&mut self, id: EntityId) -> bool {
        if self.generations.get(id.index as usize) != Some(&id.generation) {
            return false;
        }
        self.free_list.push(id.index);
        true
    }
}
```

```rust
// crates/ecs_adapter/src/mod.rs (공개 API 예시)

pub fn spawn_entity() -> EntityId;
pub fn despawn_entity(id: EntityId);
pub fn get_component<T: Component>(id: EntityId) -> Option<&T>;
pub fn set_component<T: Component>(id: EntityId, component: T);
pub fn entities_with<T: Component>() -> impl Iterator<Item = EntityId>;
```

## 2.4 Command Stream 구조 구현

```rust
// crates/engine_core/src/command.rs

/// WASM Plugin이 ECS를 직접 수정하지 않도록,
/// 모든 상태 변경은 Command로 표현한다.
/// Core가 tick의 commit 단계에서 일괄 처리한다.
#[derive(Debug)]
pub enum EngineCommand {
    SetComponent {
        entity: EntityId,
        component_id: ComponentId,
        data: Vec<u8>,
    },
    RemoveComponent {
        entity: EntityId,
        component_id: ComponentId,
    },
    EmitEvent {
        event_id: EventId,
        payload: Vec<u8>,
    },
    SpawnEntity {
        template_id: u32,
    },
    DestroyEntity {
        entity: EntityId,
    },
    MoveEntity {
        entity: EntityId,
        destination: AreaId,
    },
}

/// 한 tick에서 수집된 모든 Command를 저장.
/// Plugin 실행 순서대로 Command가 추가된다.
pub struct CommandStream {
    commands: Vec<(PluginId, EngineCommand)>,
}

impl CommandStream {
    pub fn push(&mut self, plugin: PluginId, cmd: EngineCommand) {
        self.commands.push((plugin, cmd));
    }

    /// Last Writer Wins 정책으로 충돌 해결 후 최종 Command 목록 반환.
    pub fn resolve(&self) -> Vec<EngineCommand> {
        // 같은 Entity+Component에 대한 여러 SetComponent 중
        // 마지막 것만 유지
        // ...
    }
}
```

## 2.5 Tick Loop 기본 구현

```rust
// crates/engine_core/src/tick.rs

/// 메인 Tick 루프.
/// 이 단계에서는 네트워크/WASM 없이 순수 시뮬레이션만 돌린다.
pub fn run_tick_loop(config: TickConfig) {
    let tick_duration = Duration::from_secs(1) / config.tps; // 기본 30 TPS
    let mut current_tick: u64 = 0;

    loop {
        let tick_start = Instant::now();

        // Phase 0에서는 fake input으로 대체
        let commands = generate_test_commands(current_tick);

        // Core systems 실행 (이동 등)
        run_core_systems(current_tick);

        // Command 처리 → 상태 확정
        process_command_stream(&commands);

        // 메트릭 기록
        let elapsed = tick_start.elapsed();
        record_tick_duration(elapsed);

        if elapsed > tick_duration {
            warn!("Tick {} overran budget: {:?}", current_tick, elapsed);
        }

        current_tick += 1;

        // 다음 tick까지 sleep
        let remaining = tick_duration.saturating_sub(tick_start.elapsed());
        std::thread::sleep(remaining);
    }
}
```

## 2.6 Observability 초기 설정

Phase 0부터 반드시 포함:

```rust
// tracing 초기화
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .init();

// Tick 메트릭
#[instrument(skip_all, fields(tick = current_tick))]
fn run_tick(current_tick: u64) {
    let _span = info_span!("tick", number = current_tick).entered();
    // ...
}
```

## 2.7 완료 조건

- [ ] CLI 기반 가짜 플레이어 100개 생성 가능
- [ ] 위치 이동 시뮬레이션이 30 TPS로 안정적 동작
- [ ] Command Stream이 올바르게 수집 → resolve → 적용
- [ ] 동일 입력 → 동일 결과 (결정론) 테스트 통과
- [ ] ecs_adapter 공개 API에서 bevy_ecs 타입 미노출 확인
- [ ] tick duration 메트릭이 로그에 기록
- [ ] EntityId generation 정책 동작: allocate → deallocate → re-allocate 시 generation 증가 확인
- [ ] EntityAllocator 직렬화/역직렬화 테스트 통과

---

# 3. PHASE 1 — WASM Runtime 통합

**예상 기간: 6~8주**

## 3.1 목표

WASM 플러그인이 실제로 Command Stream을 생성하고,
Core가 이를 안전하게 처리하는 전체 파이프라인 완성.

## 3.2 Wasmtime 통합

```rust
// crates/plugin_runtime/src/lib.rs

/// WASM Plugin을 로드하고 실행하는 런타임.
/// tick thread에서 동기적으로 호출된다.
pub struct PluginRuntime {
    engine: wasmtime::Engine,
    plugins: Vec<LoadedPlugin>,
}

struct LoadedPlugin {
    id: PluginId,
    priority: u32,              // 실행 순서 결정
    instance: wasmtime::Instance,
    fuel_limit: u64,            // 결정론적 Fuel 한도
    consecutive_failures: u32,  // quarantine 판단용
}
```

### Fuel 설정

```rust
/// Fuel 한도는 엔진 설정의 일부이며, 런타임에 변경 불가.
/// Replay 시 동일한 Fuel 한도를 적용하여 결정론을 보장한다.
let mut config = wasmtime::Config::new();
config.consume_fuel(true);

// 매 tick, 매 plugin 호출 전에 fuel 충전
store.set_fuel(plugin.fuel_limit)?;
```

## 3.3 Memory Boundary 설계

### 금지 사항

- WASM Linear Memory를 Rust의 `&[u8]`로 장기 보관
- Tick을 넘어가는 slice 참조 유지
- `memory.grow()` 전후로 동일 포인터 사용

### 필수 구현: WasmMemoryView

```rust
/// WASM Linear Memory에 대한 안전한 접근 래퍼.
/// 모든 read/write는 이 래퍼를 통해 수행하며,
/// 매 접근 시 base pointer를 재획득하여 grow() 이후에도 안전하다.
///
/// store를 &mut로 보유하므로 한 번에 하나의 뷰만 존재 가능.
/// 이는 Rust의 독점 빌림 규칙에 의한 의도적 안전 장치다.
/// 다수 Component를 연속 읽어야 하는 경우에도 순차 read → drop → read로 처리하며,
/// 이것이 병목인지는 Phase 1 프로파일링에서 확인한다.
/// 필요 시 읽기 전용 WasmMemoryReadView(&Store, 불변 참조)를 별도로 만들 수 있다.
struct WasmMemoryView<'a> {
    memory: &'a wasmtime::Memory,
    store: &'a mut wasmtime::Store<HostState>,
}

impl<'a> WasmMemoryView<'a> {
    fn read_bytes(&self, offset: u32, len: u32) -> Result<Vec<u8>, MemoryError> {
        let data = self.memory.data(&self.store);
        let start = offset as usize;
        let end = start + len as usize;
        if end > data.len() {
            return Err(MemoryError::OutOfBounds);
        }
        Ok(data[start..end].to_vec())
    }

    fn write_bytes(&mut self, offset: u32, bytes: &[u8]) -> Result<(), MemoryError> {
        let data = self.memory.data_mut(&mut self.store);
        let start = offset as usize;
        let end = start + bytes.len();
        if end > data.len() {
            return Err(MemoryError::OutOfBounds);
        }
        data[start..end].copy_from_slice(bytes);
        Ok(())
    }
}
```

### grow() 스트레스 테스트 (Phase 1 필수)

```rust
/// grow() 10,000회 반복 후에도 안전하게 동작하는지 검증.
/// Segfault 및 UB가 발생하지 않아야 한다.
#[test]
fn test_memory_grow_stress() {
    for _ in 0..10_000 {
        memory.grow(&mut store, 1)?;      // 1 page(64KB) 확장
        let view = WasmMemoryView::new(&memory, &store);
        view.write_bytes(0, &test_data)?; // grow 후에도 안전한지 확인
        let read = view.read_bytes(0, test_data.len() as u32)?;
        assert_eq!(read, test_data);
    }
}
```

## 3.4 Zero-Copy Command 작성 구조

```
Plugin on_tick() 호출
    │
    ├─ Plugin이 Linear Memory의 Command Buffer에 명령 기록
    │   (postcard 직렬화 사용)
    │
    ├─ Plugin이 (buffer_offset, buffer_length) 반환
    │
    └─ Host가 WasmMemoryView로 해당 영역 읽기
        │
        └─ postcard::from_bytes()로 역직렬화
            │
            └─ Vec<EngineCommand>로 변환 → CommandStream에 추가
```

> **설계 근거:** Plugin이 host_emit_command()를 매 command마다 호출하면
> FFI round-trip이 command 수만큼 발생한다.
> 대신 Plugin이 Linear Memory에 일괄 기록하고 offset만 반환하면
> FFI 호출은 1회로 줄어든다.

## 3.5 Panic 격리

```rust
/// Plugin 실행을 catch_unwind로 감싸서 panic이 전파되지 않도록 한다.
/// wasmtime의 trap도 동일하게 처리한다.
fn execute_plugin(plugin: &mut LoadedPlugin, tick: u64) -> PluginResult {
    match plugin.call_on_tick(tick) {
        Ok(commands) => PluginResult::Success(commands),
        Err(trap) if trap.is_fuel_exhausted() => {
            plugin.consecutive_failures += 1;
            PluginResult::FuelExceeded
        }
        Err(trap) => {
            plugin.consecutive_failures += 1;
            warn!("Plugin {} panicked at tick {}: {}", plugin.id, tick, trap);
            PluginResult::Panic(trap.to_string())
        }
    }
}

/// 3회 연속 실패 시 quarantine
fn check_quarantine(plugin: &mut LoadedPlugin) -> bool {
    if plugin.consecutive_failures >= 3 {
        error!("Plugin {} quarantined after 3 consecutive failures", plugin.id);
        plugin.quarantined = true;
        true
    } else {
        false
    }
}
```

## 3.6 직렬화 전략

Phase 1~2 에서는 postcard 사용.

```rust
/// 직렬화 추상화 계층.
/// Phase 3 이후 FlatBuffers 등으로 교체할 때 이 trait의 구현체만 변경한다.
trait WasmSerializer {
    fn serialize<T: serde::Serialize>(&self, value: &T) -> Result<Vec<u8>, Error>;
    fn deserialize<T: serde::de::DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Error>;
}

struct PostcardSerializer;
impl WasmSerializer for PostcardSerializer {
    fn serialize<T: serde::Serialize>(&self, value: &T) -> Result<Vec<u8>, Error> {
        postcard::to_allocvec(value).map_err(Into::into)
    }
    fn deserialize<T: serde::de::DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Error> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }
}
```

## 3.7 완료 조건

- [ ] WASM Plugin에서 이동 로직이 결정됨 (방향 결정 → MoveEntity Command 생성)
- [ ] Fuel 초과 시 Plugin 안전 종료, 해당 tick Command 폐기
- [ ] 무한 루프 Plugin이 엔진을 멈추지 않음 (Fuel에 의해 30ms 이내 중단)
- [ ] grow() 10,000회 스트레스 테스트 통과
- [ ] Segfault 및 UB 미발생 (MIRI 또는 Address Sanitizer)
- [ ] 3회 연속 Panic → quarantine 동작 확인
- [ ] 동일 입력 + 동일 Fuel → 동일 결과 (결정론) 테스트
- [ ] Component Registry 동작: ComponentId로 직렬화/역직렬화 round-trip 성공

---

# 4. PHASE 2 — Playable MUD + Persistence

**예상 기간: 10~12주**

## 4.1 목표

텍스트 기반 MUD 1개 완성. Telnet으로 접속하여 이동, 전투, 아이템 획득이 가능하고,
서버 재시작 후 캐릭터 상태가 복구된다.

## 4.2 네트워크 계층 구현

### Thread 구조 연결

```
[Tokio Runtime]
  ├─ accept_loop()
  │    └─ 새 연결 → spawn(handle_client)
  │
  ├─ handle_client(socket)
  │    ├─ Telnet IAC negotiation
  │    ├─ read loop → input_tx.send(SessionInput)
  │    └─ output_rx → socket.write()
  │
  └─ ...

[Tick Thread]
  └─ input_rx.try_recv() → drain all pending inputs
```

### Session Layer 구현

```rust
/// 세션별 상태.
/// 네트워크 계층(async)과 tick thread 사이의 경계 객체.
struct Session {
    id: SessionId,
    player_entity: Option<EntityId>,
    auth_state: AuthState,
    protocol: ProtocolType,
    line_buffer: LineBuffer,  // MUD 전용: 분할 패킷 누적
}

/// MUD 텍스트 입력의 분할 패킷 처리.
/// 개행 문자를 감지할 때까지 바이트를 누적한다.
struct LineBuffer {
    buffer: Vec<u8>,
    max_size: usize,  // DoS 방지: 기본 4096
}

impl LineBuffer {
    /// raw bytes를 누적하고, 완성된 줄이 있으면 반환.
    fn feed(&mut self, data: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(data);
        let mut lines = Vec::new();
        while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line).trim().to_string();
            if !text.is_empty() {
                lines.push(text);
            }
        }
        // buffer overflow 방지
        if self.buffer.len() > self.max_size {
            self.buffer.clear();
        }
        lines
    }
}
```

### Telnet 프로토콜 주의사항

Telnet 구현 시 주의할 엣지 케이스:

- **IAC (Interpret As Command)**: 0xFF 바이트는 IAC 시퀀스 시작. 게임 데이터와 구분 필요.
- **IAC 이스케이프**: RFC 854에 따라 데이터 내 0xFF는 0xFF 0xFF로 이스케이프. UTF-8 환경에서 0xFF는 등장하지 않지만, IAC 파서는 이 처리를 반드시 포함해야 한다. EUC-KR 지원 시(Phase 5) 0x80~0xFF 바이트와의 충돌을 재검증.
- **NAWS (Negotiate About Window Size)**: 클라이언트 터미널 크기 협상. 필수는 아니나 유용.
- **MCCP (Mud Client Compression Protocol)**: zlib 압축. Phase 3에서 선택적 도입.
- **Line Mode vs Character Mode**: 기본은 Line Mode (클라이언트가 줄 단위 전송).
- **Encoding**: UTF-8 기본, EUC-KR 등 레거시 인코딩은 Phase 5에서 고려.

> **Phase 2 최소 구현:** IAC WILL/WONT/DO/DONT 기본 핸들링 + IAC 바이트 이스케이프 + Line Mode.
> 고급 Telnet 기능(MCCP, MSDP, GMCP)은 Phase 3 이후.

## 4.3 RoomGraphSpace 구현

```rust
/// MUD 전용 Room 그래프 공간 모델.
/// Room은 ECS Entity이며, Exit 정보를 Component로 가진다.
pub struct RoomGraphSpace {
    room_occupants: HashMap<EntityId, HashSet<EntityId>>,
    entity_room: HashMap<EntityId, EntityId>,
}

impl SpaceModel for RoomGraphSpace {
    fn entities_in_same_area(&self, entity: EntityId) -> Vec<EntityId> {
        let room = self.entity_room.get(&entity);
        room.and_then(|r| self.room_occupants.get(r))
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    fn neighbors(&self, entity: EntityId) -> Vec<EntityId> {
        // MUD에서 neighbors = 인접 Room의 점유자들
        // look 범위, 소리 전파 등에 사용
        // ...
    }

    fn move_entity(&mut self, entity: EntityId, destination: AreaId)
        -> Result<(AreaId, AreaId), MoveError>
    {
        let old_room = self.entity_room.get(&entity)
            .copied()
            .ok_or(MoveError::EntityNotFound)?;
        // Exit 유효성 검사
        // ...
        self.room_occupants.get_mut(&old_room).map(|s| s.remove(&entity));
        self.room_occupants.entry(destination).or_default().insert(entity);
        self.entity_room.insert(entity, destination);
        Ok((old_room, destination))
    }

    fn broadcast_targets(&self, entity: EntityId) -> Vec<EntityId> {
        // MUD: 같은 Room에 있는 플레이어만
        self.entities_in_same_area(entity)
    }
}
```

## 4.4 기본 게임 시스템

### 이동 시스템
- 텍스트 명령 파서: `north`, `south`, `east`, `west`, `look`, `go <direction>`
- RoomGraphSpace.move_entity() 호출
- 이동 결과 이벤트 발행 (AreaLeave, AreaEnter)
- Room description 전송

### 전투 시스템
- 데미지 계산: **정수 기반** (결정론 보장)
- WASM Plugin이 데미지 공식 결정
- Core가 HP 변경 적용
- 사망 처리, 전리품 드랍

### 인벤토리
- 아이템 획득/사용/버리기
- ECS Component로 표현

### 간단한 AI
- NPC 이동 (WASM Plugin에서 방향 결정)
- 기본 공격 AI (타겟 선택 → 공격 Command)

## 4.5 텍스트 명령 파서

```rust
/// MUD 텍스트 명령 파서.
/// 입력 문자열을 구조화된 GameCommand로 변환한다.
pub fn parse_command(input: &str) -> Option<GameCommand> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    match parts.first()?.to_lowercase().as_str() {
        "north" | "n" => Some(GameCommand::Move(Direction::North)),
        "south" | "s" => Some(GameCommand::Move(Direction::South)),
        "east" | "e" => Some(GameCommand::Move(Direction::East)),
        "west" | "w" => Some(GameCommand::Move(Direction::West)),
        "look" | "l" => Some(GameCommand::Look),
        "say" => Some(GameCommand::Say(parts[1..].join(" "))),
        "attack" | "kill" => Some(GameCommand::Attack(parts.get(1)?.to_string())),
        "inventory" | "i" => Some(GameCommand::Inventory),
        "get" | "take" => Some(GameCommand::Get(parts.get(1)?.to_string())),
        "drop" => Some(GameCommand::Drop(parts.get(1)?.to_string())),
        "quit" => Some(GameCommand::Quit),
        _ => None,
    }
}
```

> **Phase 2 범위:** 기본 명령만 구현.
> 확장 명령(emote, whisper, channel, craft 등)은 이후 Phase에서 Plugin으로 추가.

## 4.6 Snapshot Persistence

```rust
/// 전체 월드 상태의 스냅샷.
/// bincode로 직렬화하여 파일에 저장한다.
#[derive(Serialize, Deserialize)]
struct WorldSnapshot {
    /// Snapshot 포맷 버전. 구조 변경 시 증가.
    schema_version: u32,
    tick: u64,
    timestamp: SystemTime,
    entities: Vec<EntitySnapshot>,
    rooms: Vec<RoomSnapshot>,
    room_graph: RoomGraphSnapshot,
    /// EntityAllocator 상태 복원용.
    /// 없으면 복원 후 새 Entity가 기존 ID와 충돌할 수 있다.
    entity_allocator: EntityAllocatorSnapshot,
}

#[derive(Serialize, Deserialize)]
struct EntityAllocatorSnapshot {
    generations: Vec<u32>,
    free_list: Vec<u32>,
    next_index: u32,
}

/// Snapshot 저장 정책.
/// Tier 2 (유휴 시간)에서 실행된다.
struct SnapshotManager {
    save_interval: Duration,     // 기본 60초
    max_snapshots: usize,        // 기본 5개 유지
    save_path: PathBuf,
    last_save: Instant,
}

impl SnapshotManager {
    fn maybe_save(&mut self, world: &World) {
        if self.last_save.elapsed() >= self.save_interval {
            let snapshot = world.to_snapshot();
            let bytes = bincode::serialize(&snapshot).unwrap();
            // 파일명에 tick 번호 포함
            let path = self.save_path.join(format!("snapshot_{}.bin", snapshot.tick));
            std::fs::write(&path, &bytes).unwrap();
            self.rotate_old_snapshots();
            self.last_save = Instant::now();
        }
    }

    fn load_latest(&self) -> Option<WorldSnapshot> {
        // save_path에서 가장 최근 snapshot 파일 로드
        // ...
    }
}
```

## 4.7 완료 조건

- [ ] Telnet 클라이언트로 접속 가능
- [ ] 캐릭터 생성 → 이동 → 전투 → 아이템 획득 가능
- [ ] NPC AI가 WASM Plugin으로 동작
- [ ] 서버 종료 → 재시작 후 캐릭터 상태 복구
- [ ] 동접 100명 테스트 성공 (부하 테스트 스크립트)
- [ ] Room 이동 시 같은 방 플레이어에게 메시지 전파

---

# 5. PHASE 3 — 성능 안정화 및 스케줄링 고도화

**예상 기간: 6~8주**

## 5.1 목표

부하 평탄화 및 예측 가능한 Tick Budget 확보.
동접 200~300명에서 안정적 30 TPS 유지.

## 5.2 Stratified Tick System

```rust
/// Tier별 시스템 실행.
/// Budget 초과 시 하위 Tier를 skip한다.
fn run_stratified_tick(tick: u64, budget: TickBudget) {
    let start = Instant::now();

    // Tier 0 — Essential (매 tick, 무조건 실행)
    drain_input_channel();
    process_line_buffers();
    run_movement_system();
    process_command_stream();

    if start.elapsed() > budget.tier0_limit {
        warn!("Tier 0 exceeded budget at tick {}", tick);
    }

    // Tier 1 — Heavy (N tick마다 분산)
    if start.elapsed() < budget.tier1_deadline {
        if tick % 3 == 0 { run_ai_system(); }
        if tick % 5 == 0 { run_aoi_update(); }
    }

    // Tier 2 — Background (유휴 시간 기반)
    if start.elapsed() < budget.tier2_deadline {
        flush_logs();
        collect_metrics();
        snapshot_manager.maybe_save(&world);
    }
}
```

## 5.3 Time Slicing

```rust
/// AOI 갱신을 전체 유저에 대해 N틱에 걸쳐 분산 처리.
/// 한 tick에 전체 유저의 1/N만 처리하여 CPU 스파이크를 방지한다.
fn update_aoi_sliced(tick: u64, entities: &[EntityId], slice_count: u32) {
    for entity in entities {
        if entity.0 % slice_count as u64 == tick % slice_count as u64 {
            update_aoi_for(entity);
        }
    }
}
```

## 5.4 Determinism Hash 검증

```rust
/// 결정론 검증용 월드 상태 해시.
/// ECS 내부 순서에 의존하지 않도록 명시적 정렬을 강제한다.
/// 정렬 순서: EntityId(index→generation) → ComponentId → raw bytes hash.
fn compute_world_hash(world: &World, mode: HashMode) -> u64 {
    let mut hasher = DefaultHasher::new();

    // EntityId 기준 정렬 (ECS 내부 순서에 의존하지 않음)
    let mut entities: Vec<EntityId> = world.all_entities().collect();
    entities.sort_by_key(|e| (e.index, e.generation));

    for entity in &entities {
        entity.index.hash(&mut hasher);
        entity.generation.hash(&mut hasher);

        match mode {
            HashMode::Debug => {
                // ComponentId 기준 정렬 후 raw bytes hash
                let mut components = world.all_components_raw(*entity);
                components.sort_by_key(|(comp_id, _)| comp_id.0);
                for (comp_id, raw_bytes) in &components {
                    comp_id.0.hash(&mut hasher);
                    raw_bytes.hash(&mut hasher);
                }
            }
            HashMode::Production => {
                // Critical Component만 동일 방식 적용
                for comp_id in CRITICAL_COMPONENTS {
                    if let Some(bytes) = world.get_component_raw(*entity, comp_id) {
                        comp_id.0.hash(&mut hasher);
                        bytes.hash(&mut hasher);
                    }
                }
            }
        }
    }
    hasher.finish()
}
```

- N tick마다 Hash 계산 후 로그 기록
- Replay 모드에서 동일 tick의 Hash 비교
- 불일치 발생 시 tick 번호 + 상태 diff 기록

## 5.5 FlatBuffers 선택적 도입

WASM ABI가 프로파일링에서 병목으로 확인된 경우에만:

- schema_version 필드 포함
- ABI Version Table 유지
- Forward Compatibility 원칙 (필드 삭제 금지, Optional만 추가)

> **판단 기준:** postcard 직렬화/역직렬화 시간이 tick budget의 10% 이상을 차지하면 도입 검토.

## 5.6 Snapshot 최적화

- Delta Snapshot: 이전 Snapshot 대비 변경된 Entity만 저장
- 압축: lz4 또는 zstd 적용
- 비동기 저장: Snapshot 데이터를 복제 후 별도 스레드에서 파일 기록

## 5.7 완료 조건

- [ ] 30 TPS에서 CPU spike 없음 (p99 < 25ms)
- [ ] 동접 200~300명 안정화
- [ ] Determinism Hash가 Replay에서 일치
- [ ] Tier 2 skip 발생 빈도 < 1%
- [ ] Snapshot 저장이 tick budget에 영향 없음

---

# 6. PHASE 4 — 2D MMO 확장

**예상 기간: 8~12주**

## 6.1 목표

텍스트 기반 구조를 2D 구조로 확장.
간단한 2D 클라이언트와 서버가 동기화된다.

## 6.2 GridSpace 구현

```rust
/// 2D 좌표 기반 공간 모델.
/// Broad Phase 충돌 감지, AOI 계산, 브로드캐스트 필터링을 담당.
pub struct GridSpace {
    cell_size: f32,
    grid: HashMap<(i32, i32), HashSet<EntityId>>,
    entity_positions: HashMap<EntityId, (f32, f32)>,
}

impl SpaceModel for GridSpace {
    fn entities_in_same_area(&self, entity: EntityId) -> Vec<EntityId> {
        let pos = self.entity_positions.get(&entity)?;
        let cell = (
            (pos.0 / self.cell_size) as i32,
            (pos.1 / self.cell_size) as i32,
        );
        self.grid.get(&cell).map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    fn neighbors(&self, entity: EntityId) -> Vec<EntityId> {
        // 주변 9칸(3x3) 셀의 엔티티 반환
        // AOI 범위에 따라 확장 가능
        // ...
    }

    fn move_entity(&mut self, entity: EntityId, destination: AreaId)
        -> Result<(AreaId, AreaId), MoveError>
    {
        // 좌표 기반 이동. destination은 (x, y) 좌표를 인코딩한 AreaId.
        // ...
    }

    fn broadcast_targets(&self, entity: EntityId) -> Vec<EntityId> {
        // AOI 범위 내 플레이어
        self.neighbors(entity)
    }
}
```

## 6.3 AOI (Area of Interest) 시스템

- SpaceModel.broadcast_targets()로 대상 결정
- Time Slicing으로 분산 갱신
- Enter/Leave 이벤트 → 클라이언트에 Entity 생성/소멸 통지

## 6.4 Determinism 타협 모델 적용

- 로직(데미지, 드랍): 정수 기반 (WASM)
- 이동: f32 + Snap Correction (Core)
- Snap 주기: configurable (기본 10 tick마다)

## 6.5 WebSocket 네트워크 레이어

```rust
/// WebSocket Transport 구현.
/// 동일한 Transport trait을 구현하므로 Session Layer에 변경 없음.
struct WebSocketTransport {
    ws: tokio_tungstenite::WebSocketStream<TcpStream>,
}

#[async_trait]
impl Transport for WebSocketTransport {
    // ...
}
```

### Delta Snapshot 전송

- 이전 tick 대비 변경된 Component만 전송
- 압축 적용 (lz4)
- 클라이언트 측 보간 (Interpolation) 지원을 위해 tick 번호 포함

## 6.6 완료 조건

- [ ] 간단한 2D 클라이언트(웹 기반)와 서버 동기화 성공
- [ ] 다수 Entity가 화면에서 이동하며 AOI 기반 필터링 동작
- [ ] MUD 클라이언트와 2D 클라이언트가 동시에 같은 서버에 접속 가능
- [ ] Delta Snapshot으로 대역폭 50% 이상 절감

---

# 7. PHASE 5 — 클라이언트 확장

**예상 기간: 별도 산정**

서버 구조는 그대로 유지하고 클라이언트만 추가한다.

선택지:
- 웹 클라이언트 (HTML5 Canvas / WebGL)
- Windows 네이티브 클라이언트 (Godot 등)
- 모바일 앱

서버 측 변경: 없음 (Transport + Codec 추가만)

---

# 8. 리스크 검증 일정표

| 리스크 | 검증 단계 | 대응 |
|--------|-----------|------|
| WASM 무한 루프 | Phase 1 | Fuel 강제 종료 (결정론적 한도) |
| WASM memory.grow() 포인터 무효화 | Phase 1 | WasmMemoryView 래퍼 + grow() 10K 스트레스 테스트 |
| WASM ↔ Host 직렬화 병목 | Phase 3 | postcard로 시작, 프로파일링 후 FlatBuffers 선택적 도입 |
| Plugin 간 Command 충돌 | Phase 1 | 고정 실행 순서 + Last Writer Wins + exclusive ownership |
| Tick Budget 초과 | Phase 3 | Stratified Scheduling + Time Slicing |
| 결정론 깨짐 | Phase 3 | Determinism Hash 검증 (EntityId→ComponentId→bytes 정렬) |
| Telnet 엣지 케이스 (IAC 등) | Phase 2 | 최소 IAC 핸들링, 고급 기능은 Phase 3으로 이연 |
| Snapshot 파일 손상 | Phase 2 | 복수 Snapshot 유지 + checksum 검증 + schema_version |
| Snapshot 호환성 깨짐 | Phase 2 | schema_version 필드로 버전 분기, Migration은 Phase 3 |
| bevy_ecs breaking change | Phase 0 | ecs_adapter 모듈 격리로 영향 범위 제한 |
| 동접 확장 한계 | Phase 3 | 부하 테스트 스크립트 자동화, 병목 프로파일링 |
| EntityId 충돌 (Snapshot 복원 후) | Phase 0 | generation+index 방식 + EntityAllocator 상태 저장 |
| 클라이언트 DoS (입력 폭주) | Phase 3 | 세션별 rate limit + input channel HWM 모니터링 |

---

# 9. 장기 확장 전략 (Phase 6+)

1. **Replay 모드**: Command Stream + Event Log 기록 → 전체 게임 재현
2. **Memory Arena per Tick**: WASM 측 tick 단위 Arena 할당/리셋 공식화
3. **Plugin Hot Reload 고도화**: ABI 버전 검사 + graceful migration
4. **멀티 노드 샤딩**: Zone 기반 서버 분산 (Phase 6 이후)
5. **PostgreSQL 연동**: Snapshot → WAL → DB 복구 체인
6. **Admin Dashboard**: 실시간 서버 모니터링 웹 UI

---

# 10. 일정 총괄 (1인 기준, 현실적 추정)

| 단계 | 기간 | 누적 |
|------|------|------|
| Phase 0: Core | 4~6주 | 4~6주 |
| Phase 1: WASM | 6~8주 | 10~14주 |
| Phase 2: MUD 완성 | 10~12주 | 20~26주 |
| Phase 3: 안정화 | 6~8주 | 26~34주 |
| Phase 4: 2D 확장 | 8~12주 | 34~46주 |

**총 예상: 10~12개월** (Phase 4까지)

> **일정 근거:**
> - Phase 2가 가장 많은 새 코드를 작성하는 단계 (네트워크, 게임 시스템, 파서, 퍼시스턴스)
> - Telnet 프로토콜 구현만 2~3주, 텍스트 명령 파서 2주, 게임 시스템 4~5주, 통합 테스트 2주
> - 각 Phase에 디버깅/리팩터링 버퍼 20% 포함
> - Phase 5는 클라이언트 기술 선택에 따라 별도 산정

---

# 11. 최종 목표

이 구현 계획의 최종 목적은:

**"MUD에서 시작해 2D MMO까지 확장 가능한 Rust 기반 결정론적 모듈형 서버 엔진"을
현실적인 단계와 검증 가능한 완료 조건으로 안정적으로 구축하는 것이다.**

핵심 전략:
- 초기에는 단순하게, 구조는 확장 가능하게
- 최적화는 프로파일링 이후에 수행
- 매 Phase 완료 시 동작하는 시스템이 존재
- 결정론은 처음부터, Persistence는 Phase 2부터
