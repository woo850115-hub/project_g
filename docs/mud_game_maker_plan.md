# MUD Game Maker 설계 계획서

> 작성일: 2026-02-22
> 상태: 설계 단계

---

## 1. 왜 만드는가

### 1.1 현재 상황

project_mud 엔진은 Lua 스크립팅과 JSON 콘텐츠 시스템으로 코드 없이 게임을 만들 수 있는 구조를 이미 갖추고 있다. 하지만 실제로 콘텐츠를 제작하려면:

- Lua 문법을 알아야 한다
- ECS 개념을 이해해야 한다 (엔티티, 컴포넌트, 스폰)
- 방 연결 구조를 머릿속으로 그려야 한다 (텍스트만으로는 6개 방도 헷갈림)
- JSON 파일을 직접 편집해야 한다
- 테스트하려면 서버를 수동으로 시작하고 Telnet 클라이언트를 열어야 한다

즉, **엔진은 준비되었지만 도구가 없다.** 게임 디자이너가 아닌 프로그래머만 콘텐츠를 만들 수 있는 상태다.

### 1.2 해결하려는 문제

| 현재 | 목표 |
|------|------|
| Lua 코드를 직접 작성 | 폼 입력 + 비주얼 편집으로 80% 해결 |
| 방 구조를 텍스트로 관리 | 노드 그래프로 시각화 + 드래그앤드롭 |
| JSON 수동 편집 | 구조화된 폼 UI |
| 서버 수동 시작 + Telnet 접속 | "테스트 플레이" 버튼 한 번 |
| 에러를 터미널 로그에서 확인 | 에디터 내 실시간 로그 패널 |

### 1.3 목표 사용자

1. **게임 디자이너** — 코딩 경험 없음, 비주얼 도구로 방/NPC/아이템/이벤트 제작
2. **스크립터** — 기본 프로그래밍 가능, Lua로 커스텀 로직 작성
3. **엔진 개발자** — Rust 레벨에서 새 컴포넌트/시스템 추가 (기존 워크플로우 유지)

---

## 2. 왜 이렇게 설계하는가

### 2.1 기존 게임메이커 분석

역대 성공한 게임 제작 도구들의 공통 패턴을 분석했다.

#### RPG Maker (RPG 쯔꾸르) — 가장 성공한 게임메이커

- **3기둥 구조**: 맵 에디터 + 데이터베이스 + 이벤트 시스템
- **핵심 성공 요인**: 코딩 없이 완성된 게임을 만들 수 있음
- **스크립팅은 확장용**: Ruby/JS 스크립트는 "고급 사용자 탈출구"로만 존재
- **배울 점**: 데이터베이스 폼 UI가 콘텐츠 제작의 핵심. 몬스터 하나 만드는 데 코드 한 줄 필요 없음
- **MUD에 적용**: 타일맵 → 방 그래프, 이벤트 시스템 → 트리거 빌더로 변환

#### Twine — 텍스트 게임 제작 도구

- **노드 기반 스토리 그래프**가 핵심 UI
- 텍스트 기반 게임에서 "연결 구조를 시각화"하는 것의 가치를 증명
- **배울 점**: MUD의 방 연결 = Twine의 패시지 연결. 동일한 노드 그래프 패러다임이 작동
- **한계**: 단방향 스토리 전용이라 MUD의 양방향 이동, 동적 상태에는 부족

#### Roblox Studio — 가장 성공한 UGC 게임 플랫폼

- **Lua 스크립팅** 채택 (우리 엔진과 동일한 언어)
- **에디터 안에서 즉시 플레이** (만들면서 바로 테스트)
- **배울 점**: "테스트 플레이" 즉시성이 제작 효율에 결정적. 수정 → 확인 사이클이 짧을수록 좋음
- **한계**: 3D 전용이라 UI 복잡도가 높음. MUD는 훨씬 단순한 UI로 같은 효과를 낼 수 있음

#### Evennia (Python MUD 엔진) — 가장 유사한 프로젝트

- 웹 어드민 패널 제공하지만 **비주얼 맵 에디터가 없음**
- 명령줄 기반 방 생성 (`@dig`, `@create`)
- **배울 점**: 웹 어드민만으로는 부족. "방 30개짜리 던전"을 텍스트 명령으로 만드는 것은 고통
- **우리의 차별점**: 비주얼 맵 에디터를 추가하면 Evennia 대비 명확한 UX 우위

#### 분석 결론

```
성공 공식 = 비주얼 편집(80%) + 스크립팅 탈출구(20%) + 즉시 테스트
```

