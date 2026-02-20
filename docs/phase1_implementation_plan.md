# Phase 1 구현 계획: WASM Runtime 통합

## Context

Phase 0에서 완성된 결정론적 시뮬레이션 루프(ECS, CommandStream, EventBus, TickLoop, SpaceModel) 위에
**WASM 플러그인 런타임**을 통합합니다.

목표는 **WASM 플러그인이 Command Stream을 생성하고, Core가 이를 안전하게 처리하는 전체 파이프라인**을 완성하는 것입니다.

### Phase 0 기반 (이미 구현됨)
- `ecs_adapter`: EntityId(generation+index), EcsAdapter, EntityAllocator, Component re-export
- `engine_core`: EngineCommand, CommandStream(LWW), EventBus, TickLoop(step/run)
- `space`: SpaceModel trait, RoomGraphSpace, RoomExits
- `observability`: init_logging(), TickMetrics

## 기술 스택 추가

- `wasmtime` (latest stable, Fuel 지원) — WASM 실행 런타임
- `postcard` + `serde` — WASM ABI 직렬화 (no_std 호환, 경량)
- `plugin_abi` — Host/Guest 공유 ABI 타입 (native + wasm32 동시 컴파일)

## 프로젝트 구조 변경

```
rust_mud_engine/
├── crates/
│   ├── ecs_adapter/              (기존 유지)
│   ├── engine_core/              (tick.rs 수정: PluginRuntime 통합)
│   ├── space/                    (기존 유지)
│   ├── observability/            (기존 유지)
│   ├── plugin_abi/               ← NEW: Host/Guest 공유 ABI 타입
│   │   ├── Cargo.toml            (no_std 호환, serde + postcard)
│   │   └── src/
│   │       └── lib.rs            ← WasmCommand, ABI 상수, 반환 코드
│   └── plugin_runtime/           ← NEW: WASM 런타임 (Host 측)
│       ├── Cargo.toml            (wasmtime, postcard, ecs_adapter 의존)
│       └── src/
│           ├── lib.rs            ← pub mod + PluginRuntime 공개 API
│           ├── config.rs         ← PluginConfig, FuelConfig
│           ├── memory.rs         ← WasmMemoryView (안전한 Linear Memory 접근)
│           ├── host_api.rs       ← Host 함수 (host_emit_command, host_log 등)
│           ├── plugin.rs         ← LoadedPlugin, PluginState, quarantine
│           ├── registry.rs       ← ComponentRegistry (ComponentId → 직렬화 함수)
│           ├── serializer.rs     ← WasmSerializer trait + PostcardSerializer
│           └── error.rs          ← PluginError
├── plugins/                      ← NEW: 테스트 WASM 플러그인
│   └── test_movement/
│       ├── Cargo.toml            (wasm32-unknown-unknown 타겟)
│       └── src/lib.rs            ← on_load, on_tick → MoveEntity 생성
└── tests/
    ├── (기존 tick_determinism, tick_simulation, space_test)
    ├── wasm_plugin_test.rs       ← WASM 플러그인 통합 테스트
    ├── fuel_determinism.rs       ← Fuel + 결정론 테스트
    └── memory_grow_stress.rs     ← grow() 10,000회 스트레스 테스트
```

## 핵심 설계 결정

### 1. Host ↔ Guest 통신 방식

Phase 1에서는 **per-command host call** 방식을 사용합니다:
- Plugin이 `host_emit_command(cmd_ptr, cmd_len)`을 호출할 때마다 Host가 즉시 수집
- 구현이 단순하고 디버깅이 쉬움
- 프로파일링 후 batch 방식(Linear Memory 일괄 기록)으로 최적화 가능 (Phase 3)

### 2. WASM 타겟

- `wasm32-unknown-unknown` 사용 (WASI 불필요, 파일/네트워크 접근 원천 차단)
- Plugin은 순수 계산만 수행 (Stateless 원칙)

### 3. Fuel 정책

- Plugin 1개당 Tick당 고정 Fuel 상한 (설정 파일에서 정의)
- Fuel 한도는 런타임 불변 (변경 시 엔진 재시작 필요)
- Fuel 초과 시: 해당 Plugin의 이번 tick Command 전부 폐기 (암묵적 롤백)
- 동일 입력 + 동일 Fuel = 동일 결과 (결정론 보장)

