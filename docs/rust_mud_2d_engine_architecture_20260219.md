# Rust 기반 MUD/2D 겸용 엔진 아키텍처 설계 문서 (v3)

---

# 변경 이력

| 버전 | 날짜 | 주요 변경 |
|------|------|-----------|
| v1 | 2026-02-19 | 초안 작성 |
| v2 | 2026-02-19 | 피드백 반영 요약본 |
| v3 | 2026-02-19 | v1 상세 내용 유지 + v2 수정사항 + 구조적 결함 보완 통합본 |
| v3.1 | 2026-02-19 | EntityId generation 정책, Snapshot versioning, Determinism Hash 정렬 기준 확정 |

### v3.1 변경 사항

- EntityId를 `(generation: u32, index: u32)` 구조로 변경. Phase 0에서 확정 필수.
- WorldSnapshot에 `schema_version: u32` 필드 추가. Snapshot 호환성 보장.
- Determinism Hash 정렬 기준 명시: EntityId → ComponentId → raw bytes 순.
- WASM ↔ ECS 접근 경로 명확화: Untyped 변환은 plugin_runtime에서 수행, ecs_adapter는 Typed API 유지.
- 네트워크 Backpressure 전략 추가 (Phase 3 초반).

### v3 주요 변경 사항

- ECS 추상화: trait 기반에서 모듈 격리(Adapter) 전략으로 변경
- Thread Boundary: async ↔ tick thread 간 채널 구조 상세 명시
- TPS: 30 기본 (configurable), MUD 20 권장
- Fuel 정책: "결정론적 실패 경로"로 재정의 (결정론 밖으로 추방하지 않음)
- MUD 공간 모델: Spatial Grid와 완전 분리, RoomGraph 모델 도입
- 직렬화: postcard 초기 사용, FlatBuffers는 프로파일링 후 선택적 도입
- Plugin Stateless 원칙 명문화
- Persistence 최소 전략 Phase 2에 포함
- Command Stream 설계 상세화 (WASM ↔ Host 데이터 흐름)
- WASM Memory Boundary, Crash Recovery, Schema Evolution 등 v1 상세 내용 복원

---

# 1. 설계 의도 (Design Intent)

## 1.1 핵심 목표

본 엔진은 다음을 달성하는 것을 목표로 한다.

- 단일 서버 코어로 MUD(Text 기반)와 2D MMORPG 동시 지원
- 게임 로직은 WASM 플러그인으로 완전 분리
- 서버 재시작 없는 Hot Plug 지원
- 고성능 / 고안정성 / 대규모 동접 대응
- 서버 authoritative 구조
- 장기적으로 2D 확장 및 Web 클라이언트 대응

## 1.2 핵심 철학

엔진은 "월드 시뮬레이션 코어"다.

플러그인은 "의사결정 로직 계층"이다.

절대 원칙:

- 물리 / 이동 / Spatial / ECS iteration은 Core에 둔다
- AI / 스킬 계산 / 퀘스트 로직은 WASM에 둔다
- Tick은 단일 고정 주기
- 모든 월드 변경은 이벤트 기반으로 처리

---

# 2. 전체 아키텍처

```
[ Client Layer ]
  ├─ Telnet (MUD)
  ├─ WebSocket (Web)
  └─ TCP (2D Native)

        │ (비동기 네트워크)
        ▼
[ Transport Layer ]
  └─ Transport Trait 추상화
        │
        ▼
[ Protocol Layer ]
  ├─ TextCodec   (MUD Telnet)
  ├─ BinaryCodec (2D Native)
  └─ JsonCodec   (WebSocket)

        │ MPSC Channel (input_rx)
        ▼
[ Tick Thread — 단일 쓰기 스레드 ]
  ┌─────────────────────────────────┐
  │ [ Session Layer ]               │
  │   ├─ Auth State                 │
  │   ├─ Player ID                  │
  │   ├─ Protocol Binding           │
  │   └─ Line Buffer (MUD 전용)    │
  │                                 │
  │ [ Core Engine ]                 │
  │   ├─ ECS Module (격리된 접근)   │
  │   ├─ SpaceModel (추상화)        │
  │   │   ├─ RoomGraphSpace (MUD)   │
  │   │   └─ GridSpace (2D)         │
  │   ├─ Interest Management        │
  │   ├─ Event Bus                  │
  │   ├─ Fixed Tick Scheduler       │
  │   ├─ Command Stream Processor   │
  │   └─ WASM Runtime (동기 호출)   │
  │                                 │
  │ [ Plugin Layer (WASM) ]         │
  │   ├─ Combat Logic               │
  │   ├─ Skill Logic                │
  │   ├─ AI Logic                   │
  │   ├─ Quest Logic                │
  │   └─ Script Logic               │
  └─────────────────────────────────┘
        │ MPSC Channel (output_tx)
        ▼
[ Async Broadcast Tasks ]
  └─ 프로토콜별 직렬화 후 전송

[ Persistence Layer ]
  ├─ Snapshot (bincode 파일)
  ├─ Event Log (Command Stream 기록)
  └─ DB Adapter (Phase 3 이후)
```

---

# 3. Thread Boundary 설계

## 3.1 스레드 구조

엔진은 두 종류의 실행 컨텍스트를 가진다.

### Async Runtime (Tokio)
- 클라이언트 연결 수락
- 패킷 수신 → 디코딩 → MPSC channel로 tick thread에 전달
- tick thread로부터 MPSC channel로 받은 메시지를 인코딩 → 전송
- DB I/O (Phase 3 이후)

### Tick Thread (단일 스레드, 동기)
- World 상태의 유일한 Writer
- ECS 조회 및 수정
- WASM Plugin 동기 호출
- Command Stream 처리
- 이벤트 발행 및 소비

## 3.2 채널 구조