모든 성공한 게임메이커가 이 공식을 따른다. RPG Maker, GameMaker, Roblox, Unity 모두 동일.

### 2.2 왜 별도 프로젝트(project_maker/)인가

**대안 1: project_mud에 웹 UI 통합** — 기각

- MUD 서버에 에디터 기능을 추가하면 프로덕션 게임 서버에 편집 API가 노출됨
- 보안 문제: 운영 중인 서버에서 파일 쓰기 API가 열려있으면 위험
- 관심사 혼재: 게임 서버(플레이어 서빙)와 에디터 서버(개발자 도구)는 목적이 다름
- MUD 서버가 꺼져있어도 에디터는 사용할 수 있어야 함

**대안 2: 완전 독립 앱 (Electron 등)** — 기각

- 별도 설치가 필요해 진입장벽이 높아짐
- 크로스 플랫폼 빌드/배포 부담
- 브라우저만 있으면 되는 웹 앱이 접근성 최고

**채택: 별도 project_maker/ 웹 서비스**

- 엔진 crate를 의존성으로 공유하되 독립 바이너리
- `project_mud/` 디렉토리를 직접 읽고 쓰는 "개발 도구"
- 개발 환경에서만 실행하는 용도 (프로덕션에 배포하지 않음)
- MUD 서버 프로세스를 자식 프로세스로 시작/중지 (테스트 플레이)

### 2.3 왜 React + TypeScript인가

**대안 1: Vue** — 학습 곡선은 낮지만

- React Flow (노드 그래프 라이브러리)가 React 전용
- Vue에는 동급 품질의 노드 에디터 라이브러리가 부족
- 맵 에디터가 핵심 기능이므로 노드 에디터 품질이 프로젝트 성패를 좌우

**대안 2: Svelte** — 가볍지만

- Monaco Editor, React Flow 같은 복잡한 컴포넌트의 Svelte 바인딩이 미성숙
- 생태계 규모에서 React 대비 불리

**채택: React + TypeScript**

- **React Flow**: 방 그래프 에디터의 핵심. 노드 드래그앤드롭, 엣지 연결, 미니맵, 줌/팬 내장
- **Monaco Editor**: VS Code와 동일한 코드 편집 경험. Lua 구문 강조 + 자동완성 지원
- **React Hook Form**: 콘텐츠 데이터베이스 폼 빌더에 적합
- **TypeScript**: 복잡한 에디터 UI에서 타입 안전성 필수

### 2.4 왜 파일 시스템 기반인가 (별도 DB 없음)

**대안: SQLite/PostgreSQL에 콘텐츠 저장** — 기각

- 이미 `content/*.json`과 `scripts/*.lua`로 잘 동작하는 시스템이 있음
- DB를 추가하면 "DB ↔ 파일 동기화" 문제가 발생
- Git으로 버전 관리가 가능하다는 파일 기반의 장점을 잃음
- MUD 서버는 파일을 직접 읽으므로, Maker도 같은 파일을 직접 편집하는 것이 가장 단순

**채택: 직접 파일 읽기/쓰기**

```
Maker가 편집 → project_mud/content/monsters.json에 직접 저장
                project_mud/scripts/01_world_setup.lua에 직접 저장
MUD 서버가 실행 → 같은 파일을 그대로 읽음
```

중간 계층이 없으므로 동기화 문제가 원천적으로 없다.

### 2.5 왜 이 Phase 순서인가

```
Phase 1: 콘텐츠 DB  → 가장 쉽고, 즉시 쓸 수 있는 가치를 제공
Phase 2: 맵 에디터  → 가장 큰 차별점, 핵심 기능
Phase 3: 테스트     → "만들면서 바로 확인"이 가능해지는 전환점
Phase 4: 스크립트   → 고급 사용자용, Monaco 통합
Phase 5: 트리거     → 코딩 없이 이벤트를 만드는 비주얼 시스템
```

Phase 1~3까지가 **MVP**. 이 시점에서 "비주얼 맵 + 콘텐츠 폼 + 즉시 테스트"가 가능해지며, 이것만으로도 기존 MUD 제작 도구 대비 명확한 가치가 있다.

Phase 4~5는 사용성 향상. 특히 Phase 5 트리거 시스템이 완성되면 "코딩 없이 게임 만들기"가 실현된다.

---

## 3. 전체 아키텍처

### 3.1 시스템 구조도