### 4. Panic 격리 & Quarantine

- wasmtime trap → 해당 Plugin의 Command 폐기 + 경고 로그
- 3회 연속 실패 (panic 또는 fuel 초과) → Plugin quarantine (비활성화)
- quarantine된 Plugin은 on_tick 호출 생략

---

## ABI 설계

### Guest → Host 함수 (Plugin이 호출)

```rust
// WASM에서 import하는 Host 함수들
extern "C" {
    /// 직렬화된 WasmCommand를 Host에 전달.
    /// cmd_ptr: WASM Linear Memory 내 postcard 직렬화된 WasmCommand 시작 offset
    /// cmd_len: 바이트 길이
    /// 반환: 0=성공, 음수=에러 코드
    fn host_emit_command(cmd_ptr: u32, cmd_len: u32) -> i32;

    /// 로그 출력.
    /// level: 0=trace, 1=debug, 2=info, 3=warn, 4=error
    fn host_log(level: u32, msg_ptr: u32, msg_len: u32);

    /// 현재 tick 번호 조회.
    fn host_get_tick() -> u64;

    /// 결정론적 난수 시드 조회.
    /// 매 tick마다 고정된 시드를 반환하여 결정론을 보장한다.
    fn host_random_seed() -> u64;

    /// Component 조회.
    /// entity_id: EntityId.to_u64()
    /// component_id: ComponentId.0
    /// out_ptr: 결과를 쓸 WASM Linear Memory offset
    /// out_cap: 출력 버퍼 용량
    /// 반환: 실제 쓴 바이트 수. 음수=에러 코드
    fn host_get_component(entity_id: u64, component_id: u32,
                          out_ptr: u32, out_cap: u32) -> i32;
}
```

### Host → Guest 함수 (Core가 호출)

```rust
// WASM이 export하는 진입점
#[no_mangle]
pub extern "C" fn on_load() -> i32;                    // 0=성공

#[no_mangle]
pub extern "C" fn on_tick(tick_number: u64) -> i32;    // 0=성공

#[no_mangle]
pub extern "C" fn on_event(event_id: u32,
                           payload_ptr: u32,
                           payload_len: u32) -> i32;   // 0=성공
```

### 공유 타입 (plugin_abi)

```rust
/// WASM ABI용 Command. u64/u32 원시 타입만 사용.
#[derive(Serialize, Deserialize)]
pub enum WasmCommand {
    SetComponent { entity_id: u64, component_id: u32, data: Vec<u8> },
    RemoveComponent { entity_id: u64, component_id: u32 },
    EmitEvent { event_id: u32, payload: Vec<u8> },
    SpawnEntity { tag: u64 },
    DestroyEntity { entity_id: u64 },
    MoveEntity { entity_id: u64, target_room_id: u64 },
}

/// ABI 버전. Major 불일치 시 로드 거부.
pub const ABI_VERSION_MAJOR: u32 = 1;
pub const ABI_VERSION_MINOR: u32 = 0;

/// 반환 코드.
pub const RESULT_OK: i32 = 0;
pub const RESULT_ERR_SERIALIZE: i32 = -1;
pub const RESULT_ERR_OUT_OF_BOUNDS: i32 = -2;
pub const RESULT_ERR_UNKNOWN_COMPONENT: i32 = -3;
pub const RESULT_ERR_ENTITY_NOT_FOUND: i32 = -4;
```

---

## 구현 단계 (12 Steps)

### Step 1: plugin_abi crate 생성
- `WasmCommand` enum (Serialize/Deserialize via postcard)
- ABI 버전 상수, 반환 코드 상수
- `no_std` 호환 (wasm32 타겟에서도 사용 가능)
- WasmCommand ↔ EngineCommand 변환 함수 (native 전용)
- **검증:** `cargo build -p plugin_abi`, postcard round-trip 테스트

### Step 2: plugin_runtime crate 골격 + error 타입
- Cargo.toml (wasmtime, postcard, ecs_adapter, plugin_abi 의존)
- `PluginError` enum: WasmTrap, FuelExceeded, MemoryOutOfBounds, SerializationError, Quarantined, LoadError
- 모듈 구조 (lib.rs, config.rs, memory.rs, host_api.rs, plugin.rs, registry.rs, serializer.rs, error.rs)
- **검증:** `cargo build -p plugin_runtime` 성공