```
[Async Net Task 1] ──┐
[Async Net Task 2] ──┤── input_tx ──→ [ input_rx ] ──→ Tick Thread
[Async Net Task N] ──┘                                    │
                                                           │
Tick Thread ──→ [ output_tx ] ──→ [ output_rx ] ──→ [Async Broadcast Task]
```

### Input Channel
- 타입: `mpsc::UnboundedSender<SessionInput>`
- SessionInput: `{ session_id, raw_bytes }`
- Tick thread는 매 tick 시작 시 `try_recv()`로 drain

### Output Channel
- 타입: `mpsc::UnboundedSender<SessionOutput>`
- SessionOutput: `{ session_id, payload: Vec<u8> }` (이미 직렬화된 상태)
- Async broadcast task가 해당 세션의 소켓으로 전송

> **설계 근거:** Unbounded channel을 사용하는 이유는 tick thread가 blocking되면 안 되기 때문이다.
> 대신 input channel에 High Water Mark 모니터링을 두어 클라이언트 과부하를 감지한다.
> Output은 tick thread에서 직렬화까지 완료하여 async 측에서 추가 연산이 없도록 한다.

## 3.3 금지 사항

- Tick thread에서 `.await` 호출 금지
- Async task에서 World 직접 접근 금지
- 스레드 간 `Arc<RwLock<World>>` 공유 금지

---

# 4. Tick 및 시뮬레이션 설계

## 4.1 TPS 정책

- 기본 TPS: **30** (configurable)
- MUD 모드 권장: **20 TPS** (50ms budget)
- 2D 모드 권장: **30 TPS** (33ms budget)

> **설계 근거:** 60 TPS는 격투 게임/FPS 수준이며 2D MMORPG에는 과잉이다.
> 30 TPS는 업계 표준(WoW 20 TPS, 대부분의 2D MMO 20~30 TPS)에 부합하며,
> WASM 호출 여유를 확보한다. MUD는 텍스트 명령 특성상 20 TPS면 체감 지연이 없다.

Tick은 로직 처리 빈도와 분리된다. 각 시스템은 자체 실행 주기를 가진다.

예:
- MovementSystem → 매 Tick
- CombatSystem → 2 Tick마다
- AISystem → 3~5 Tick마다
- MudCommandSystem → 2 Tick마다

## 4.2 Tick 루프 구조

```
loop {
    let tick_start = Instant::now();

    // 1. 네트워크 입력 수집 (input_rx drain)
    drain_input_channel();

    // 2. 세션별 Line Buffer 처리 (MUD: 완성된 줄만 CommandEvent로 변환)
    process_line_buffers();

    // 3. 입력을 이벤트로 변환
    convert_inputs_to_events();

    // 4. Core Systems 실행 (Movement, Physics, Collision)
    run_core_systems(current_tick);

    // 5. WASM 플러그인 호출 (결정 로직) → Command Stream 수집
    run_wasm_plugins(current_tick);

    // 6. Command Stream 처리 → 상태 변경 확정
    process_command_stream();

    // 7. Interest Management 필터링
    compute_broadcast_targets();

    // 8. 결과 직렬화 → output_tx로 전송
    broadcast_results();

    // 9. Tick Budget 체크 및 메트릭 기록
    record_tick_metrics(tick_start);

    // 10. 다음 Tick까지 sleep
    sleep_until_next_tick(tick_start);
}
```

### Backfilling 처리 (분할 패킷 대응)

유저가 `a`,`t`,`t`,`a`,`c`,`k` 처럼 분할 패킷을 보낼 수 있다.

따라서:
- `drain_input_channel()` 단계에서는 세션별 Line Buffer에 raw bytes를 누적
- `process_line_buffers()` 에서 개행 문자(`\n`) 감지 시에만 CommandEvent로 변환
- 불완전 입력은 **절대** Tick 로직에 전달하지 않음
- 2D 모드는 바이너리 패킷 단위로 처리하므로 Line Buffer 불필요

---

# 5. ECS 설계 (모듈 격리 전략)

## 5.1 왜 Trait 추상화가 아닌 모듈 격리인가

ECS의 query 시스템은 타입 시스템과 깊게 결합된 제네릭 연산이다.
`Query<(&Position, &Velocity), With<Player>>` 같은 구조를 trait 하나로 추상화하려면
제네릭 파라미터나 associated type이 과도하게 필요해져서 추상화 비용이 직접 사용보다 커진다.

> **설계 근거:** Trait 추상화 대신, ECS 접근을 Core 내부의 `ecs_adapter` 모듈에 격리한다.
> 나머지 코드는 이 모듈의 공개 API만 호출하므로, 백엔드 교체 시 이 모듈만 재작성하면 된다.

## 5.2 EntityId 정책 (Phase 0 확정 필수)

> **설계 근거:** 단순 `u64` 단조 증가는 Snapshot restore 후 ID 충돌,
> Entity 삭제 후 재사용 시 dangling reference 등 치명적 문제를 일으킨다.
> bevy_ecs와 동일한 generation+index 방식을 채택하여
> ID 재사용 시에도 세대(generation)가 달라 안전하게 구분된다.

```rust
/// 엔진 전용 Entity ID.
/// index: Entity 슬롯 번호 (재사용 가능)
/// generation: 해당 슬롯이 몇 번째로 사용되는지 (재사용 시 증가)
///
/// 동일 index라도 generation이 다르면 다른 Entity다.
/// Snapshot restore 시 generation 카운터도 함께 복원하여 ID 충돌을 방지한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId {
    pub index: u32,
    pub generation: u32,
}

impl EntityId {
    /// WASM ABI용 u64 직렬화. 상위 32bit = generation, 하위 32bit = index.
    pub fn to_u64(&self) -> u64 {
        ((self.generation as u64) << 32) | (self.index as u64)
    }

    pub fn from_u64(val: u64) -> Self {
        Self {
            index: val as u32,
            generation: (val >> 32) as u32,
        }
    }
}
```

### EntityId 생성기