```
┌─────────────────────────────────────────────────┐
│                    브라우저                       │
│                                                   │
│  ┌─────────┐ ┌──────────┐ ┌────────┐ ┌────────┐ │
│  │맵 에디터│ │콘텐츠 DB │ │스크립트│ │프리뷰  │ │
│  │(React   │ │(폼 CRUD) │ │(Monaco)│ │(MUD    │ │
│  │ Flow)   │ │          │ │        │ │클라이언│ │
│  │         │ │          │ │        │ │트)     │ │
│  └────┬────┘ └────┬─────┘ └───┬────┘ └───┬────┘ │
│       │           │           │           │       │
│       └───────────┴───────────┴───────────┘       │
│                       │ HTTP / WebSocket           │
└───────────────────────┼───────────────────────────┘
                        │
┌───────────────────────┼───────────────────────────┐
│              project_maker 서버 (axum)             │
│                       │                            │
│  ┌────────────────────┼────────────────────────┐  │
│  │              REST API 레이어                 │  │
│  │                                              │  │
│  │  /api/content/*    콘텐츠 CRUD               │  │
│  │  /api/world/*      방 그래프 CRUD            │  │
│  │  /api/scripts/*    스크립트 편집             │  │
│  │  /api/server/*     MUD 서버 시작/중지        │  │
│  │  /ws/preview       테스트 플레이 중계        │  │
│  │  /ws/logs          실시간 로그 스트리밍      │  │
│  └─────────────────────────────────────────────┘  │
│                       │                            │
│              파일 시스템 직접 접근                  │
└───────────────────────┼───────────────────────────┘
                        │
                        ▼
┌───────────────────────────────────────────────────┐
│              project_mud/ 디렉토리                 │
│                                                    │
│  content/           scripts/           server.toml │
│  ├── monsters.json  ├── 00_utils.lua               │
│  ├── items.json     ├── 01_world_setup.lua         │
│  └── skills/        ├── 02_commands.lua            │
│      └── ...        └── ...                        │
└───────────────────────────────────────────────────┘
                        │
                        │ Maker가 "테스트 플레이" 시
                        │ 자식 프로세스로 시작
                        ▼
┌───────────────────────────────────────────────────┐
│              project_mud 서버 (MUD 서버)           │
│              telnet :4000                          │
└───────────────────────────────────────────────────┘
```

### 3.2 데이터 흐름

```
[맵 에디터에서 방 추가]
  → PUT /api/world/rooms
  → 서버: 방 데이터를 메모리 WorldGraph에 반영
  → 서버: project_mud/scripts/01_world_setup.lua를 재생성
  → 응답: 200 OK

[콘텐츠 DB에서 몬스터 수정]
  → PUT /api/content/monsters/goblin
  → 서버: project_mud/content/monsters.json 업데이트
  → 응답: 200 OK

[테스트 플레이 버튼 클릭]
  → POST /api/server/start
  → 서버: cargo run -p project_mud -- --config ... 를 자식 프로세스로 실행
  → 응답: {status: "started", port: 4000}
  → 프론트: WebSocket MUD 클라이언트를 4000 포트에 연결
```

---

## 4. 프로젝트 구조

```
project_maker/
├── Cargo.toml                       # axum + serde + tokio
├── server.toml                      # Maker 설정 (포트, project_mud 경로)
├── src/
│   ├── main.rs                      # axum 서버 진입점
│   ├── config.rs                    # 설정 파싱
│   ├── state.rs                     # AppState (공유 상태)
│   ├── api/
│   │   ├── mod.rs                   # 라우터 조립
│   │   ├── content.rs               # GET/PUT/DELETE /api/content/*
│   │   ├── world.rs                 # GET/PUT/DELETE /api/world/*
│   │   ├── scripts.rs               # GET/PUT /api/scripts/*
│   │   └── server.rs                # POST /api/server/start|stop|status
│   ├── world/
│   │   ├── mod.rs
│   │   ├── graph.rs                 # WorldGraph — 방/출구 인메모리 표현
│   │   ├── entity_placement.rs      # NPC/아이템 배치 정보
│   │   └── lua_generator.rs         # WorldGraph → Lua 스크립트 생성
│   └── process/
│       └── mud_server.rs            # MUD 서버 프로세스 관리
├── web_client/
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── index.html
│   └── src/
│       ├── main.tsx                 # React 진입점
│       ├── App.tsx                  # 탭 레이아웃
│       ├── api/
│       │   └── client.ts            # REST/WS 클라이언트
│       ├── pages/
│       │   ├── MapEditor/
│       │   │   ├── MapEditor.tsx    # React Flow 기반 메인 뷰
│       │   │   ├── RoomNode.tsx     # 커스텀 노드 컴포넌트
│       │   │   ├── ExitEdge.tsx     # 커스텀 엣지 컴포넌트
│       │   │   └── RoomPanel.tsx    # 방 속성 편집 사이드 패널
│       │   ├── Database/
│       │   │   ├── Database.tsx     # 컬렉션 목록 + 편집 뷰
│       │   │   ├── CollectionList.tsx
│       │   │   ├── ItemForm.tsx     # 동적 폼 (JSON 스키마 기반)
│       │   │   └── SchemaEditor.tsx # 컬렉션 스키마 정의
│       │   ├── ScriptEditor/
│       │   │   ├── ScriptEditor.tsx # Monaco 래퍼
│       │   │   └── LuaCompletion.ts # Lua API 자동완성 정의
│       │   └── Preview/
│       │       ├── Preview.tsx      # 테스트 플레이 화면
│       │       ├── MudTerminal.tsx  # 웹 기반 MUD 클라이언트
│       │       └── LogViewer.tsx    # 실시간 서버 로그
│       ├── components/
│       │   ├── Layout.tsx           # 상단 탭 + 사이드바 레이아웃
│       │   ├── ConfirmDialog.tsx
│       │   └── Toast.tsx
│       └── types/
│           ├── world.ts             # Room, Exit, Entity 타입
│           ├── content.ts           # Collection, Item 타입
│           └── api.ts               # API 요청/응답 타입
└── tests/
    └── api_test.rs                  # API 통합 테스트
```