### Step 3: WasmSerializer trait + PostcardSerializer
- `WasmSerializer` trait: `serialize<T: Serialize>` → `Vec<u8>`, `deserialize<T: DeserializeOwned>` → `Result<T>`
- `PostcardSerializer` 구현체
- 직렬화 계층 교체 가능 (Phase 3에서 FlatBuffers 도입 시 구현체만 교체)
- **검증:** WasmCommand postcard round-trip, 다양한 variant 테스트

### Step 4: WasmMemoryView 구현
- `WasmMemoryView<'a>`: wasmtime::Memory + &mut Store<HostState> 래핑
- `read_bytes(offset, len) -> Result<Vec<u8>, PluginError>`: 매번 data_ptr 재획득
- `write_bytes(offset, data) -> Result<(), PluginError>`: 경계 검사 + 복사
- OOB 접근 시 `PluginError::MemoryOutOfBounds` 반환 (panic 아님)
- **검증:** 기본 read/write, OOB 에러, grow() 이후 안전성

### Step 5: HostState + Host API 함수 등록
- `HostState` 구조체: 현재 tick, random seed, 수집된 commands, ComponentRegistry 참조
- `host_emit_command`: WasmMemoryView로 데이터 읽기 → postcard 역직렬화 → commands에 추가
- `host_log`: WasmMemoryView로 문자열 읽기 → tracing 출력
- `host_get_tick`: HostState에서 현재 tick 반환
- `host_random_seed`: tick 기반 결정론적 시드 반환
- `host_get_component`: ComponentRegistry에서 조회 → 직렬화 → WasmMemoryView에 쓰기
- wasmtime Linker에 모든 Host 함수 등록
- **검증:** 각 Host 함수의 단위 테스트 (mock WASM instance)

### Step 6: PluginConfig + FuelConfig
- `PluginConfig`: plugin_id, wasm_path, priority(실행 순서), fuel_limit, enabled
- `FuelConfig`: default_fuel_limit, max_consecutive_failures(기본 3)
- `PluginManifest`: 여러 Plugin의 설정 목록 (priority 순 정렬)
- **검증:** 설정 직렬화/역직렬화, priority 정렬

### Step 7: LoadedPlugin + quarantine 로직
- `PluginState` enum: Active, Quarantined { since_tick, reason }
- `LoadedPlugin`: id, priority, wasmtime::Instance, Store<HostState>, fuel_limit, state, consecutive_failures
- `execute_on_tick(tick)` 메서드: Fuel 충전 → on_tick 호출 → 결과 처리
- `execute_on_load()` 메서드: Plugin 초기화
- trap/fuel 초과 시 consecutive_failures 증가, 성공 시 0으로 리셋
- 3회 연속 실패 → Quarantined 전환
- Quarantined 상태에서는 on_tick 호출 생략
- **검증:** 정상 실행, fuel 초과, panic, 3회 연속 quarantine

### Step 8: ComponentRegistry
- `ComponentSerializer` trait: `serialize_from_ecs(&self, ecs: &EcsAdapter, entity: EntityId) -> Option<Vec<u8>>`
- `ComponentRegistry`: HashMap<ComponentId, Box<dyn ComponentSerializer>>
- `register<C: Component + Serialize>()` 매크로/함수
- host_get_component에서 ComponentRegistry를 통해 직렬화 수행
- **검증:** 등록 → 조회 → 직렬화 round-trip

### Step 9: PluginRuntime 공개 API
- `PluginRuntime`: wasmtime::Engine + Vec<LoadedPlugin> + ComponentRegistry
- `new(fuel_config) -> Self`
- `load_plugin(config: PluginConfig) -> Result<(), PluginError>`: WASM 바이너리 로드 → 컴파일 → Instance 생성 → on_load 호출
- `run_tick(tick, ecs, space, event_bus) -> Vec<EngineCommand>`: priority 순으로 각 Plugin 실행 → WasmCommand 수집 → EngineCommand 변환
- `unload_plugin(id)`: Plugin 제거 (Hot Reload 대비)
- `quarantined_plugins() -> Vec<PluginId>`: 격리된 Plugin 목록
- **검증:** 전체 로드→실행→언로드 사이클