```rust
/// Entity 슬롯 관리자.
/// 삭제된 Entity의 index를 재사용하되, generation을 증가시켜 구분한다.
struct EntityAllocator {
    /// 각 index의 현재 generation
    generations: Vec<u32>,
    /// 재사용 가능한 index 목록
    free_list: Vec<u32>,
    /// 다음 신규 할당 index
    next_index: u32,
}

impl EntityAllocator {
    fn allocate(&mut self) -> EntityId {
        if let Some(index) = self.free_list.pop() {
            // 재사용: generation 증가
            self.generations[index as usize] += 1;
            EntityId { index, generation: self.generations[index as usize] }
        } else {
            // 신규 할당
            let index = self.next_index;
            self.next_index += 1;
            self.generations.push(0);
            EntityId { index, generation: 0 }
        }
    }

    fn deallocate(&mut self, id: EntityId) -> bool {
        // generation 불일치 시 이미 해제된 Entity → 무시
        if self.generations.get(id.index as usize) != Some(&id.generation) {
            return false;
        }
        self.free_list.push(id.index);
        true
    }

    fn is_alive(&self, id: EntityId) -> bool {
        self.generations.get(id.index as usize) == Some(&id.generation)
    }
}
```

### bevy_ecs와의 매핑

```rust
/// ecs_adapter 내부에서 우리 EntityId ↔ bevy Entity 간 양방향 매핑 유지.
/// 외부에는 EntityId만 노출된다.
struct EntityMapping {
    to_bevy: HashMap<EntityId, bevy_ecs::entity::Entity>,
    from_bevy: HashMap<bevy_ecs::entity::Entity, EntityId>,
}
```

> **Snapshot 복원 시:** EntityAllocator의 `generations` 벡터와 `next_index`도
> 함께 저장/복원하여 복원 후 새로 생성되는 Entity가 기존 ID와 충돌하지 않도록 한다.

## 5.3 모듈 구조

```
engine_core/
  ├─ ecs_adapter/          ← ECS 백엔드 격리 계층
  │   ├─ mod.rs            ← 공개 API (spawn, despawn, get, set, query 등)
  │   ├─ bevy_backend.rs   ← 초기 구현 (bevy_ecs)
  │   └─ types.rs          ← EntityId, ComponentId 등 엔진 전용 타입
  ├─ systems/
  ├─ events/
  └─ ...
```

### 공개 API 예시

```rust
// ecs_adapter/mod.rs
// 외부 코드는 이 API만 사용한다. bevy_ecs 타입이 밖으로 노출되지 않는다.

pub fn spawn_entity() -> EntityId;
pub fn despawn_entity(id: EntityId);
pub fn get_component<T: Component>(id: EntityId) -> Option<&T>;
pub fn set_component<T: Component>(id: EntityId, component: T);
pub fn entities_with<T: Component>() -> impl Iterator<Item = EntityId>;
pub fn query_pairs<A: Component, B: Component>() -> impl Iterator<Item = (EntityId, &A, &B)>;
```

> **교체 시나리오:** bevy_ecs → hecs 또는 shipyard로 전환 시,
> `bevy_backend.rs`를 `hecs_backend.rs`로 교체하고 `mod.rs`의 내부 위임만 변경.
> 나머지 엔진 코드는 수정 불필요.

## 5.5 Component 설계 원칙

- Component는 순수 데이터 (로직 없음)
- Component 타입은 `ecs_adapter/types.rs`에서 엔진 전용으로 정의
- WASM과의 교환에 사용되는 Component는 별도 직렬화 가능 타입으로 래핑

## 5.6 WASM에서 ECS에 접근하는 경로

> **설계 결정:** ecs_adapter에 Untyped API(`get_component_raw(id, comp_id) -> &[u8]`)를
> 추가하지 않는다. Untyped 변환은 plugin_runtime(Host API 구현부)에서 수행한다.
>
> 이유: ecs_adapter에 `&[u8]` 반환 API를 노출하면 타입 안전성이 훼손되고,
> Component 직렬화 책임이 ecs_adapter와 plugin_runtime에 분산된다.

접근 흐름:

```
WASM Plugin
  → host_get_component(entity_id: u64, component_id: u32, out_ptr, out_len)
    → plugin_runtime 내부:
      1. EntityId::from_u64(entity_id)로 변환
      2. ComponentRegistry에서 component_id로 직렬화 함수 조회
      3. ecs_adapter의 Typed API로 Component 조회
      4. postcard 직렬화
      5. WasmMemoryView를 통해 WASM memory에 write
```

### Component Registry (Phase 1에서 구현)

```rust
/// Component 등록 시 직렬화/역직렬화 함수를 함께 등록한다.
/// WASM Host API가 ComponentId만으로 적절한 직렬화를 수행할 수 있도록 한다.
struct ComponentRegistry {
    serializers: HashMap<ComponentId, Box<dyn ComponentSerializer>>,
}

trait ComponentSerializer: Send + Sync {
    fn serialize_from_world(&self, world: &World, entity: EntityId) -> Option<Vec<u8>>;
    fn deserialize_to_command(&self, data: &[u8]) -> Result<EngineCommand, Error>;
}
```

---

# 6. 공간 모델 설계 (SpaceModel 추상화)

## 6.1 왜 MUD Room과 Spatial Grid를 분리하는가

MUD의 Room은 **그래프 구조**다 (Room A → north → Room B → east → Room C).
Spatial Grid는 **좌표 기반 연속 공간**이다.
Room을 Grid Cell로 매핑하면 "근접 검색" 기능이 MUD에서 의미가 없어지고,
Room 간 방향성 연결(north/south/east/west)을 표현할 수 없다.

## 6.2 SpaceModel Trait