---

## 5. API 설계

### 5.1 콘텐츠 API

```
GET    /api/content
  → 200: ["monsters", "items", "skills"]   # 컬렉션 목록

GET    /api/content/:collection
  → 200: [
      {id: "goblin", name: "고블린", hp: 30, ...},
      {id: "skeleton", name: "해골 전사", hp: 50, ...}
    ]

GET    /api/content/:collection/:id
  → 200: {id: "goblin", name: "고블린", hp: 30, attack: 8, defense: 2}

PUT    /api/content/:collection/:id
  Body: {id: "goblin", name: "고블린", hp: 50, attack: 10, defense: 3}
  → 200: {ok: true}
  → 파일: project_mud/content/monsters.json 업데이트

DELETE /api/content/:collection/:id
  → 200: {ok: true}

POST   /api/content/:collection
  Body: {id: "new_collection_name"}
  → 201: {ok: true}
  → 파일: project_mud/content/new_collection_name.json 생성 (빈 배열)
```

### 5.2 월드(맵) API

맵 에디터의 데이터는 별도 JSON 파일(`project_mud/world.json`)로 관리한다.
이 파일은 방 위치(에디터용 좌표), 출구 연결, 배치된 엔티티 정보를 포함한다.
"저장" 시 이 데이터에서 `01_world_setup.lua`를 자동 생성한다.

```
GET    /api/world
  → 200: {
      rooms: [
        {
          id: "room_1",
          name: "시작의 방",
          description: "따뜻한 방입니다...",
          position: {x: 0, y: 0},            # 에디터 캔버스 좌표
          exits: {east: "room_2"},
          entities: [
            {type: "npc", content_id: "goblin", override: {name: "늙은 고블린"}},
            {type: "item", content_id: "potion"}
          ]
        },
        ...
      ]
    }

PUT    /api/world/rooms/:id
  Body: {
    name: "새로운 방",
    description: "설명...",
    position: {x: 200, y: 100},
    exits: {north: "room_1", south: "room_3"}
  }
  → 200: {ok: true}

DELETE /api/world/rooms/:id
  → 200: {ok: true}
  → 연결된 출구도 자동 정리

PUT    /api/world/rooms/:id/entities
  Body: [
    {type: "npc", content_id: "goblin"},
    {type: "item", content_id: "potion"}
  ]
  → 200: {ok: true}

POST   /api/world/generate
  → Lua 스크립트 재생성 (world.json → 01_world_setup.lua)
  → 200: {ok: true, path: "scripts/01_world_setup.lua"}
```

#### world.json → Lua 변환 예시

```json
// project_mud/world.json (Maker가 관리)
{
  "rooms": [
    {
      "id": "spawn",
      "name": "시작의 방",
      "description": "따뜻하고 환한 방입니다.",
      "position": {"x": 0, "y": 0},
      "exits": {"east": "market"},
      "entities": []
    },
    {
      "id": "market",
      "name": "시장 광장",
      "description": "활기찬 시장입니다.",
      "position": {"x": 250, "y": 0},
      "exits": {"west": "spawn", "south": "weapon_shop"},
      "entities": [
        {"type": "item", "content_id": "potion"}
      ]
    }
  ]
}
```

자동 생성되는 Lua:

```lua
-- 01_world_setup.lua (자동 생성 — 직접 수정하지 마세요)
-- Generated by MUD Game Maker

hooks.on_init(function()
    if space:room_count() > 0 then
        log.info("World already loaded from snapshot, skipping creation")
        return
    end

    log.info("Creating world...")

    -- Rooms
    local spawn = ecs:spawn()
    ecs:set(spawn, "Name", "시작의 방")
    ecs:set(spawn, "Description", "따뜻하고 환한 방입니다.")

    local market = ecs:spawn()
    ecs:set(market, "Name", "시장 광장")
    ecs:set(market, "Description", "활기찬 시장입니다.")

    -- Exits
    space:register_room(spawn, {east = market})
    space:register_room(market, {west = spawn, south = weapon_shop})

    -- Entities: market
    local market_entity_1 = ecs:spawn()
    ecs:set(market_entity_1, "Name", "치유 물약")
    ecs:set(market_entity_1, "ItemTag", true)
    space:place_entity(market_entity_1, market)

    log.info("World created: 2 rooms, 1 entity")
end)
```

### 5.3 스크립트 API

```
GET    /api/scripts
  → 200: [
      {filename: "00_utils.lua", size: 4280, modified: "2026-02-22T..."},
      {filename: "02_commands.lua", size: 8120, modified: "2026-02-22T..."},
      ...
    ]
  → 01_world_setup.lua는 목록에서 제외 (Maker가 자동 생성하므로)

GET    /api/scripts/:filename
  → 200: {filename: "02_commands.lua", content: "-- 02_commands.lua: ..."}

PUT    /api/scripts/:filename
  Body: {content: "-- 수정된 스크립트 내용..."}
  → 200: {ok: true}

POST   /api/scripts
  Body: {filename: "05_quests.lua", content: "-- 퀘스트 시스템\n"}
  → 201: {ok: true}

DELETE /api/scripts/:filename
  → 200: {ok: true}
```

### 5.4 서버 관리 API

```
GET    /api/server/status
  → 200: {running: false}
  → 200: {running: true, pid: 12345, uptime: 120}

POST   /api/server/start
  → MUD 서버를 자식 프로세스로 시작
  → 200: {ok: true, pid: 12345, telnet_port: 4000}

POST   /api/server/stop
  → SIGTERM 전송 → graceful shutdown
  → 200: {ok: true}

POST   /api/server/restart
  → stop + start
  → 200: {ok: true, pid: 12346}

WebSocket /ws/logs
  → MUD 서버 stdout/stderr를 실시간 스트리밍
  → 메시지: {level: "INFO", target: "scripting", message: "World created..."}

WebSocket /ws/preview
  → Telnet 프록시 (브라우저 WS ↔ MUD Telnet 4000 중계)
  → 클라이언트 → 서버: {type: "input", text: "북"}
  → 서버 → 클라이언트: {type: "output", text: "== 시장 광장 ==\n..."}
```

---

## 6. UI 화면 설계

### 6.1 메인 레이아웃

```
┌──────────────────────────────────────────────────────────────┐
│  MUD Game Maker     [맵] [데이터베이스] [스크립트] [테스트]   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                    (활성 탭의 콘텐츠 영역)                    │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│  상태바: 서버 ● 중지됨  |  방 6개  |  저장됨 ✓              │
└──────────────────────────────────────────────────────────────┘
```

### 6.2 맵 에디터 탭

```
┌──────────────────────────────────────────────────────────────┐
│  [+ 방 추가]  [자동 정렬]  [저장]           줌: [−] 100% [+] │
├────────────────────────────────┬─────────────────────────────┤
│                                │  방 속성                     │
│     ┌──────────┐               │                             │
│     │ 시작의 방 │──east──┐     │  이름: [시장 광장        ]  │
│     └──────────┘        │     │  설명:                       │
│                    ┌────┴───┐ │  [활기찬 시장 광장입니다.  ] │
│                    │시장 광장│ │  [상인들이 물건을 팔고    ] │
│                    └──┬──┬──┘ │  [있습니다.               ] │
│                       │  │    │                             │
│              south────┘  │    │  출구:                       │
│                          │    │  ├ 동 → 어두운 골목          │
│                 ┌────────┘    │  ├ 남 → 무기 상점            │
│                 │  east       │  └ 서 → 시작의 방            │
│           ┌─────┴────┐        │                             │
│           │어두운 골목│        │  배치된 엔티티:              │
│           └──────────┘        │  ┌─────────────────────┐    │
│           ┌──────────┐        │  │ 🧪 치유 물약 (아이템)│    │
│           │ 무기 상점 │        │  └─────────────────────┘    │
│           └────┬─────┘        │  [+ NPC 추가] [+ 아이템]    │
│                │south         │                             │
│           ┌────┴─────┐        │                             │
│           │ 던전 입구 │        │                             │
│           └──────────┘        │                             │
│                                │                             │
│  ┌─────┐ 미니맵               │                             │
│  │ · · │                       │                             │
│  │ · · │                       │                             │
│  └─────┘                       │                             │
├────────────────────────────────┴─────────────────────────────┤
│  [Lua 미리보기]  생성될 Lua 코드를 미리 확인                  │
└──────────────────────────────────────────────────────────────┘
```