### Step 10: TickLoop 통합 (engine_core 수정)
- TickLoop에 `Option<PluginRuntime>` 필드 추가
- `step()` 수정: Command resolve 전에 `plugin_runtime.run_tick()` 호출
- Plugin이 생성한 EngineCommand를 CommandStream에 push
- WASM 실행 시간을 TickMetrics에 추가 (`wasm_duration_us` 필드)
- **검증:** PluginRuntime 없이도 기존 동작 유지 (backward compatible)

### Step 11: 테스트 WASM 플러그인 작성
- `plugins/test_movement/`: Rust → wasm32-unknown-unknown 컴파일
- `on_load()`: 초기화, 0 반환
- `on_tick(tick)`: tick % 3 == 0일 때 host_get_tick() 호출 후 MoveEntity Command 생성
- `host_emit_command()` 호출로 Command 전달
- 간단한 Guest 측 allocator (bump allocator)
- 빌드 스크립트 또는 Makefile로 .wasm 파일 생성
- **검증:** .wasm 바이너리 정상 생성, export 함수 존재 확인

### Step 12: 통합 테스트 + 스트레스 테스트
- `wasm_plugin_test.rs`: WASM 플러그인 로드 → tick 실행 → 이동 Command 확인
- `fuel_determinism.rs`: 동일 seed + 동일 Fuel → 동일 결과 검증
- `memory_grow_stress.rs`: grow() 10,000회 + read/write 안전성 검증
- quarantine 테스트: 의도적 panic Plugin → 3회 후 quarantine 확인
- **검증:** 모든 Phase 1 완료 조건 충족

---

## 의존성 순서

```
Step 1 (plugin_abi)
  → Step 2 (plugin_runtime 골격)
    → Step 3 (serializer)     ─┐
    → Step 4 (WasmMemoryView) ─┤── 병렬 가능
    → Step 6 (config)         ─┘
      → Step 5 (Host API)
        → Step 7 (LoadedPlugin + quarantine)
          → Step 8 (ComponentRegistry) ── 병렬 가능 → Step 11 (test plugin)
            → Step 9 (PluginRuntime API)
              → Step 10 (TickLoop 통합)
                → Step 12 (통합 테스트)
```

---

## 핵심 구조체 상세

### HostState (Plugin 실행 중 Host 측 상태)

```rust
/// 각 Plugin Instance의 Store에 저장되는 Host 상태.
/// Plugin 실행 중 Host 함수가 접근하는 컨텍스트.
pub struct HostState {
    /// 현재 tick 번호
    pub current_tick: u64,
    /// 결정론적 랜덤 시드 (tick + plugin_id 기반)
    pub random_seed: u64,
    /// 이번 tick에서 이 Plugin이 생성한 Command 목록
    pub pending_commands: Vec<WasmCommand>,
    /// WASM Memory 참조 (on_tick 호출 전에 설정)
    pub memory: Option<wasmtime::Memory>,
    /// Component Registry 참조용 데이터 (직렬화된 스냅샷)
    /// 주의: Host 함수에서 EcsAdapter에 직접 접근할 수 없으므로
    /// 필요한 Component 데이터는 호출 전에 준비
    pub component_data_cache: HashMap<(u64, u32), Vec<u8>>,
}
```

### WasmMemoryView 안전성 보장

```rust
/// grow() 이후에도 안전한 WASM Linear Memory 접근.
/// 핵심: 매 read/write마다 memory.data()/data_mut()을 재호출하여
///        grow() 이후 invalidated pointer를 사용하는 것을 방지.
///
/// store를 &mut로 보유하므로 Rust borrow checker가
/// 동시에 두 개의 WasmMemoryView 생성을 컴파일 타임에 차단한다.
pub struct WasmMemoryView<'a> {
    memory: wasmtime::Memory,
    store: &'a mut wasmtime::StoreContextMut<'a, HostState>,
}
```

### Plugin 실행 흐름 (1 tick)