```rust
/// 공간 모델 추상화.
/// MUD(RoomGraph)와 2D(Grid) 모두 이 인터페이스를 구현한다.
pub trait SpaceModel {
    /// 해당 엔티티와 같은 공간에 있는 모든 엔티티 반환
    fn entities_in_same_area(&self, entity: EntityId) -> Vec<EntityId>;

    /// 해당 엔티티의 인접 공간에 있는 엔티티 반환 (AOI 용도)
    fn neighbors(&self, entity: EntityId) -> Vec<EntityId>;

    /// 엔티티를 목적지로 이동. 성공 시 (이전 영역, 새 영역) 반환.
    fn move_entity(&mut self, entity: EntityId, destination: AreaId)
        -> Result<(AreaId, AreaId), MoveError>;

    /// 네트워크 브로드캐스트 대상 결정 (Interest Management)
    fn broadcast_targets(&self, entity: EntityId) -> Vec<EntityId>;
}
```

## 6.3 RoomGraphSpace (MUD 모드)

```rust
/// MUD 전용 공간 모델.
/// Room은 Entity로 표현되며, Exit 목록을 Component로 가진다.
struct RoomGraphSpace {
    /// Room Entity → 해당 Room에 있는 Entity 목록
    room_occupants: HashMap<EntityId, HashSet<EntityId>>,
    /// Entity → 현재 위치한 Room
    entity_room: HashMap<EntityId, EntityId>,
}
```

Room 간 연결은 ECS Component로 표현:

```rust
struct RoomExits {
    north: Option<EntityId>,
    south: Option<EntityId>,
    east: Option<EntityId>,
    west: Option<EntityId>,
    // 커스텀 출구도 지원
    custom: HashMap<String, EntityId>,
}
```

### 이동 시 이벤트 흐름

`move_entity()` 호출 시:
1. 이전 Room에서 Entity 제거
2. 새 Room에 Entity 추가
3. `AreaLeaveEvent { entity, old_room }` 발행 → 이전 Room 점유자에게 퇴장 메시지
4. `AreaEnterEvent { entity, new_room }` 발행 → 새 Room 점유자에게 진입 메시지

> **설계 근거:** 이벤트 발행 책임을 SpaceModel이 아닌 MovementSystem에 둔다.
> SpaceModel은 순수 데이터 조작만 수행하고, 이벤트 발행은 System 계층에서 처리한다.
> 이렇게 해야 SpaceModel을 테스트할 때 이벤트 시스템 의존성이 없다.

## 6.4 GridSpace (2D 모드)

```rust
/// 2D 전용 공간 모델.
/// 좌표 기반 Grid 또는 QuadTree로 구현한다.
struct GridSpace {
    cell_size: f32,
    grid: HashMap<(i32, i32), HashSet<EntityId>>,
    entity_positions: HashMap<EntityId, (f32, f32)>,
}
```

역할:
- 근접 엔티티 검색
- 충돌 후보 추출 (Broad Phase)
- 관심 영역(AOI) 계산
- 네트워크 브로드캐스트 필터링

---

# 7. WASM 플러그인 설계

## 7.1 책임 분리 원칙

### WASM이 담당하는 것
- AI 의사결정
- 스킬 수치 계산
- 퀘스트 조건 판단
- 룰 기반 이벤트 처리

### WASM이 절대 담당하지 않는 것
- Physics / Collision
- 대량 ECS iteration
- Spatial Query
- 네트워크 I/O

## 7.2 Plugin Stateless 원칙

- Plugin 내부에 **영구 상태 저장 금지**
- 모든 게임 상태는 ECS에 저장
- Plugin은 입력을 받아 Command를 출력하는 **순수 계산 모듈**
- Tick 간에 Plugin 내부 변수에 의존하는 로직 금지

> **설계 근거:** Stateless 원칙은 Hot Reload를 근본적으로 단순화한다.
> 새 Plugin을 로드해도 "이전 상태 마이그레이션" 문제가 발생하지 않는다.
> Plugin 교체 시나리오: 현재 tick 완료 → 기존 Plugin unload → 새 Plugin load → 다음 tick부터 적용.

## 7.3 Host ↔ Guest 경계 전략

### 문제
- WASM 함수 호출 비용 (FFI 오버헤드)
- Linear Memory 접근 비용
- 직렬화/역직렬화 비용

### 해결 전략
- 필요한 데이터만 전달 (전체 ECS 노출 금지)
- Entity ID 중심 인터페이스
- 대량 데이터는 Core에서 처리 후 결과만 전달
- 한 번의 호출에 필요한 모든 데이터를 묶어서 전달 (round-trip 최소화)

## 7.4 Host API (Core → Plugin)

```rust
// Plugin이 호출할 수 있는 Host 함수들
extern "C" {
    fn host_get_component(entity_id: u64, component_id: u32,
                          out_ptr: u32, out_len: u32) -> i32;
    fn host_emit_command(cmd_ptr: u32, cmd_len: u32) -> i32;
    fn host_log(level: u32, msg_ptr: u32, msg_len: u32);
    fn host_get_tick() -> u64;
    fn host_random_seed() -> u64;  // 결정론적 시드 제공
}
```

## 7.5 Plugin Entry Points (Plugin → Core)

```rust
// Core가 호출하는 Plugin 진입점
#[no_mangle]
pub extern "C" fn on_load() -> i32;

#[no_mangle]
pub extern "C" fn on_event(event_id: u32, payload_ptr: u32, payload_len: u32) -> i32;

#[no_mangle]
pub extern "C" fn on_tick(tick_number: u64) -> i32;
```

---

# 8. Command Stream 설계

## 8.1 왜 Command Stream인가

WASM Plugin이 ECS를 직접 수정하면:
- Host ↔ Guest 메모리 경계 위반 위험
- 동시에 여러 Plugin이 같은 Entity를 수정할 때 경합 발생
- 롤백 불가능

따라서 Plugin은 "의도"를 Command로 기록하고, Core가 일괄 처리한다.

## 8.2 Command 정의