### 6.3 데이터베이스 탭

```
┌──────────────────────────────────────────────────────────────┐
│  컬렉션: [monsters ▼]  [+ 새 컬렉션]                         │
├──────────────────┬───────────────────────────────────────────┤
│  목록             │  편집: 고블린                              │
│                  │                                           │
│  ┌────────────┐  │  ID:     [goblin          ]  (수정 불가)  │
│  │● 고블린    │  │  이름:   [고블린           ]              │
│  │  해골 전사 │  │  설명:   [으르렁거리는... ]               │
│  │  오크 전사 │  │                                           │
│  │  드래곤    │  │  ── 스탯 ──                               │
│  └────────────┘  │  HP:     [30    ]                         │
│                  │  공격력: [8     ]                         │
│  [+ 추가]        │  방어력: [2     ]                         │
│                  │                                           │
│                  │  ── 드롭 ──                               │
│                  │  [rusty_dagger    ] [×]                   │
│                  │  [+ 드롭 아이템 추가]                      │
│                  │                                           │
│                  │  [저장]  [삭제]  [JSON 보기]               │
└──────────────────┴───────────────────────────────────────────┘
```

### 6.4 스크립트 에디터 탭

```
┌──────────────────────────────────────────────────────────────┐
│  파일: [02_commands.lua ▼]  [+ 새 파일]  [저장 Ctrl+S]       │
├──────────────────────────────────────────────────────────────┤
│  1  -- 02_commands.lua: 명령어 처리                          │
│  2                                                           │
│  3  hooks.on_action("look", function(ctx)                    │
│  4      local room = space:entity_room(ctx.entity)           │
│  5      if not room then                                     │
│  6          output:send(ctx.session_id, "위치를 알 수 없음") │
│  7          return true                                      │
│  8      end                                                  │
│  9      output:send(ctx.session_id, format_room(room, ctx.e  │
│ 10      return true                           ┌────────────┐ │
│ 11  end)                                      │ecs:get     │ │
│ 12                                            │ecs:set     │ │
│ 13  hooks.on_action("move", function(ctx)     │ecs:has     │ │
│ 14      -- ...                                │ecs:remove  │ │
│ 15                                            │ecs:spawn   │ │
│ 16                                            │ecs:despawn │ │
│ 17                                            │ecs:query   │ │
│                                               └────────────┘ │
│                                                자동완성       │
├──────────────────────────────────────────────────────────────┤
│  API 레퍼런스:  [ecs] [space] [output] [sessions] [hooks]    │
│  ecs:get(entity, component) → value or nil                   │
│  ecs:set(entity, component, value) → void                    │
└──────────────────────────────────────────────────────────────┘
```

### 6.5 테스트 플레이 탭

```
┌──────────────────────────────────────────────────────────────┐
│  서버: ● 실행 중 (PID 12345, 45초)   [재시작]  [중지]        │
├──────────────────────────────┬───────────────────────────────┤
│  MUD 클라이언트               │  서버 로그                    │
│                              │                               │
│  == 시작의 방 ==             │  INFO  World created: 6 rooms │
│  따뜻하고 환한 방입니다.     │  INFO  New connection: sid=0  │
│  벽에 안내문이 붙어 있습니다 │  DEBUG on_action: look        │
│  출구: 동                    │  INFO  Player moved east      │
│                              │  DEBUG on_enter_room: market   │
│  > 동                        │                               │
│                              │                               │
│  == 시장 광장 ==             │                               │
│  활기찬 시장 광장입니다.     │                               │
│  출구: 동, 남, 서            │                               │
│  주위에: [치유 물약]         │                               │
│                              │                               │
│  > 줍기 물약                 │                               │
│  치유 물약을(를) 주웠습니다. │                               │
│                              │                               │
│  > _                         │                               │
│                              │                               │
├──────────────────────────────┴───────────────────────────────┤
│  입력: [                                          ] [전송]   │
└──────────────────────────────────────────────────────────────┘
```

---

## 7. 기술 스택

### 7.1 백엔드 (Rust)

