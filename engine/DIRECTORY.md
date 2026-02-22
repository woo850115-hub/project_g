# engine/ 디렉토리 구조

MUD와 2D Grid 프로젝트가 공유하는 엔진 crate 10개.
게임별 스키마를 모르는 범용 엔진 계층으로, 게임 로직은 Lua 스크립트와 동적 레지스트리로 처리.

```
engine/crates/
├── ecs_adapter/                        # ECS 백엔드 격리 (bevy_ecs 래핑)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # 공개 API — EcsAdapter, EntityId 재export
│       ├── types.rs                    # EntityId (generation + index), u64 변환
│       ├── allocator.rs                # EntityAllocator — 세대 기반 ID 할당/해제
│       ├── bevy_backend.rs             # EcsAdapter — bevy_ecs World 래핑 (spawn/despawn/get/set/query)
│       └── error.rs                    # ECS 에러 타입
├── engine_core/                        # 엔진 코어 — 틱 루프, 명령 스트림, 이벤트 버스
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # 공개 API 재export
│       ├── tick.rs                     # TickLoop<S: SpaceModel> — 제네릭 시뮬레이션 루프
│       ├── command.rs                  # CommandStream (LWW) — WASM 명령 수집/적용
│       └── events.rs                   # EventBus — 타입별 이벤트 큐
├── space/                              # 공간 모델 — SpaceModel trait + 구현체 2종
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # 공개 API 재export
│       ├── model.rs                    # SpaceModel trait — 공용 인터페이스 (entity_room/move/place/remove)
│       ├── room_graph.rs               # RoomGraphSpace — 방+출구 기반 공간 (MUD용)
│       ├── grid_space.rs               # GridSpace — 정수 좌표 기반 2D 공간 (Grid용, BTreeMap 결정론)
│       └── snapshot.rs                 # SpaceSnapshotData enum + SpaceSnapshotCapture trait (다형성 스냅샷)
├── observability/                      # 로깅 초기화
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # init_logging() — tracing-subscriber 설정
├── plugin_abi/                         # WASM ABI 공유 타입 (no_std)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # WasmCommand, ABI 버전 상수, postcard 직렬화
├── plugin_runtime/                     # WASM 플러그인 런타임 (wasmtime)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # 공개 API 재export
│       ├── plugin.rs                   # WasmPlugin — 개별 플러그인 인스턴스 관리
│       ├── registry.rs                 # PluginRegistry — 플러그인 등록/실행/격리
│       ├── host_api.rs                 # 호스트 함수 — WASM에서 호출하는 Rust 함수
│       ├── memory.rs                   # WASM 메모리 접근 헬퍼
│       ├── serializer.rs              # postcard 기반 명령 직렬화/역직렬화
│       ├── config.rs                   # 플러그인 매니페스트, 우선순위 정렬
│       └── error.rs                    # 런타임 에러 타입
├── session/                            # 세션 관리 (엔진 레이어, 게임 비의존)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # SessionId, SessionOutput, SessionManager, PlayerSession,
│                                       # SessionState (로그인 상태머신), LingeringEntity, PermissionLevel
├── scripting/                          # Lua 스크립팅 엔진 (mlua/Luau 기반)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # 공개 API 재export
│       ├── engine.rs                   # ScriptEngine — Lua VM 관리, ScriptContext, run_on_* 메서드
│       ├── sandbox.rs                  # Lua 샌드박스 (메모리 16MB, 명령어 1M 제한)
│       ├── hooks.rs                    # HookRegistry — on_init/on_tick/on_action/on_enter_room/on_connect/on_admin
│       ├── component_registry.rs       # ScriptComponentRegistry — Lua table ↔ Rust Component 변환
│       ├── content.rs                  # ContentRegistry — JSON 콘텐츠 로드 (content/*.json)
│       ├── template.rs                 # 게임 템플릿 로더 (game.toml + scripts/ 자동 발견)
│       ├── error.rs                    # 스크립팅 에러 타입
│       └── api/                        # Lua API 모듈별 분리
│           ├── mod.rs                  # API 모듈 등록 총괄
│           ├── ecs.rs                  # ecs:get/set/has/remove/spawn/despawn/query
│           ├── space.rs                # space:* — 공용 SpaceModel + RoomGraph 전용 + Grid 전용
│           ├── session.rs              # sessions:session_for/playing_list
│           ├── output.rs               # output:send/broadcast_room
│           └── log.rs                  # log.info/warn/error/debug
├── persistence/                        # 영속성 — 스냅샷 캡처/복원, 디스크 I/O
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # 공개 API 재export
│       ├── registry.rs                # PersistenceRegistry — trait-object 기반 컴포넌트 등록
│       ├── snapshot.rs                 # capture/restore<S: SpaceSnapshotCapture> — ECS+Space 스냅샷
│       ├── manager.rs                  # SnapshotManager — 디스크 저장/로드/latest 관리
│       └── error.rs                    # 영속성 에러 타입
└── net/                                # 네트워크 — Telnet, WebSocket, 웹 서버
    ├── Cargo.toml
    └── src/
        ├── lib.rs                      # 공개 모듈 재export
        ├── channels.rs                 # 채널 타입 정의 (NetToTick, PlayerTx, OutputTx, RegisterTx 등)
        ├── output_router.rs            # 세션별 출력 라우팅 (OutputRx → 세션 write 채널)
        ├── server.rs                   # TCP 서버 — 접속 수락, 세션별 reader/writer 태스크
        ├── web_server.rs               # axum 웹 서버 — WebSocket 업그레이드 + 정적 파일 서빙
        ├── ws_server.rs                # WebSocket 메시지 핸들러 (JSON 프로토콜 파싱)
        ├── protocol.rs                 # JSON 프로토콜 타입 (ClientMessage, ServerMessage, StateDelta)
        ├── telnet.rs                   # Telnet LineBuffer — IAC 시퀀스 제거, 줄 단위 파싱
        ├── ansi.rs                     # ANSI 색상 상수 + strip_ansi() + colorize()
        ├── gmcp.rs                     # GMCP 패키지 (Char.Vitals, Room.Info, Telnet 서브네고시에이션)
        └── rate_limiter.rs             # 접속/명령어 제한 (ConnectionLimiter, CommandThrottle)
```

## Crate 의존 관계

```
engine_core → ecs_adapter, space, observability, plugin_abi, plugin_runtime
plugin_runtime → plugin_abi, ecs_adapter
scripting → ecs_adapter, space, session
session → ecs_adapter
persistence → ecs_adapter, space
net → session
space → ecs_adapter
observability → (독립)
plugin_abi → (독립, no_std)
ecs_adapter → bevy_ecs (내부만, 외부 노출 금지)
```