```rust
enum EngineCommand {
    /// Component 값 설정
    SetComponent {
        entity: EntityId,
        component_id: ComponentId,
        data: Vec<u8>,  // postcard 직렬화된 데이터
    },
    /// Component 제거
    RemoveComponent {
        entity: EntityId,
        component_id: ComponentId,
    },
    /// 이벤트 발행 (다른 시스템/Plugin이 수신)
    EmitEvent {
        event_id: EventId,
        payload: Vec<u8>,
    },
    /// Entity 생성
    SpawnEntity {
        template_id: u32,  // 사전 정의된 Entity 템플릿
    },
    /// Entity 제거
    DestroyEntity {
        entity: EntityId,
    },
    /// 엔티티 이동 (SpaceModel에 위임)
    MoveEntity {
        entity: EntityId,
        destination: AreaId,
    },
}
```

## 8.3 WASM에서 Command를 쓰는 방식

```
[ WASM Linear Memory ]
┌─────────────────────────────────────────┐
│ ...                                     │
│ [Command Buffer Region]                 │
│   ├─ header: { count: u32, total_len }  │
│   ├─ cmd[0]: { type, entity, data... }  │
│   ├─ cmd[1]: { type, entity, data... }  │
│   └─ ...                                │
│ ...                                     │
└─────────────────────────────────────────┘
```

1. Plugin의 `on_tick()` 호출 전, Host가 Command Buffer 시작 offset을 알려줌
2. Plugin은 해당 offset부터 Command를 순차 기록
3. Plugin 반환 후, Host가 해당 영역을 읽어 `Vec<EngineCommand>`로 변환
4. **매 Plugin 호출마다 Buffer를 리셋** (cross-plugin 오염 방지)

## 8.4 Command 충돌 해결

한 tick에서 여러 Plugin이 같은 Entity의 같은 Component를 수정할 수 있다.

정책:
- **Plugin 실행 순서는 고정** (설정 파일에서 priority 지정)
- 같은 Entity/Component에 대한 마지막 Command가 승리 (Last Writer Wins)
- 충돌 발생 시 warning 로그 기록
- 치명적 충돌이 예상되는 Component는 "exclusive ownership" 태그로 보호

> **설계 근거:** 복잡한 병합 전략 대신 "고정 순서 + Last Writer Wins"를 선택한 이유는
> 구현 복잡도 대비 실용성이 높고, 결정론을 깨지 않기 때문이다.
> Exclusive ownership으로 대부분의 실질적 충돌을 사전에 방지할 수 있다.
>
> Delta 연산 Command(AddInt 등)를 도입하지 않는 이유:
> SetComponent(최종 상태 기록)와 AddInt(연산 기록)가 혼합되면 resolve() 로직이 복잡해지고,
> 적용 순서에 따라 결과가 달라지는 새로운 비결정론 경로가 생긴다.
> 동일한 문제를 아래 Intent Event 패턴으로 Command 타입 추가 없이 해결할 수 있다.

### Exclusive Ownership + Intent Event 패턴

공유 자원(HP, Mana 등)은 **단일 Plugin만 SetComponent를 수행**하고,
다른 Plugin은 **Intent Event를 발행**하여 간접적으로 변경을 요청한다.

```
┌─────────────────────────────────────────────────────┐
│ HealPlugin                                          │
│   → EmitEvent(HealIntent { target: E1, amount: 20 })│
│   (HP를 직접 수정하지 않음)                          │
└─────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────┐
│ DamagePlugin                                        │
│   → EmitEvent(DamageIntent { target: E1, amount: 30})│
│   (HP를 직접 수정하지 않음)                          │
└─────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────┐
│ CombatPlugin (HP의 exclusive owner)                  │
│   ← on_event(HealIntent) → +20 누적                 │
│   ← on_event(DamageIntent) → -30 누적               │
│   → SetComponent(HP, 80 + 20 - 30 = 70)            │
│   (유일한 HP 변경 권한)                              │
└─────────────────────────────────────────────────────┘
```

이 구조의 장점:
- HP에 대한 SetComponent는 CombatPlugin 한 곳에서만 발생 → LWW 충돌 자체가 불가능
- Heal과 Damage가 동시에 들어와도 CombatPlugin이 둘 다 반영
- Command Stream에 새로운 Command 타입(AddInt 등)을 추가할 필요 없음
- Exclusive Ownership 위반 시 CommandStream.resolve()에서 거부 + warning 로그

---

# 9. WASM Memory Boundary & Zero-Copy 전략

## 9.1 핵심 문제

WASM Linear Memory는 `memory.grow()` 발생 시 재할당되며, 기존 포인터가 무효화된다.

## 9.2 설계 원칙

### 1. 포인터 직접 보관 금지
WASM memory의 raw pointer를 Rust 변수에 저장하고 tick을 넘겨서 사용하면 UB 발생.

### 2. Offset 기반 접근 강제
- Host는 `(base_ptr + offset)` 형태로 매 접근 시 계산
- Plugin ABI는 항상 `offset + length` 기반

### 3. Stable Memory Wrapper 계층

```rust
/// WASM Linear Memory에 대한 안전한 접근 래퍼.
/// 매 호출 시 base pointer를 재획득하여 grow() 이후에도 안전하다.
struct WasmMemoryView<'a> {
    memory: &'a wasmtime::Memory,
    store: &'a wasmtime::Store<HostState>,
}

impl<'a> WasmMemoryView<'a> {
    /// offset에서 len 바이트를 읽는다. 매번 data_ptr()을 재획득.
    fn read(&self, offset: u32, len: u32) -> &[u8] {
        let base = self.memory.data(&self.store);
        &base[offset as usize..(offset + len) as usize]
    }

    /// offset에 data를 쓴다.
    fn write(&self, offset: u32, data: &[u8]) {
        let base = self.memory.data_mut(&mut self.store);
        base[offset as usize..offset as usize + data.len()].copy_from_slice(data);
    }
}
```

### 4. Tick Arena 설계
- Tick 시작 시 WASM 측 Arena 영역 지정
- Tick 종료 시 Arena reset
- Cross-tick pointer 보관 절대 금지

## 9.3 실질적 Zero-Copy 범위

