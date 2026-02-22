# MUD 게임 콘텐츠 개발 가이드

## 목차

1. [개요](#1-개요)
2. [아키텍처 이해](#2-아키텍처-이해)
3. [스크립트 시스템](#3-스크립트-시스템)
4. [ECS 컴포넌트](#4-ecs-컴포넌트)
5. [Lua API 레퍼런스](#5-lua-api-레퍼런스)
6. [훅(Hook) 시스템](#6-훅hook-시스템)
7. [JSON 콘텐츠 시스템](#7-json-콘텐츠-시스템)
8. [실전 가이드: 새 콘텐츠 추가](#8-실전-가이드-새-콘텐츠-추가)
9. [명령어 시스템](#9-명령어-시스템)
10. [전투 시스템](#10-전투-시스템)
11. [관리자 시스템](#11-관리자-시스템)
12. [ANSI 색상](#12-ansi-색상)
13. [샌드박스 제약사항](#13-샌드박스-제약사항)
14. [디버깅과 테스트](#14-디버깅과-테스트)
15. [베스트 프랙티스](#15-베스트-프랙티스)

---

## 1. 개요

MUD 게임의 모든 콘텐츠(월드, 명령어, 전투, NPC, 아이템 등)는 **Lua 스크립트**와 **JSON 데이터 파일**로 작성됩니다. Rust 코드를 수정할 필요 없이 스크립트만으로 게임 로직을 구현할 수 있습니다.

### 파일 위치

```
project_mud/
├── scripts/          # Lua 게임 스크립트 (번호순 로드)
│   ├── 00_utils.lua
│   ├── 01_world_setup.lua
│   ├── 02_commands.lua
│   ├── 03_combat.lua
│   └── 04_admin.lua
└── content/          # JSON 콘텐츠 데이터 (선택사항)
    ├── monsters.json
    ├── items.json
    └── skills/       # 하위 디렉토리도 지원
        ├── warrior.json
        └── mage.json
```

### 실행 흐름

```
서버 시작
  → content/*.json 로드 → Lua content 글로벌 테이블에 등록
  → scripts/*.lua 파일명 정렬순 로드 (훅 등록)
  → on_init 훅 실행 (월드 생성)
  → 틱 루프 시작
    → 매 틱: 플레이어 입력 → on_action 훅 → on_tick 훅 → 출력 전송
```

---

## 2. 아키텍처 이해

### 엔티티-컴포넌트 시스템 (ECS)

모든 게임 오브젝트(방, 플레이어, NPC, 아이템)는 **엔티티**입니다. 엔티티는 숫자 ID(u64)이며, **컴포넌트**를 붙여서 속성을 부여합니다.

```lua
-- 고블린 만들기
local goblin = ecs:spawn()                              -- 빈 엔티티 생성
ecs:set(goblin, "Name", "고블린")                        -- 이름 부여
ecs:set(goblin, "Health", {current = 30, max = 30})     -- 체력 부여
ecs:set(goblin, "Attack", 8)                            -- 공격력 부여
ecs:set(goblin, "NpcTag", true)                         -- NPC 태그 부여
space:place_entity(goblin, some_room)                   -- 방에 배치
```

### 공간 모델 (RoomGraphSpace)

MUD 월드는 **방(Room)**과 **출구(Exit)**로 구성된 그래프입니다.

- 방도 엔티티입니다 (Name, Description 컴포넌트를 가짐)
- 출구는 방향(north/south/east/west) 또는 커스텀 이름으로 연결
- 엔티티는 하나의 방에 존재하며, `space:move_entity()`로 이동

```
[시작의 방] --east--> [시장 광장] --east--> [어두운 골목]
                          |
                        south
                          |
                          v
                      [무기 상점] --south--> [던전 입구] --east--> [던전 1층]
```

### 틱(Tick) 기반 시뮬레이션

서버는 초당 10회(기본값) 고정 주기로 게임 상태를 갱신합니다.

```
틱 1개 = 100ms (기본)
  1. 플레이어 입력 수신 → on_action 훅 호출
  2. 엔진 틱 (WASM 플러그인)
  3. on_tick 훅 호출 (전투 해결, 상태 갱신 등)
  4. 출력 메시지 전송
```

---

## 3. 스크립트 시스템

### 스크립트 로드 순서

`scripts/` 디렉토리의 `*.lua` 파일은 **파일명 알파벳순**으로 로드됩니다. 번호 접두사를 붙여 순서를 제어하세요.

```
00_utils.lua       ← 공용 함수/상수 (가장 먼저 로드)
01_world_setup.lua ← 월드 생성 (on_init)
02_commands.lua    ← 명령어 처리 (on_action)
03_combat.lua      ← 전투 시스템 (on_tick)
04_admin.lua       ← 관리자 명령어 (on_admin)
05_quests.lua      ← 퀘스트 시스템 (새로 추가 가능)
```

### 전역 변수

먼저 로드된 스크립트의 전역 변수/함수는 이후 스크립트에서 사용 가능합니다.

```lua
-- 00_utils.lua
function get_name(eid)
    return ecs:get(eid, "Name") or "누군가"
end

-- 02_commands.lua (get_name 사용 가능)
hooks.on_action("look", function(ctx)
    local name = get_name(ctx.entity)
    -- ...
end)
```

### 스냅샷 복원 주의

서버 재시작 시 스냅샷이 있으면 ECS 상태가 복원됩니다. `on_init`에서 중복 생성을 방지하세요.

```lua
hooks.on_init(function()
    -- 이미 월드가 존재하면 건너뜀
    if space:room_count() > 0 then
        log.info("월드가 이미 존재합니다, 생성 건너뜀")
        return
    end
    -- 월드 생성 로직...
end)
```

---

## 4. ECS 컴포넌트

### 등록된 컴포넌트 목록

| 컴포넌트 | Lua 타입 | 설명 | 예시 |
|----------|---------|------|------|
| `Name` | string | 이름 | `"고블린"` |
| `Description` | string | 설명 텍스트 | `"으르렁거리는 고블린"` |
| `Health` | table | 체력 | `{current=30, max=30}` |
| `Attack` | number | 공격력 | `8` |
| `Defense` | number | 방어력 | `2` |
| `Inventory` | table | 소지품 (엔티티 ID 배열) | `{items={4294967296, ...}}` |
| `PlayerTag` | boolean | 플레이어 태그 | `true` |
| `NpcTag` | boolean | NPC 태그 | `true` |
| `ItemTag` | boolean | 아이템 태그 | `true` |
| `Dead` | boolean | 사망 태그 | `true` |
| `CombatTarget` | number | 전투 대상 엔티티 ID | `4294967296` |
| `InRoom` | number | 현재 방 엔티티 ID | `4294967296` |

### 데이터 컴포넌트 vs 태그 컴포넌트

**데이터 컴포넌트** (Name, Health, Attack 등):
```lua
-- 값을 가짐
ecs:set(eid, "Name", "고블린")
ecs:set(eid, "Health", {current = 30, max = 30})
ecs:set(eid, "Attack", 8)

local name = ecs:get(eid, "Name")           -- "고블린"
local hp = ecs:get(eid, "Health")            -- {current=30, max=30}
local atk = ecs:get(eid, "Attack")           -- 8
```

**태그 컴포넌트** (PlayerTag, NpcTag, ItemTag, Dead):
```lua
-- true로 설정 (값 없는 마커)
ecs:set(eid, "NpcTag", true)
ecs:set(eid, "Dead", true)

local is_npc = ecs:has(eid, "NpcTag")        -- true/false
local is_dead = ecs:has(eid, "Dead")          -- true/false

-- 제거할 때는 ecs:remove 사용 (false 설정 불가)
ecs:remove(eid, "Dead")
```

**엔티티 참조 컴포넌트** (CombatTarget, InRoom, Inventory):
```lua
-- 엔티티 ID(숫자)를 값으로 가짐
ecs:set(attacker, "CombatTarget", target_entity_id)

local target = ecs:get(attacker, "CombatTarget")  -- 엔티티 ID (number)

-- Inventory는 엔티티 ID 배열
ecs:set(player, "Inventory", {items = {item1_id, item2_id}})
local inv = ecs:get(player, "Inventory")           -- {items={...}}
```

---

## 5. Lua API 레퍼런스

### ecs (ECS 접근)

```lua
-- 엔티티 생성/삭제
local eid = ecs:spawn()             -- 새 엔티티 생성, ID 반환
ecs:despawn(eid)                    -- 엔티티 삭제 (모든 컴포넌트 제거)

-- 컴포넌트 조작
ecs:set(eid, "Name", "고블린")       -- 컴포넌트 설정 (없으면 추가, 있으면 덮어쓰기)
local val = ecs:get(eid, "Name")    -- 컴포넌트 읽기 (없으면 nil)
local has = ecs:has(eid, "Name")    -- 컴포넌트 존재 여부 (true/false)
ecs:remove(eid, "Dead")             -- 컴포넌트 제거

-- 쿼리
local entities = ecs:query("Health")  -- 해당 컴포넌트를 가진 모든 엔티티 ID 배열
```

### space (공간 조작)

```lua
-- === 공용 SpaceModel 메서드 (MUD/Grid 양쪽) ===
local room = space:entity_room(eid)              -- 엔티티가 있는 방 ID (없으면 nil)
space:move_entity(eid, target_room)              -- 인접한 방으로 이동 (출구 필요)
space:place_entity(eid, room)                    -- 엔티티를 방에 배치 (처음 또는 텔레포트)
space:remove_entity(eid)                         -- 엔티티를 공간에서 제거

-- === RoomGraph 전용 (MUD 모드) ===
space:register_room(room_eid, exits_table)       -- 방 등록 + 출구 설정
space:room_occupants(room)                       -- 방에 있는 모든 엔티티 ID 배열
space:exits(room)                                -- 출구 테이블 {north=room_id, ...}
space:room_exists(room)                          -- 방 존재 여부 (true/false)
space:room_count()                               -- 등록된 방 총 수
space:all_rooms()                                -- 모든 방 ID 배열
```

#### 방 등록 예시

```lua
local room_a = ecs:spawn()
local room_b = ecs:spawn()

ecs:set(room_a, "Name", "마을 입구")
ecs:set(room_a, "Description", "평화로운 마을의 입구입니다.")

ecs:set(room_b, "Name", "마을 광장")
ecs:set(room_b, "Description", "시끌벅적한 광장입니다.")

-- 양방향 출구 설정
space:register_room(room_a, {east = room_b})
space:register_room(room_b, {west = room_a})
```

#### 커스텀 출구

표준 방향(north/south/east/west) 외에 커스텀 출구도 가능합니다.

```lua
space:register_room(dungeon, {
    north = corridor,
    up = surface,          -- 커스텀 방향
    enter = secret_room,   -- 커스텀 방향
})
```

> 커스텀 출구를 사용하려면 `parser.rs`에 해당 명령어를 추가하거나, `on_action("unknown")`에서 처리해야 합니다.

### output (플레이어 출력)

```lua
-- 특정 세션에 텍스트 전송
output:send(session_id, "환영합니다!")

-- 방 전체에 브로드캐스트 (특정 엔티티 제외 가능)
output:broadcast_room(room_id, "큰 소리가 들린다!", {exclude = sender_eid})
```

### sessions (세션 조회)

```lua
-- 엔티티에 연결된 세션 ID 조회 (플레이어만 세션이 있음)
local sid = sessions:session_for(entity_id)   -- 있으면 session_id, 없으면 nil

-- 접속 중인 플레이어 목록
local list = sessions:playing_list()
-- 결과: [{session_id=0, entity=12345, name="홍길동"}, ...]
for _, info in ipairs(list) do
    output:send(info.session_id, "공지: 서버 점검 예정")
end
```

### log (서버 로그)

```lua
log.info("월드 생성 완료")
log.warn("알 수 없는 아이템 ID: " .. tostring(item_id))
log.error("치명적 오류 발생")
log.debug("디버그: entity=" .. tostring(eid))
```

### content (JSON 콘텐츠)

```lua
-- 콘텐츠 컬렉션의 모든 항목
local all_monsters = content.all("monsters")   -- [{id="goblin", name="고블린", ...}, ...]

-- 특정 ID로 조회
local goblin = content.get("monsters", "goblin")
-- {id="goblin", name="고블린", hp=30, attack=8, defense=2}

if goblin then
    local npc = ecs:spawn()
    ecs:set(npc, "Name", goblin.name)
    ecs:set(npc, "Health", {current = goblin.hp, max = goblin.hp})
    ecs:set(npc, "Attack", goblin.attack)
end
```

---

## 6. 훅(Hook) 시스템

훅은 게임 이벤트에 반응하는 콜백 함수입니다.

### hooks.on_init(fn)

서버 시작 시 1회 호출. 월드 초기 생성에 사용.

```lua
hooks.on_init(function()
    log.info("서버 초기화 시작")
    -- 방 생성, NPC 배치, 아이템 배치 등
end)
```

### hooks.on_tick(fn)

매 틱(기본 100ms)마다 호출. 틱 번호가 인자로 전달.

```lua
hooks.on_tick(function(tick)
    -- 전투 해결
    -- NPC AI
    -- 상태 효과 처리
    -- 리스폰 체크
end)
```

### hooks.on_action(action_name, fn)

플레이어 명령어 처리. `true` 반환 시 명령어 소비 (다른 핸들러 호출 안 됨).

```lua
hooks.on_action("look", function(ctx)
    -- ctx.entity     : 명령을 입력한 플레이어 엔티티 ID
    -- ctx.session_id : 플레이어 세션 ID
    -- ctx.args       : 명령어 인자 (문자열)
    output:send(ctx.session_id, "주변을 둘러봅니다.")
    return true   -- 명령어 처리 완료
end)
```

**사용 가능한 action_name:**

| action | 트리거 명령어 | ctx.args |
|--------|-------------|----------|
| `look` | 보기, ㅂ, look, l, (빈 입력) | (없음) |
| `move` | 북, 남, 동, 서, north, south, east, west | 방향 (`"north"` 등) |
| `attack` | 공격, attack, kill, k | 대상 이름 |
| `get` | 줍기, get, take, pick | 아이템 이름 |
| `drop` | 버리기, drop | 아이템 이름 |
| `inventory` | 가방, 인벤, inventory, inv, i | (없음) |
| `say` | 말, say | 메시지 텍스트 |
| `who` | 접속자, who | (없음) |
| `help` | 도움말, ?, help | (없음) |
| `unknown` | (인식 못한 입력) | 원본 입력 텍스트 |

### hooks.on_enter_room(fn)

엔티티가 방에 입장할 때 호출.

```lua
hooks.on_enter_room(function(entity, room, old_room)
    -- entity   : 입장한 엔티티 ID
    -- room     : 새 방 ID
    -- old_room : 이전 방 ID (nil일 수 있음)

    -- 예: 던전 입장 시 경고 메시지
    local room_name = ecs:get(room, "Name")
    if room_name == "던전 1층" then
        local sid = sessions:session_for(entity)
        if sid then
            output:send(sid, colors.red .. "경고: 위험한 지역입니다!" .. colors.reset)
        end
    end
end)
```

> `hooks.fire_enter_room(entity, room, old_room)`으로 Lua에서 직접 트리거할 수도 있습니다.

### hooks.on_connect(fn)

새 플레이어가 접속할 때 호출.

```lua
hooks.on_connect(function(session_id)
    output:send(session_id, "MUD 서버에 오신 것을 환영합니다!")
end)
```

### hooks.on_admin(command, min_permission, fn)

관리자 명령어 (`/` 접두사). 권한 검증은 Rust에서 자동 수행.

```lua
-- min_permission: 0=Player, 1=Builder, 2=Admin, 3=Owner
hooks.on_admin("spawn_npc", 2, function(ctx)
    -- ctx.session_id : 관리자 세션 ID
    -- ctx.entity     : 관리자 엔티티 ID
    -- ctx.args       : 명령어 인자
    -- ctx.permission : 실제 권한 레벨 (number)
    output:send(ctx.session_id, "NPC를 생성했습니다.")
    return true
end)
-- 사용: /spawn_npc 고블린
```

---

## 7. JSON 콘텐츠 시스템

반복되는 게임 데이터(몬스터/아이템/스킬 정의 등)를 JSON 파일로 관리할 수 있습니다.

### 디렉토리 구조

```
project_mud/content/
├── monsters.json       # 배열 형태 — 컬렉션명: "monsters"
├── items.json          # 배열 형태 — 컬렉션명: "items"
└── skills/             # 디렉토리 형태 — 컬렉션명: "skills"
    ├── fireball.json   # 단일 오브젝트
    └── heal.json       # 단일 오브젝트
```

### 배열 파일 형식

파일명이 컬렉션명이 됩니다. 각 항목에 `"id"` 필드 필수.

```json
// content/monsters.json
[
  {
    "id": "goblin",
    "name": "고블린",
    "description": "으르렁거리는 작은 고블린",
    "hp": 30,
    "attack": 8,
    "defense": 2,
    "loot": ["rusty_dagger"]
  },
  {
    "id": "skeleton",
    "name": "해골 전사",
    "description": "덜거덕거리는 해골이 낡은 검을 들고 있다",
    "hp": 50,
    "attack": 12,
    "defense": 5,
    "loot": ["old_sword", "bone"]
  }
]
```

### 디렉토리 형식

디렉토리명이 컬렉션명이 됩니다. 각 JSON 파일은 단일 오브젝트이며 `"id"` 필드 필수.

```json
// content/skills/fireball.json
{
  "id": "fireball",
  "name": "파이어볼",
  "damage": 25,
  "mp_cost": 10,
  "description": "불의 구슬을 발사합니다"
}
```

### Lua에서 콘텐츠 사용

```lua
hooks.on_init(function()
    -- 모든 몬스터 정의를 순회하며 월드에 배치
    local all_monsters = content.all("monsters")
    if all_monsters then
        for _, def in ipairs(all_monsters) do
            log.info("몬스터 정의 로드: " .. def.id)
        end
    end

    -- 특정 몬스터 생성
    local goblin_def = content.get("monsters", "goblin")
    if goblin_def then
        local goblin = ecs:spawn()
        ecs:set(goblin, "Name", goblin_def.name)
        ecs:set(goblin, "Description", goblin_def.description)
        ecs:set(goblin, "NpcTag", true)
        ecs:set(goblin, "Health", {current = goblin_def.hp, max = goblin_def.hp})
        ecs:set(goblin, "Attack", goblin_def.attack)
        ecs:set(goblin, "Defense", goblin_def.defense)
        space:place_entity(goblin, dungeon_room)
    end
end)
```

---

## 8. 실전 가이드: 새 콘텐츠 추가

### 8.1 새로운 방(Room) 추가

```lua
-- 01_world_setup.lua 또는 별도 스크립트에서

hooks.on_init(function()
    if space:room_count() > 0 then return end  -- 중복 방지

    local tavern = ecs:spawn()
    ecs:set(tavern, "Name", "여관")
    ecs:set(tavern, "Description", "따뜻한 벽난로가 타오르는 아늑한 여관입니다. 맥주 냄새가 풍깁니다.")

    -- 기존 방과 연결
    -- 주의: 양방향 출구를 설정해야 돌아올 수 있습니다
    space:register_room(tavern, {south = market_square})

    -- 기존 방에도 출구 추가 필요 → register_room을 다시 호출하면 출구가 재설정됨
    -- 따라서 처음부터 모든 출구를 포함하여 등록하세요
end)
```

### 8.2 새로운 NPC 추가

```lua
hooks.on_init(function()
    if space:room_count() > 0 then return end

    -- 여관 주인
    local bartender = ecs:spawn()
    ecs:set(bartender, "Name", "여관 주인")
    ecs:set(bartender, "Description", "덩치 큰 남자가 잔을 닦고 있습니다.")
    ecs:set(bartender, "NpcTag", true)
    ecs:set(bartender, "Health", {current = 100, max = 100})
    ecs:set(bartender, "Attack", 15)
    ecs:set(bartender, "Defense", 10)
    space:place_entity(bartender, tavern)
end)
```

### 8.3 새로운 아이템 추가

```lua
hooks.on_init(function()
    if space:room_count() > 0 then return end

    -- 바닥에 놓인 아이템
    local sword = ecs:spawn()
    ecs:set(sword, "Name", "강철 검")
    ecs:set(sword, "Description", "날카로운 강철 검입니다.")
    ecs:set(sword, "ItemTag", true)
    space:place_entity(sword, weapon_shop)
end)
```

### 8.4 새로운 명령어 추가

**방법 1: Rust 파서에 등록된 action 사용**

기존 action (`look`, `move`, `attack` 등)에 대한 훅을 추가로 등록할 수 있습니다. 먼저 등록된 훅이 `true`를 반환하지 않으면 다음 훅이 호출됩니다.

**방법 2: unknown action에서 커스텀 명령어 처리**

Rust 파서를 수정하지 않고 새 명령어를 추가하는 방법:

```lua
-- 05_custom_commands.lua

hooks.on_action("unknown", function(ctx)
    local input = ctx.args:lower()

    -- "쉬기" 명령어
    if input == "쉬기" or input == "rest" then
        local hp = ecs:get(ctx.entity, "Health")
        if hp and hp.current < hp.max then
            hp.current = math.min(hp.current + 5, hp.max)
            ecs:set(ctx.entity, "Health", hp)
            output:send(ctx.session_id, "잠시 쉬면서 체력을 회복합니다. (HP: " .. hp.current .. "/" .. hp.max .. ")")
        else
            output:send(ctx.session_id, "체력이 이미 가득 찼습니다.")
        end
        return true
    end

    -- "조사 <대상>" 명령어
    if input:sub(1, 2) == "조사" or input:sub(1, 7) == "examine" then
        local target_name = input:gsub("^조사%s*", ""):gsub("^examine%s*", "")
        if target_name == "" then
            output:send(ctx.session_id, "무엇을 조사할까요?")
            return true
        end

        local room = space:entity_room(ctx.entity)
        if room then
            local occupants = space:room_occupants(room)
            for _, occ in ipairs(occupants) do
                local name = ecs:get(occ, "Name")
                if name and string.find(name:lower(), target_name, 1, true) then
                    local desc = ecs:get(occ, "Description") or "특별한 것은 보이지 않습니다."
                    output:send(ctx.session_id, colors.cyan .. name .. colors.reset .. ": " .. desc)
                    return true
                end
            end
        end
        output:send(ctx.session_id, "'" .. target_name .. "'을(를) 찾을 수 없습니다.")
        return true
    end

    -- 처리하지 못한 명령어는 false 반환 (또는 기본 메시지 출력)
    return false
end)
```

**방법 3: Rust 파서에 새 명령어 등록 (권장)**

빈번하게 사용되는 명령어는 `parser.rs`에 직접 추가하는 것이 좋습니다:

```rust
// project_mud/crates/mud/src/parser.rs
pub enum PlayerAction {
    // ... 기존 항목 ...
    Rest,                    // 새 명령어
    Examine(String),         // 새 명령어
}

// parse_input() 함수 내:
match cmd {
    // ... 기존 매칭 ...
    "rest" | "쉬기" => PlayerAction::Rest,
    "examine" | "조사" => {
        if arg.is_empty() {
            PlayerAction::Unknown("무엇을 조사할까요?".to_string())
        } else {
            PlayerAction::Examine(arg)
        }
    }
    _ => PlayerAction::Unknown(trimmed.to_string()),
}
```

그리고 `main.rs`에서 PlayerAction → on_action 매핑 추가 필요.

### 8.5 NPC AI (on_tick 활용)

```lua
-- 06_npc_ai.lua

-- NPC 배회 시스템 (50틱마다 랜덤 이동)
hooks.on_tick(function(tick)
    if tick % 50 ~= 0 then return end  -- 50틱마다 실행 (5초)

    local npcs = ecs:query("NpcTag")
    for _, npc in ipairs(npcs) do
        -- 죽은 NPC는 건너뜀
        if ecs:has(npc, "Dead") then goto continue end

        -- 전투 중인 NPC는 건너뜀
        if ecs:has(npc, "CombatTarget") then goto continue end

        local room = space:entity_room(npc)
        if not room then goto continue end

        local exits = space:exits(room)
        if not exits then goto continue end

        -- 출구 목록 수집
        local dirs = {}
        for dir, _ in pairs(exits) do
            table.insert(dirs, dir)
        end
        table.sort(dirs)  -- 결정론적 순서

        -- 30% 확률로 이동 (틱 기반 의사 난수)
        if #dirs > 0 and (tick * 7 + npc) % 10 < 3 then
            local idx = (tick * 13 + npc) % #dirs + 1
            local dir = dirs[idx]
            local target = exits[dir]

            local npc_name = get_name(npc)
            broadcast_room(room, npc_name .. "이(가) 어딘가로 떠났습니다.")

            space:move_entity(npc, target)

            broadcast_room(target, npc_name .. "이(가) 나타났습니다.", npc)
        end

        ::continue::
    end
end)
```

> **주의: 결정론적 난수** — `math.random()` 대신 `tick` 값을 기반으로 의사 난수를 생성하세요.
> 동일 입력 + 동일 틱에서 동일한 결과를 보장해야 합니다.

### 8.6 NPC 리스폰 시스템

```lua
-- 07_respawn.lua

-- 리스폰 정보를 전역 테이블에 저장
RESPAWN_QUEUE = {}

-- NPC 사망 시 리스폰 예약
hooks.on_tick(function(tick)
    -- 사망 처리: Dead 태그가 붙은 NPC를 리스폰 큐에 추가
    local dead_npcs = ecs:query("Dead")
    for _, npc in ipairs(dead_npcs) do
        if ecs:has(npc, "NpcTag") and not RESPAWN_QUEUE[npc] then
            RESPAWN_QUEUE[npc] = {
                tick = tick + 300,    -- 30초 후 리스폰 (TPS 10 기준)
                room = space:entity_room(npc),
            }
        end
    end

    -- 리스폰 큐 확인
    local to_respawn = {}
    for npc, info in pairs(RESPAWN_QUEUE) do
        if tick >= info.tick then
            table.insert(to_respawn, {npc = npc, room = info.room})
        end
    end

    -- 정렬 (결정론 보장)
    table.sort(to_respawn, function(a, b) return a.npc < b.npc end)

    for _, entry in ipairs(to_respawn) do
        local npc = entry.npc
        -- 부활
        ecs:remove(npc, "Dead")
        local hp = ecs:get(npc, "Health")
        if hp then
            ecs:set(npc, "Health", {current = hp.max, max = hp.max})
        end

        -- 원래 방으로 이동
        if entry.room and space:room_exists(entry.room) then
            local current = space:entity_room(npc)
            if current ~= entry.room then
                space:remove_entity(npc)
                space:place_entity(npc, entry.room)
            end
            local name = get_name(npc)
            broadcast_room(entry.room, name .. "이(가) 다시 나타났습니다!")
        end

        RESPAWN_QUEUE[npc] = nil
    end
end)
```

### 8.7 간단한 퀘스트 시스템

```lua
-- 08_quests.lua

-- 퀘스트 상태를 플레이어 엔티티에 저장하는 예시
-- 주의: 커스텀 컴포넌트는 Rust에 등록해야 영속성이 보장됩니다.
-- 간단한 방법: 전역 테이블에 저장 (서버 재시작 시 초기화됨)

QUEST_STATE = {}  -- [entity_id] = {quest_id = status, ...}

-- NPC와 대화로 퀘스트 수락
hooks.on_action("unknown", function(ctx)
    local input = ctx.args:lower()

    if input == "대화" or input == "talk" then
        local room = space:entity_room(ctx.entity)
        if not room then return false end

        local occupants = space:room_occupants(room)
        for _, occ in ipairs(occupants) do
            if ecs:has(occ, "NpcTag") and not ecs:has(occ, "Dead") then
                local name = get_name(occ)

                -- 고블린 퇴치 퀘스트 예시
                if name == "여관 주인" then
                    local state = QUEST_STATE[ctx.entity] or {}

                    if not state["goblin_hunt"] then
                        state["goblin_hunt"] = "accepted"
                        QUEST_STATE[ctx.entity] = state
                        output:send(ctx.session_id, colors.yellow .. "[퀘스트 수락] 고블린 퇴치" .. colors.reset)
                        output:send(ctx.session_id, name .. ": \"던전에 고블린이 나타났어! 처치해주게!\"")
                    elseif state["goblin_hunt"] == "accepted" then
                        -- 고블린이 죽었는지 확인
                        local goblins = ecs:query("NpcTag")
                        local goblin_dead = false
                        for _, g in ipairs(goblins) do
                            if get_name(g) == "고블린" and ecs:has(g, "Dead") then
                                goblin_dead = true
                                break
                            end
                        end

                        if goblin_dead then
                            state["goblin_hunt"] = "completed"
                            QUEST_STATE[ctx.entity] = state
                            output:send(ctx.session_id, colors.green .. "[퀘스트 완료] 고블린 퇴치" .. colors.reset)
                            output:send(ctx.session_id, name .. ": \"잘 해냈어! 보상으로 물약을 주지.\"")
                            -- 보상 아이템 생성
                            local reward = ecs:spawn()
                            ecs:set(reward, "Name", "고급 치유 물약")
                            ecs:set(reward, "ItemTag", true)
                            local inv = ecs:get(ctx.entity, "Inventory") or {items = {}}
                            table.insert(inv.items, reward)
                            ecs:set(ctx.entity, "Inventory", inv)
                        else
                            output:send(ctx.session_id, name .. ": \"던전의 고블린을 아직 처치하지 못했군.\"")
                        end
                    else
                        output:send(ctx.session_id, name .. ": \"고마워, 모험가여!\"")
                    end
                    return true
                end
            end
        end

        output:send(ctx.session_id, "대화할 상대가 없습니다.")
        return true
    end

    return false
end)
```

---

## 9. 명령어 시스템

### 입력 파싱 흐름

```
플레이어 입력 "공격 고블린"
  → Rust 파서 (parser.rs): PlayerAction::Attack("고블린")
  → main.rs: action 이름 "attack", args "고블린"으로 변환
  → Lua: hooks.on_action("attack", fn) 호출
    → ctx = {entity=..., session_id=..., args="고블린"}
```

### 현재 등록된 명령어 (parser.rs)

| 한글 | 영문 | 약어 | PlayerAction |
|------|------|------|-------------|
| 보기 | look | l, ㅂ | Look |
| 북 | north | n | Move(North) |
| 남 | south | s | Move(South) |
| 동 | east | e | Move(East) |
| 서 | west | w | Move(West) |
| 공격 | attack, kill | k | Attack(target) |
| 줍기 | get, take, pick | | Get(item) |
| 버리기 | drop | | Drop(item) |
| 가방, 인벤 | inventory | inv, i | InventoryList |
| 말 | say | | Say(msg) |
| 접속자 | who | | Who |
| 도움말 | help | ? | Help |
| 종료 | quit, exit | | Quit |
| /{명령} | | | Admin |
| (기타) | | | Unknown(input) |

### 도움말 업데이트

새 명령어를 추가하면 `HELP_TEXT` (00_utils.lua)도 갱신하세요:

```lua
HELP_TEXT = [[사용 가능한 명령어:
  보기 (ㅂ)           - 주변을 둘러봅니다
  북/남/동/서          - 해당 방향으로 이동
  공격 <대상>         - 대상을 공격합니다
  줍기 <아이템>       - 아이템을 줍습니다
  버리기 <아이템>     - 아이템을 버립니다
  가방 (인벤)         - 소지품을 확인합니다
  조사 <대상>         - 대상을 자세히 살펴봅니다
  쉬기               - 체력을 회복합니다
  대화               - 주변 NPC와 대화합니다
  말 <내용>           - 말을 합니다
  접속자              - 접속 중인 플레이어 목록
  도움말 (?)          - 이 도움말을 표시합니다
  종료                - 접속을 종료합니다]]
```

---

## 10. 전투 시스템

### 현재 전투 흐름

```
1. 플레이어: "공격 고블린"
2. on_action("attack"): CombatTarget 컴포넌트 설정
3. 매 틱 on_tick:
   a. CombatTarget이 있는 모든 엔티티 수집
   b. 같은 방에 있는지, 대상이 살아있는지 확인
   c. 데미지 = max(공격력 - 방어력, 1)
   d. 대상 HP 감소
   e. HP <= 0 이면 Dead 태그 부여
   f. 전투 메시지 출력 (공격자/피격자/방 관전자)
```

### 전투 확장 예시

```lua
-- 크리티컬 히트 추가 (틱 기반 결정론적)
hooks.on_tick(function(tick)
    local combatants = ecs:query("CombatTarget")

    for _, attacker in ipairs(combatants) do
        if ecs:has(attacker, "Dead") then goto continue end

        local target = ecs:get(attacker, "CombatTarget")
        if not target then goto continue end

        local atk_stat = ecs:get(attacker, "Attack") or 5
        local def_stat = ecs:get(target, "Defense") or 0

        -- 크리티컬 (틱 기반 의사 난수)
        local is_crit = ((tick * 31 + attacker * 17) % 100) < 15  -- 15% 확률
        local damage = math.max(atk_stat - def_stat, 1)
        if is_crit then
            damage = damage * 2
        end

        -- 데미지 적용...
        ::continue::
    end
end)
```

---

## 11. 관리자 시스템

### 권한 레벨

| 레벨 | 이름 | 설명 |
|------|------|------|
| 0 | Player | 일반 플레이어 |
| 1 | Builder | 월드 빌더 (서버 통계 조회 등) |
| 2 | Admin | 관리자 (추방, 공지, 텔레포트 등) |
| 3 | Owner | 서버 소유자 (모든 권한) |

### 관리자 명령어 추가 예시

```lua
-- /heal <플레이어> — 체력 회복 (Admin+)
hooks.on_admin("heal", 2, function(ctx)
    local target_name = ctx.args
    if target_name == "" then
        output:send(ctx.session_id, "사용법: /heal <플레이어이름>")
        return true
    end

    local playing = sessions:playing_list()
    for _, info in ipairs(playing) do
        local name = ecs:get(info.entity, "Name")
        if name and name:lower() == target_name:lower() then
            local hp = ecs:get(info.entity, "Health")
            if hp then
                ecs:set(info.entity, "Health", {current = hp.max, max = hp.max})
                output:send(info.session_id, "관리자에 의해 체력이 회복되었습니다.")
                output:send(ctx.session_id, name .. "의 체력을 회복시켰습니다.")
            end
            return true
        end
    end

    output:send(ctx.session_id, target_name .. "을(를) 찾을 수 없습니다.")
    return true
end)
```

---

## 12. ANSI 색상

Telnet 클라이언트에 표시되는 색상 코드입니다. `00_utils.lua`에 정의된 `colors` 테이블을 사용하세요.

### 사용 가능한 색상

```lua
colors.reset          -- 모든 서식 초기화
colors.bold           -- 굵게
colors.dim            -- 흐리게
colors.underline      -- 밑줄

-- 기본 색상
colors.black, colors.red, colors.green, colors.yellow
colors.blue, colors.magenta, colors.cyan, colors.white

-- 밝은 색상
colors.bright_red, colors.bright_green, colors.bright_yellow
colors.bright_blue, colors.bright_magenta, colors.bright_cyan, colors.bright_white
```

### 사용 예시

```lua
-- 빨간 굵은 텍스트
output:send(sid, colors.bold .. colors.red .. "위험!" .. colors.reset .. " 체력이 낮습니다.")

-- 초록색 메시지
output:send(sid, colors.green .. "퀘스트 완료!" .. colors.reset)

-- 여러 색상 조합
local msg = colors.cyan .. "[시스템]" .. colors.reset .. " "
    .. colors.yellow .. player_name .. colors.reset
    .. "님이 접속했습니다."
output:send(sid, msg)
```

### 현재 적용된 색상 규칙

| 용도 | 색상 |
|------|------|
| 방 이름 | bold + cyan |
| 출구 | green |
| 데미지 (공격자 시점) | yellow |
| 데미지 (피격자 시점) | red |
| 사망 메시지 | bold + red |

---

## 13. 샌드박스 제약사항

Lua 스크립트는 보안 샌드박스 내에서 실행됩니다.

### 제한사항

| 항목 | 제한 |
|------|------|
| 메모리 | 16 MB |
| 명령어 수 | 틱당 1,000,000 |
| 파일 접근 | 불가 (`io`, `os` 비활성) |
| 네트워크 | 불가 |
| `require` | 불가 (모든 스크립트는 자동 로드) |
| `loadfile` | 불가 |
| `dofile` | 불가 |

### 사용 가능한 표준 라이브러리

- `string` (string.find, string.format, string.lower, string.upper, string.sub, string.rep 등)
- `table` (table.insert, table.remove, table.sort, table.concat 등)
- `math` (math.max, math.min, math.floor, math.abs 등)
- `tostring`, `tonumber`, `type`, `pairs`, `ipairs`, `pcall`, `error`

### 주의: 결정론

- `math.random()` 사용 금지 — 틱 번호 기반 의사 난수 사용
- `os.time()`, `os.clock()` 사용 불가
- 해시맵 순회 시 정렬 후 사용 (`table.sort`)

---

## 14. 디버깅과 테스트

### 서버 실행

```bash
cd /home/genos/workspace/project_g
export PATH="/home/genos/.cargo/bin:$PATH"

# MUD 서버 시작
cargo run -p project_mud -- --config project_mud/server.toml

# 클라이언트 접속 (별도 터미널)
telnet localhost 4000
```

### 로그 확인

Lua `log.*` 호출은 서버 콘솔에 출력됩니다:

```lua
log.info("디버그: 고블린 HP = " .. tostring(hp.current))
log.warn("경고: 방을 찾을 수 없음 room_id=" .. tostring(room_id))
```

서버 콘솔 출력 예:
```
INFO scripting: 디버그: 고블린 HP = 22
WARN scripting: 경고: 방을 찾을 수 없음 room_id=nil
```

### pcall로 에러 안전 처리

```lua
local ok, err = pcall(function()
    space:move_entity(entity, target_room)
end)
if not ok then
    output:send(session_id, "이동 불가: " .. tostring(err))
end
```

### 통합 테스트

```bash
# 전체 테스트
cargo test --workspace

# 게임 시스템 통합 테스트
cargo test --test game_systems_integration -- --nocapture

# 스크립팅 관련 테스트
cargo test -p scripting -- --nocapture

# 콘텐츠 레지스트리 테스트
cargo test --test content_registry_test -- --nocapture
```

---

## 15. 베스트 프랙티스

### 스크립트 구조화

- **번호 접두사**로 로드 순서 보장 (00, 01, 02, ...)
- **공용 함수**는 `00_utils.lua`에 정의
- **한 파일 = 한 시스템** (전투, 퀘스트, 경제 등)
- 파일이 커지면 분리 (예: `05_quests_goblin.lua`, `05_quests_dragon.lua`)

### 결정론 보장

동일한 입력에 항상 동일한 결과가 나와야 합니다.

```lua
-- 나쁜 예 (비결정적)
local random_room = rooms[math.random(#rooms)]

-- 좋은 예 (결정론적)
local idx = (tick * 7 + entity_id) % #rooms + 1
local room = rooms[idx]
```

```lua
-- 나쁜 예 (비결정적 순회)
for key, value in pairs(some_table) do
    -- pairs()의 순회 순서는 보장되지 않음
end

-- 좋은 예 (정렬 후 순회)
local keys = {}
for k, _ in pairs(some_table) do
    table.insert(keys, k)
end
table.sort(keys)
for _, k in ipairs(keys) do
    local v = some_table[k]
    -- ...
end
```

### 스냅샷 안전

- `on_init`에서 항상 중복 생성 체크
- 전역 변수에 저장한 데이터는 서버 재시작 시 사라짐 (스냅샷에 포함 안 됨)
- 영속성이 필요한 데이터는 ECS 컴포넌트에 저장

### 성능

- `on_tick`에서 매 틱 실행할 필요 없는 로직은 간격을 두세요:
  ```lua
  if tick % 100 == 0 then  -- 10초마다
      -- 무거운 로직
  end
  ```
- `ecs:query()`는 전체 엔티티를 순회하므로, 결과를 캐시하거나 호출 빈도를 줄이세요
- 명령어 제한: 틱당 최대 1,000,000 Lua 명령어

### 안전한 코딩

```lua
-- nil 체크 필수
local hp = ecs:get(entity, "Health")
if hp then  -- nil이 아닌지 확인
    hp.current = hp.current - damage
    ecs:set(entity, "Health", hp)
end

-- 방 존재 확인
local room = space:entity_room(entity)
if not room then
    output:send(session_id, "현재 위치를 알 수 없습니다.")
    return true
end

-- 세션 확인 (NPC는 세션이 없음)
local sid = sessions:session_for(entity)
if sid then
    output:send(sid, "메시지")
end
```

---

## 부록: 파일 템플릿

### 새 시스템 스크립트 템플릿

```lua
-- XX_system_name.lua: 시스템 설명

-- 전역 상태 (서버 재시작 시 초기화됨)
local SYSTEM_STATE = {}

-- 초기화 (필요시)
hooks.on_init(function()
    log.info("시스템 초기화 완료")
end)

-- 틱 처리 (필요시)
hooks.on_tick(function(tick)
    -- 주기적 처리
end)

-- 명령어 처리 (필요시)
hooks.on_action("action_name", function(ctx)
    -- ctx.entity, ctx.session_id, ctx.args
    return true  -- 명령어 소비
end)

-- 방 입장 이벤트 (필요시)
hooks.on_enter_room(function(entity, room, old_room)
    -- 트리거 처리
end)
```

### 새 JSON 콘텐츠 템플릿

```json
[
  {
    "id": "unique_id",
    "name": "표시 이름",
    "description": "설명 텍스트",
    "custom_field": 42
  }
]
```