```
PluginRuntime.run_tick(tick, ecs, space, event_bus)
  │
  ├─ plugins를 priority 순으로 정렬 (이미 정렬된 상태 유지)
  │
  ├─ for each active plugin:
  │   │
  │   ├─ if quarantined → skip
  │   │
  │   ├─ store.set_fuel(plugin.fuel_limit)     // Fuel 충전
  │   │
  │   ├─ host_state.pending_commands.clear()   // 이전 Plugin 명령 초기화
  │   ├─ host_state.current_tick = tick
  │   ├─ host_state.random_seed = deterministic_seed(tick, plugin.id)
  │   │
  │   ├─ match plugin.call_on_tick(tick):
  │   │   ├─ Ok(0) → 성공
  │   │   │   ├─ consecutive_failures = 0
  │   │   │   └─ pending_commands → WasmCommand → EngineCommand 변환 → 수집
  │   │   │
  │   │   ├─ Err(trap) if fuel_exhausted →
  │   │   │   ├─ pending_commands 폐기 (암묵적 롤백)
  │   │   │   ├─ consecutive_failures += 1
  │   │   │   ├─ warn! 로그
  │   │   │   └─ check_quarantine()
  │   │   │
  │   │   └─ Err(trap) →
  │   │       ├─ pending_commands 폐기
  │   │       ├─ consecutive_failures += 1
  │   │       ├─ warn! 로그
  │   │       └─ check_quarantine()
  │   │
  │   └─ (다음 Plugin)
  │
  └─ 수집된 모든 EngineCommand 반환
```

---

## Phase 1 완료 조건

| 조건 | 검증 방법 |
|------|-----------|
| WASM Plugin에서 MoveEntity Command 생성 | wasm_plugin_test 통합 테스트 |
| Fuel 초과 시 안전 종료 + Command 폐기 | fuel_determinism 테스트 |
| 무한 루프 Plugin이 엔진을 멈추지 않음 | Fuel에 의해 tick budget 내 중단 |
| grow() 10,000회 스트레스 테스트 통과 | memory_grow_stress 테스트 |
| 3회 연속 Panic → quarantine 동작 | quarantine 단위/통합 테스트 |
| 동일 입력 + 동일 Fuel → 동일 결과 | fuel_determinism 테스트 |
| ComponentRegistry round-trip | registry 단위 테스트 |
| bevy_ecs 타입이 plugin_runtime에 미노출 | Cargo.toml + 컴파일 검증 |
| Phase 0 기존 테스트 전부 통과 (호환성) | cargo test --workspace |

## 검증 방법

```bash
# WASM 플러그인 빌드
cd plugins/test_movement && cargo build --target wasm32-unknown-unknown --release

# 전체 빌드
cargo build --workspace

# 전체 테스트
cargo test --workspace

# 개별 crate 테스트
cargo test -p plugin_abi
cargo test -p plugin_runtime
cargo test -p engine_core

# 통합 테스트 (WASM 포함)
cargo test --test wasm_plugin_test -- --nocapture
cargo test --test fuel_determinism -- --nocapture
cargo test --test memory_grow_stress -- --nocapture
```

## 리스크 및 완화 전략

| 리스크 | 완화 |
|--------|------|
| wasmtime API 변경 | wasmtime 버전 pin + plugin_runtime 내부에 격리 |
| grow() 이후 UB | WasmMemoryView가 매번 base ptr 재획득, stress test |
| Fuel 비결정론 | Fuel 한도를 불변 상수로 취급, 설정과 함께 Replay에 기록 |
| postcard 직렬화 병목 | Phase 1에서 프로파일링, Phase 3에서 FlatBuffers 검토 |
| Guest 측 메모리 할당 | 간단한 bump allocator 제공, 복잡한 alloc은 Phase 2 |
| ComponentRegistry 타입 안전성 | 등록 시 컴파일 타임 타입 체크, 런타임 ComponentId 매칭 |

## Phase 0 → Phase 1 수정 범위

| 파일 | 변경 내용 |
|------|-----------|
| `Cargo.toml` (workspace) | plugin_abi, plugin_runtime를 members에 추가 |
| `engine_core/Cargo.toml` | plugin_runtime 의존성 추가 (optional feature) |
| `engine_core/src/tick.rs` | TickLoop에 PluginRuntime 필드, step()에 WASM 실행 단계 |
| `observability/src/lib.rs` | TickMetrics에 wasm_duration_us 필드 추가 |
| 기타 기존 파일 | **수정 없음** (backward compatible) |