> **현실 인정:** WASM ↔ Host 간 진정한 Zero-Copy는 불가능하다.
> FlatBuffers를 쓰더라도 WASM Linear Memory의 바이트를 Host가 해석하는 과정이 필요하고,
> grow() 문제 때문에 lazy parsing 모델과 상충한다.
>
> 따라서 초기에는 postcard로 단순 직렬화하고,
> 프로파일링 후 WASM ABI가 실제 병목으로 확인될 때만 FlatBuffers를 도입한다.
> 직렬화 계층은 교체 가능하도록 추상화해 둔다.

---

# 10. 직렬화 전략

## 10.1 초기 확정 규격

| 용도 | 포맷 | 근거 |
|------|------|------|
| 내부 상태 저장 / Snapshot | bincode | 성능 우수, Rust 생태계 친화적 |
| WASM ABI | postcard (Phase 1~3) | 경량, no_std 호환, WASM 친화적 |
| 네트워크 (MUD) | UTF-8 텍스트 | Telnet 호환 |
| 네트워크 (2D) | protobuf 또는 postcard | 클라이언트 언어에 따라 결정 |

## 10.2 교체 가능한 추상화

```rust
/// 직렬화 계층 추상화.
/// 백엔드 교체 시 이 trait의 구현체만 변경한다.
trait SerializeFormat {
    fn serialize<T: Serialize>(value: &T) -> Vec<u8>;
    fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error>;
}
```

## 10.3 FlatBuffers 도입 조건 (Phase 3 이후)

- WASM ABI가 프로파일링에서 병목으로 확인될 때만
- schema_version 필드 포함 필수
- Forward Compatibility 원칙 준수 (필드 삭제 금지, Optional 필드만 추가)

---

# 11. Fuel 정책 (결정론적 실패 경로)

## 11.1 핵심 재정의

> **Fuel 초과는 "비결정론적 이벤트"가 아니라 "결정론적 실패 경로"다.**
>
> Fuel 한도는 엔진 설정의 일부이며, Replay 스트림에 기록된다.
> 동일 Fuel 한도 + 동일 입력 = 동일 결과 (Fuel 초과에 의한 실패 포함).

## 11.2 기본 정책

- Plugin 1개당 Tick당 Fuel 상한: 엔진 설정에서 정의
- Fuel 한도는 **불변 상수**로 취급 (런타임 변경 금지)
- 변경 시 엔진 재시작 필요 (Replay 일관성 보장)

## 11.3 Fuel 초과 처리

1. 해당 Plugin execution 즉시 중단
2. 해당 tick에서 해당 Plugin이 생성한 Command **전부 폐기**
3. `PluginFuelExceeded { plugin_id, tick, fuel_used }` 이벤트 기록
4. 해당 엔티티 상태는 이 tick에서 변경되지 않음 (암묵적 롤백)

## 11.4 반복 초과 대응

- 3회 연속 Fuel 초과: Plugin quarantine (비활성화)
- 관리자 승인 후 재활성화
- Quarantine 이벤트도 Replay 스트림에 기록

---

# 12. 결정론적 시뮬레이션

## 12.1 전략: 로직 결정론 vs 시각적 보간 분리

### 반드시 결정론 유지 (WASM 영역)
- 데미지 계산 → 정수 또는 고정소수점
- 아이템 드랍 확률 → 결정론적 시드 기반 PRNG
- 상태 이상 판정 → 정수 비교
- 확률 계산 → `host_random_seed()`로 제공되는 시드 사용

### 허용 오차 영역 (Server Core)
- 이동 보간 → f32 사용 가능
- 시각 효과 → 클라이언트 측 처리
- 클라이언트 예측 이동 → 주기적 Snap Correction

## 12.2 Snap Policy

- 서버 authoritative 좌표를 N tick마다 강제 전송
- 허용 오차 범위 초과 시 즉시 보정
- Snap 주기는 configurable

## 12.3 Determinism Hash 검증

### 정렬 기준 (엄격히 준수)

> **설계 근거:** ECS 내부 저장 순서(HashMap iteration order, Component storage 순서)는
> 백엔드에 따라 달라진다. bevy_ecs → hecs 교체 시 Hash가 바뀌면 안 되므로,
> Hash 계산 시 ECS 내부 순서에 의존하지 않고 명시적 정렬을 강제한다.

정렬 순서:
1. **EntityId 기준 정렬** (index → generation 순)
2. 각 Entity 내에서 **ComponentId 기준 정렬**
3. 각 Component의 **raw bytes를 그대로 Hash**