| 라이브러리 | 용도 |
|-----------|------|
| axum 0.8 | HTTP/WebSocket 서버 |
| tower-http 0.6 | 정적 파일 서빙, CORS |
| serde + serde_json | JSON 직렬화 |
| tokio | 비동기 런타임, 프로세스 관리 |
| toml | 설정 파싱 |
| tracing | 로깅 |

새로운 의존성 최소화 — 이미 workspace에 있는 crate 대부분 재사용.

### 7.2 프론트엔드 (TypeScript)

| 라이브러리 | 용도 | 선택 이유 |
|-----------|------|----------|
| React 19 | UI 프레임워크 | React Flow 필수 |
| TypeScript 5 | 타입 안전성 | 복잡한 에디터 UI에 필수 |
| Vite 6 | 빌드 도구 | 이미 project_2d에서 사용 중 |
| React Flow | 방 그래프 노드 에디터 | 유일하게 성숙한 React 노드 에디터 |
| Monaco Editor | Lua 코드 편집기 | VS Code 동일 엔진 |
| React Hook Form | 폼 관리 | 콘텐츠 DB 폼에 적합 |
| TailwindCSS | 스타일링 | 빠른 UI 개발 |
| xterm.js | MUD 터미널 에뮬레이터 | 테스트 플레이 화면 |

---

## 8. 구현 계획

### Phase 1: 프로젝트 뼈대 + 콘텐츠 DB

가장 쉽고 즉시 가치를 제공하는 기능부터 시작.

**백엔드:**
- [ ] project_maker/ Cargo.toml 생성 (workspace member 추가)
- [ ] axum 서버 기본 틀 (config, 정적 파일 서빙)
- [ ] 콘텐츠 API 구현 (파일 기반 CRUD)
  - GET/PUT/DELETE `/api/content/:collection/:id`
  - `project_mud/content/` 디렉토리 직접 읽기/쓰기

**프론트엔드:**
- [ ] React + Vite + TypeScript 프로젝트 셋업
- [ ] TailwindCSS 설정
- [ ] 탭 레이아웃 구조 (맵/DB/스크립트/테스트)
- [ ] 데이터베이스 탭 구현
  - 컬렉션 목록 (좌측)
  - 아이템 목록 (좌측 하단)
  - 편집 폼 (우측) — JSON 구조에서 자동 생성
  - 새 컬렉션/아이템 추가, 삭제

**검증:**
- content/ 디렉토리가 없으면 자동 생성
- 기존 JSON 파일 정상 로드 확인
- 수정 → 파일 저장 → MUD 서버에서 정상 로드 확인

### Phase 2: 맵 에디터

핵심 기능. 이것이 완성되어야 "게임메이커"라 부를 수 있음.

**백엔드:**
- [ ] world.json 파일 포맷 정의
- [ ] 월드 API 구현 (방 CRUD, 출구 연결)
- [ ] Lua 생성기 (world.json → 01_world_setup.lua)
  - 방 생성 코드 생성
  - 출구 연결 코드 생성
  - 엔티티 배치 코드 생성 (content DB 참조)

**프론트엔드:**
- [ ] React Flow 통합
- [ ] 커스텀 RoomNode 컴포넌트
  - 방 이름 표시, 선택 시 하이라이트
  - NPC/아이템 아이콘 뱃지
- [ ] 커스텀 ExitEdge 컴포넌트
  - 방향 라벨 표시 (북/남/동/서)
  - 양방향 자동 연결 옵션
- [ ] 방 속성 편집 사이드 패널
  - 이름, 설명 편집
  - 출구 목록 (추가/삭제)
  - 배치된 엔티티 목록 (콘텐츠 DB에서 선택)
- [ ] 방 추가: 캔버스 빈 공간 더블클릭 또는 "+" 버튼
- [ ] 출구 연결: 노드 핸들 드래그로 두 방 연결
- [ ] "Lua 미리보기" 패널 (생성될 코드 확인)
- [ ] 자동 정렬 기능 (dagre 레이아웃 알고리즘)

**기존 world → world.json 마이그레이션:**
- 기존 `01_world_setup.lua`가 있으면 파싱하여 `world.json`으로 초기 변환하는 유틸리티 제공
- 또는 수동으로 Maker에서 다시 만들기 (방 6개라 빠름)

### Phase 3: 테스트 플레이

"만들면서 바로 확인"의 핵심.

**백엔드:**
- [ ] MUD 서버 프로세스 관리
  - `tokio::process::Command`로 `cargo run -p project_mud` 실행
  - stdout/stderr 캡처 → WebSocket 로그 스트리밍
  - SIGTERM으로 graceful shutdown