```rust
fn compute_world_hash(world: &World, mode: HashMode) -> u64 {
    let mut hasher = DefaultHasher::new();

    // 1. Entity 목록을 EntityId 기준으로 정렬
    let mut entities: Vec<EntityId> = world.all_entities().collect();
    entities.sort_by_key(|e| (e.index, e.generation));

    for entity in &entities {
        entity.index.hash(&mut hasher);
        entity.generation.hash(&mut hasher);

        match mode {
            HashMode::Debug => {
                // 2. 해당 Entity의 모든 Component를 ComponentId 순으로 정렬
                let mut components = world.all_components_raw(*entity);
                components.sort_by_key(|(comp_id, _)| comp_id.0);

                for (comp_id, raw_bytes) in &components {
                    // 3. ComponentId + raw bytes를 Hash
                    comp_id.0.hash(&mut hasher);
                    raw_bytes.hash(&mut hasher);
                }
            }
            HashMode::Production => {
                // Critical Component만 (HP, Position, Inventory)
                // 동일한 ComponentId 정렬 + raw bytes hash 적용
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

### Debug 모드
- N tick마다 전체 월드 상태 Hash 계산
- Replay 모드에서 동일 Hash 비교
- 불일치 발생 시 tick 번호 + diff 기록

### Production 모드
- Critical Component (HP, Position, Inventory)만 Hash
- 성능 영향 최소화

---

# 13. 네트워크 설계

## 13.1 Transport 추상화

```rust
/// Transport와 Protocol을 분리한다.
/// Transport는 바이트 송수신만 담당.
/// Protocol은 그 바이트의 인코딩/디코딩을 담당.
#[async_trait]
trait Transport {
    async fn read(&mut self) -> Result<Vec<u8>, Error>;
    async fn write(&mut self, data: &[u8]) -> Result<(), Error>;
    async fn close(&mut self);
}
```

구현체:
- TelnetTransport (IAC negotiation 포함)
- WebSocketTransport
- TcpTransport (2D Native)

> **TelnetTransport 구현 주의:** RFC 854에 따라 데이터 스트림 내 0xFF 바이트는
> 0xFF 0xFF로 이스케이프해야 한다. 기본 인코딩은 UTF-8이므로 0xFF가 데이터에
> 등장할 일은 없지만, IAC 파서는 이 이스케이프를 반드시 처리해야 한다.
> EUC-KR 등 레거시 인코딩 지원 시(Phase 5) 0x80~0xFF 범위 바이트와의 충돌이
> 발생하므로 그 시점에서 바이트 이스케이프 처리를 재검증한다.

## 13.2 Protocol 추상화

```rust
trait ProtocolCodec {
    fn decode(&self, raw: &[u8]) -> Result<GameMessage, Error>;
    fn encode(&self, msg: &GameMessage) -> Vec<u8>;
}
```

구현체:
- TextCodec (MUD Telnet)
- BinaryCodec (2D Native)
- JsonCodec (WebSocket)

## 13.3 Interest Management

2D MMO에서 가장 큰 비용은 "누구에게 무엇을 보낼 것인가"이다.

SpaceModel의 `broadcast_targets()` + Observer 패턴 기반 필터링을 Core에 포함한다.

MUD 모드에서는 "같은 Room에 있는 플레이어"가 broadcast 대상이므로 단순하다.

## 13.4 네트워크 Backpressure 전략 (Phase 3 초반 구현)

> **설계 근거:** 동접 100명 수준에서는 LineBuffer의 max_size 방어로 충분하지만,
> 200명 이상 또는 악의적 클라이언트 대응을 위해 명시적 backpressure가 필요하다.
> Phase 2에서는 최소 방어(LineBuffer max_size)만 적용하고,
> Phase 3 초반에 아래 전략을 구현한다.

### 세션별 Rate Limiting

```rust
/// 세션별 입력 속도 제한.
/// 초당 최대 명령 수를 초과하면 경고 후 disconnect.
struct SessionRateLimit {
    max_commands_per_second: u32,  // 기본 20
    window_start: Instant,
    command_count: u32,
}

impl SessionRateLimit {
    fn check(&mut self) -> RateLimitResult {
        if self.window_start.elapsed() >= Duration::from_secs(1) {
            self.window_start = Instant::now();
            self.command_count = 0;
        }
        self.command_count += 1;
        if self.command_count > self.max_commands_per_second {
            RateLimitResult::Exceeded
        } else {
            RateLimitResult::Ok
        }
    }
}
```

### Input Channel 모니터링

- Unbounded channel은 유지하되, drain 시 현재 큐 길이를 메트릭으로 기록
- High Water Mark(기본 1000) 초과 시 경고 로그
- 특정 세션의 입력이 전체의 50% 이상을 차지하면 해당 세션 throttle

---

# 14. Time Budget & Stratified Tick Scheduling

## 14.1 Stratified Tick Layering

30 TPS(33ms budget) 기준:

### Tier 0 — Essential (매 tick)
- 네트워크 I/O drain
- 입력 처리
- 상태 확정 (Command Stream 처리)
- 물리 이동

### Tier 1 — Heavy (N tick마다 분산)
- AI 의사결정
- AOI 갱신
- 경로 탐색

### Tier 2 — Background (유휴 시간 기반)
- 로깅 flush
- 메트릭 집계
- Snapshot 큐 처리
- 통계 계산

## 14.2 Time Slicing 전략

특히 AOI 갱신은 전체 유저를 N등분:

```
if entity_id % N == current_tick % N {
    process_aoi(entity);
}
```

효과:
- CPU 스파이크 방지
- 평균 부하 평탄화
- 최대 지연 시간 상한선 계산 가능

## 14.3 Tick Budget Enforcement

```rust
let budget = Duration::from_millis(33); // 30 TPS
let tier0_budget = budget * 60 / 100;   // 60%
let tier1_budget = budget * 30 / 100;   // 30%
let tier2_budget = budget * 10 / 100;   // 10%

// 각 Tier 실행 후 경과 시간 확인
// 초과 시 하위 Tier skip
// skip된 Tier는 다음 tick에서 우선 실행
```

---

# 15. Schema Evolution Strategy

## 15.1 버전 관리

- 모든 WASM ABI 메시지에 `schema_version: u16` 포함
- Host는 구버전 Plugin을 감지 가능

## 15.2 Forward Compatibility 원칙

- 필드 삭제 금지
- Optional 필드만 추가
- Deprecated 필드는 유지하되 무시

## 15.3 ABI Version Table

Engine은 지원 가능한 Plugin ABI 버전 목록을 유지한다.

Plugin 로드 시:
- Major ABI 불일치 → 로드 거부
- Minor mismatch → 호환 모드 실행 (missing optional 필드는 default)

---

# 16. Crash Recovery & Panic Handling 전략

## 16.1 격리 원칙

WASM Plugin 패닉은 시스템 전체 크래시로 이어지면 **절대** 안 된다.

Plugin Panic 발생 시:
1. 해당 Plugin의 이번 tick Command 전부 폐기
2. 해당 엔티티 상태는 이 tick에서 변경되지 않음
3. Panic 로그 기록 (plugin_id, tick, error)

## 16.2 에스컬레이션 정책

| 상황 | 대응 |
|------|------|
| 1회 Panic | Command 폐기 + 경고 로그 |
| 3회 연속 Panic | Plugin quarantine (비활성화) |
| 치명적 메모리 위반 | 즉시 Plugin unload |
| Fuel 반복 초과 | Plugin quarantine |

## 16.3 Hot Reload 시나리오

Plugin Stateless 원칙에 의해 Hot Reload는 단순하다:

1. 현재 tick 완료 대기
2. 기존 Plugin instance 폐기 (WASM instance drop)
3. 새 Plugin 바이너리 로드 + 새 WASM instance 생성
4. `on_load()` 호출로 초기화
5. 다음 tick부터 새 Plugin으로 실행

> **주의:** ABI 버전이 다르면 Schema Evolution 정책에 따라 호환성 검사 후 로드 결정.

---

# 17. Persistence 전략

## 17.1 Phase 2 최소 구현

- **bincode Snapshot**: 전체 ECS 상태를 주기적으로 파일로 덤프
- **서버 재시작 시**: 마지막 Snapshot에서 복구
- **Snapshot 주기**: configurable (기본 60초)
- **Snapshot 보관**: 최근 N개 유지, 나머지 삭제

```rust
/// 스냅샷 최상위 구조.
/// schema_version으로 하위 호환성을 보장한다.
struct WorldSnapshot {
    /// Snapshot 포맷 버전. 구조 변경 시 증가.
    /// Loader는 이 값에 따라 적절한 역직렬화 경로를 선택한다.
    schema_version: u32,
    tick: u64,
    timestamp: SystemTime,
    entities: Vec<EntitySnapshot>,
    rooms: Vec<RoomSnapshot>,  // MUD 모드
    /// EntityAllocator 상태.
    /// Snapshot 복원 후 새로 생성되는 Entity가 기존 ID와 충돌하지 않도록 한다.
    entity_allocator_state: EntityAllocatorSnapshot,
}

/// EntityAllocator 복원에 필요한 최소 상태.
struct EntityAllocatorSnapshot {
    generations: Vec<u32>,
    free_list: Vec<u32>,
    next_index: u32,
}
```

> **설계 근거:** `schema_version`이 없으면 Component 구조 변경이나
> EntityId 정책 변경 시 기존 Snapshot을 역직렬화할 수 없게 된다.
> 비용은 `u32` 필드 하나뿐이지만, 없으면 Phase 3 이후 리팩터링 시 재앙.
>
> Snapshot Loader는 version에 따라 분기하며,
> Migration 함수 테이블은 Phase 3에서 필요 시 추가한다.

## 17.2 Phase 3 이후 확장

- Command Stream 기반 Event Log (WAL 방식)
- PostgreSQL 연동
- Snapshot + WAL 결합 복구

---

# 18. 관측 가능성 (Observability)

초기 설계에 반드시 포함.

필수 요소:
- structured logging (tracing crate)
- tick duration 측정
- system execution time 측정 (Tier별)
- entity iteration count 기록
- WASM Plugin 실행 시간 / Fuel 소비량
- Command Stream 처리량
- Prometheus metrics 노출

Tick 루프는 **항상** 성능 지표를 기록한다.

---

# 19. 기술 스택

| 영역 | 선택 | 근거 |
|------|------|------|
| 언어 | Rust | 메모리 안전성, 성능, WASM 생태계 |
| Async Runtime | Tokio | 업계 표준, 성숙한 생태계 |
| ECS (초기) | bevy_ecs | 성숙도 높음. 단, ecs_adapter로 격리 |
| WASM Runtime | wasmtime | Fuel injection, 안정성, Bytecode Alliance 지원 |
| 직렬화 (내부) | bincode / postcard | Rust 친화적, 고성능 |
| 직렬화 (네트워크) | protobuf (2D), UTF-8 (MUD) | 다중 언어 클라이언트 대응 |
| DB (Phase 3+) | PostgreSQL | 관계형 데이터, 검증된 안정성 |
| Cache (Phase 3+) | Redis | 세션, 랭킹 등 |
| Logging | tracing | 구조화 로깅, span 기반 성능 측정 |
| Metrics | Prometheus | 업계 표준 모니터링 |

---

# 20. 단계별 개발 로드맵 (요약)

| Phase | 핵심 목표 | 주요 산출물 |
|-------|-----------|-------------|
| 0 | 엔진 코어 골격 | Tick Loop, ECS, Command Stream |
| 1 | WASM 통합 | wasmtime, Memory Wrapper, Fuel |
| 2 | Playable MUD | Telnet, RoomGraph, 전투, Snapshot |
| 3 | 성능 안정화 | Stratified Tick, Determinism 검증 |
| 4 | 2D MMO 확장 | GridSpace, AOI, WebSocket, Delta |
| 5 | 클라이언트 확장 | Web / Native / Mobile |

상세 일정은 구현 계획서 참조.

---

# 21. 핵심 성공 조건 (최종)

1. **Tick 단일화** — 모든 게임 로직이 하나의 tick 루프에서 처리
2. **SpaceModel 추상화** — MUD Graph와 2D Grid를 동일 인터페이스로
3. **WASM 역할 제한** — 순수 계산만, Stateless, Core 기능 침범 금지
4. **Command Stream** — Plugin이 ECS를 직접 수정하지 않는 간접 수정 구조
5. **직렬화 교체 가능** — 초기 postcard, 필요 시 FlatBuffers
6. **Fuel = 결정론적 파라미터** — Replay 일관성 보장
7. **Thread Boundary 엄수** — 단일 쓰기 스레드, async와 채널로만 통신
8. **Observability 초기 내장** — 첫 tick부터 측정 가능
9. **EntityId Generation 정책** — Phase 0에서 확정, Snapshot 복원과 양립 가능한 구조
10. **Snapshot Versioning** — schema_version으로 하위 호환성 보장

이 원칙을 지키면 "MUD에서 시작해 2D MMORPG로 확장"하는 전략에 최적화된,
장기적으로 상용급 MMO 엔진으로 발전 가능한 구조가 된다.