- [ ] Telnet 프록시 WebSocket
  - 브라우저 WebSocket ↔ MUD Telnet 포트 중계
  - 또는: MUD 서버에 WebSocket 엔드포인트 추가 (향후)

**프론트엔드:**
- [ ] 테스트 탭 구현
  - 서버 시작/중지/재시작 버튼
  - 서버 상태 표시 (PID, 가동 시간)
- [ ] MUD 터미널 (xterm.js)
  - Telnet 프록시 WebSocket 연결
  - 입력 → 서버, 출력 ← 서버
  - ANSI 색상 지원
- [ ] 로그 뷰어
  - 실시간 서버 로그 스트리밍
  - 레벨 필터 (INFO/WARN/ERROR)
  - 자동 스크롤

### Phase 4: 스크립트 에디터

고급 사용자를 위한 Lua 편집 환경.

**프론트엔드:**
- [ ] Monaco Editor 통합
  - Lua 구문 강조
  - 커스텀 자동완성 (ecs:get, space:move_entity, hooks.on_action 등)
  - API 시그니처 호버 도움말
- [ ] 스크립트 파일 목록 (01_world_setup.lua는 읽기 전용 표시)
- [ ] 새 파일 생성, 삭제
- [ ] Ctrl+S 저장
- [ ] 하단 API 레퍼런스 패널 (접이식)

**백엔드:**
- [ ] 스크립트 API 구현 (파일 읽기/쓰기)
- [ ] 01_world_setup.lua 보호 (Maker가 관리하므로 직접 수정 경고)

### Phase 5: 트리거 시스템 (비주얼 이벤트)

코딩 없이 게임 이벤트를 만드는 시스템. RPG Maker의 이벤트 시스템에 해당.

**설계:**
```
트리거 = 조건(WHEN) + 동작(THEN)

예시:
  WHEN: 플레이어가 [던전 1층]에 입장
  THEN: 메시지 표시 "위험한 지역입니다!"

  WHEN: [고블린] 사망
  THEN: 30초 후 [던전 1층]에서 리스폰

  WHEN: 플레이어가 [여관 주인]에게 "대화" 명령
  THEN: 대화 표시 "던전에 고블린이..."
       퀘스트 시작: "고블린 퇴치"

  WHEN: [고블린 퇴치] 퀘스트 완료 조건 달성
  THEN: 보상 아이템 지급: [고급 치유 물약]
```

**백엔드:**
- [ ] 트리거 데이터 모델 (triggers.json)
- [ ] 트리거 → Lua 코드 자동 생성
- [ ] 트리거 API (CRUD)

**프론트엔드:**
- [ ] 트리거 편집기 UI
  - 조건(WHEN) 드롭다운: 방 입장, NPC 사망, 명령어 입력, 틱 간격 등
  - 동작(THEN) 드롭다운: 메시지 표시, NPC 스폰, 아이템 지급, 이동 등
  - 파라미터 입력 (방 선택, NPC 선택, 텍스트 입력 등)
- [ ] 맵 에디터에서 방/NPC 선택 시 연결된 트리거 표시

---

## 9. Cargo.toml 의존성

```toml
[package]
name = "project_maker"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mud_maker"
path = "src/main.rs"

[dependencies]
axum = { workspace = true }
tower-http = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
toml = "0.8"
```

루트 `Cargo.toml`의 workspace members에 `"project_maker"` 추가.

---

## 10. 실행 방법

```bash
# Maker 서버 시작
cargo run -p project_maker -- --config project_maker/server.toml

# 브라우저에서 접속
# http://localhost:3000

# Maker가 관리하는 대상 프로젝트
# project_mud/ 디렉토리
```

### server.toml (Maker 설정)

```toml
[server]
addr = "0.0.0.0:3000"
web_static_dir = "project_maker/web_dist"

[project]
# Maker가 편집할 대상 MUD 프로젝트 경로
mud_dir = "project_mud"
mud_config = "project_mud/server.toml"
```

---

## 11. 향후 확장 가능성

이 계획서의 범위 밖이지만, 아키텍처가 허용하는 미래 기능:

- **멀티 프로젝트**: `mud_dir`을 바꿔서 여러 MUD 프로젝트 관리
- **버전 관리**: Git 통합 (커밋/되돌리기 UI)
- **에셋 관리**: 사운드, 이미지 (웹 MUD 클라이언트용)
- **협업**: 여러 빌더가 동시에 편집 (OT/CRDT)
- **마켓플레이스**: 콘텐츠 팩 공유/다운로드
- **project_2d 지원**: 동일 Maker로 Grid 게임도 편집 (탭 전환)
