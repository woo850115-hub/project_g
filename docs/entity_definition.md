# Project G — 엔티티 정의서

> 작성일: 2026-02-20
> 개정일: 2026-02-20
> 상태: 초안 v2

## 개요

이 문서는 게임에 등장하는 모든 엔티티의 구조와 관계를 정의한다.
엔티티는 데이터가 존재하는 계층에 따라 세 가지로 구분된다:

| 계층 | 기호 | 저장소 | 설명 |
|------|------|--------|------|
| 콘텐츠 정의 | **[C]** | `content/*.json` | 게임메이커가 편집하는 템플릿. 서버 시작 시 ContentRegistry에 로드 |
| 런타임 상태 | **[R]** | ECS 인메모리 | 틱 루프에서 생성/변경/제거되는 인스턴스 |
| 영속 데이터 | **[P]** | `player.db` | 로그아웃 후에도 유지되는 플레이어/길드 데이터 |

**표기법**:
- 들여쓰기(└─)는 부모에 **내장**되는 하위 데이터를 나타낸다 (JSON 중첩 또는 ECS 컴포넌트).
- `→ ref` 표기는 문자열 ID로 다른 정의를 **참조**함을 나타낸다 (소유가 아닌 참조).
- `× N`은 배열로 여러 개 존재할 수 있음을 나타낸다.

---

## MUD 엔티티

### 1. 공간

```
Zone (지역) [C]
│   그룹 단위. 마을, 숲, 던전 1층 등.
│   content/zones/{zone_id}.json 하나가 Zone 하나.
│
└─ Room (방) [C] × N
   │   플레이어가 존재하는 이산 공간 단위.
   │   이름, 설명, 속성(safe_zone, dark 등).
   │
   ├─ Exit (출구) × N
   │     방향(north/south/up...) + 대상 방.
   │     조건부 가능 (level_min, key_item 등).
   │
   ├─ Spawn (스폰 규칙) × N
   │     entity_kind(npc/monster/object) + entity_id(→ ref).
   │
   └─ Portal (포탈) × N
         다른 존/방으로의 특수 이동. 일방향/양방향.
         비용, 레벨 제한 등.
```

### 2. 캐릭터

```
Player (플레이어) [R] [P]
│   유저가 조작하는 캐릭터.
│   [R] 접속 중에는 ECS 엔티티로 존재.
│   [P] 로그아웃 시 save_data(JSON)로 직렬화하여 DB 저장.
│
├─ Equipment (장비) — 슬롯 맵
│     {weapon, head, body, legs, feet, hands, accessory1, accessory2}
│     각 슬롯에 아이템 ID + 내구도.
│
├─ Inventory (인벤토리) × N
│     아이템 ID + 수량.
│
├─ Active Buff (활성 버프/디버프) × N
│     버프 ID(→ ref), 남은 시간, 중첩 횟수.
│
├─ Skill (습득 스킬) × N
│     스킬 ID(→ ref), 쿨다운 상태, 레벨.
│
├─ Quest Log (퀘스트 진행) × N
│     퀘스트 ID(→ ref), 현재 단계, 목표 달성도.
│
├─ Friend List (친구 목록) × N
│     대상 캐릭터 이름.
│
└─ Pet (활성 펫)
      펫 ID(→ ref), 이름, 현재 HP.
```

```
NPC (비전투 NPC) [C]
│   상인, 교관, 퀘스트 발주자, 은행원, 경비병 등.
│   역할(role)에 따라 참조하는 데이터가 달라짐.
│
├─ dialogue → ref (대화 트리 ID)
├─ shop → ref (상점 ID) — role=merchant일 때
├─ trainable_skills × N — role=trainer일 때
│     스킬 ID(→ ref), 비용, 선행 조건.
└─ quest_offers × N
      퀘스트 ID(→ ref), 수락 조건.
```

```
Monster (몬스터) [C]
│   적대 전투 대상. 일반/정예/보스 등급.
│
├─ loot_table → ref (드롭 테이블 ID)
├─ skills × N (스킬 ID 참조 배열)
└─ ai (행동 패턴)
      type, attack_style, skill_use_chance, target_priority.
      flee_hp_percent, assist 여부.
```

```
Pet (펫 정의) [C]
      HP, 공격, 방어, 스킬 목록.
      follow_distance.
```

### 3. 아이템

```
Item (아이템 정의) [C]
│   item_kind에 따라 속성이 달라짐.
│   모든 종류 공통: id, name, description, rarity, base_price, icon.
│
├─ Weapon (무기) — item_kind=weapon
│     weapon_type, attack_bonus, attack_speed, range, durability_max.
│
├─ Armor (방어구) — item_kind=armor
│     slot(head/body/legs/feet/hands), defense_bonus, durability_max.
│
├─ Accessory (악세서리) — item_kind=accessory
│     특수 효과(hp_bonus, critical_bonus 등).
│
├─ Consumable (소비품) — item_kind=consumable
│     heal_hp/heal_mp, buff(→ ref), stackable, max_stack, cooldown.
│
├─ Material (재료) — item_kind=material
│     stackable, max_stack.
│
├─ Quest Item (퀘스트 아이템) — item_kind=quest
│     quest(→ ref), tradeable=false.
│
├─ Key (열쇠) — item_kind=key
│     대상 문/상자 ID(→ ref).
│
├─ Container (가방) — item_kind=container
│     extra_slots(추가 인벤토리 슬롯 수).
│
└─ Currency (화폐) — item_kind=currency
      stackable, max_stack.
```

### 4. 오브젝트 (방 내 설치물)

```
Object (오브젝트 정의) [C]
│   object_kind에 따라 속성이 달라짐.
│
├─ Door (문) — object_kind=door
│     initial_state(open/closed/locked), key_item(→ ref).
│
├─ Chest (상자) — object_kind=chest
│     locked, key_item(→ ref), respawn_sec.
│     items × N: 아이템 ID(→ ref), 수량, 확률.
│
├─ Trap (함정) — object_kind=trap
│     hidden, damage, trigger_type(step/proximity/timed), rearm_sec.
│
├─ Sign (표지판) — object_kind=sign
│     text.
│
├─ Campfire (모닥불) — object_kind=campfire
│     heal_rate, radius.
│
├─ Crafting Station (제작대) — object_kind=crafting_station
│     craft_type(forge/alchemy/cooking...).
│     사용 가능 레시피는 ContentRegistry에서 craft_type으로 필터링.
│
└─ Bulletin Board (게시판) — object_kind=bulletin_board
      게시글은 런타임/DB 관리 (범위 외).
```

### 5. 공유 정의 (독립 콘텐츠 파일)

여러 엔티티가 ID로 참조하는 공유 정의. 각각 독립된 JSON 파일.

```
Loot Table (드롭 테이블) [C]
│   여러 몬스터가 하나의 테이블을 공유.
│   loot_tables.json
│
└─ Entry (드롭 항목) × N
      아이템 ID(→ ref), 확률(%), 수량 범위.
```

```
Shop (상점) [C]
│   여러 NPC가 하나의 상점을 공유.
│   shops.json
│
└─ Entry (판매 항목) × N
      아이템 ID(→ ref), 재고(-1=무한), 가격 오버라이드, 레벨 제한.
```

```
Dialogue (대화 트리) [C]
│   여러 NPC가 하나의 대화를 공유.
│   dialogues.json
│
├─ start_node (시작 노드 ID)
│
└─ Node (대화 노드) × N
   │   speaker, text, condition.
   │
   └─ Choice (선택지) × N
         text, next(다음 노드 ID), action, condition.
```

### 6. 시스템

```
Quest (퀘스트 정의) [C]
│   시작 조건, 반복 여부.
│
├─ Stage (단계) × N
│   │   순차 진행.
│   │
│   └─ Objective (목표) × N
│         type(kill/collect/visit/talk),
│         target(→ ref), quantity.
│
└─ Rewards (보상)
      exp, gold, items × N(아이템 ID + 수량).
```

```
Skill (스킬 정의) [C]
│   이름, 타입(active/passive), 최대 레벨.
│   mp_cost, cooldown, cast_time, range, target_type.
│
└─ Effect (효과) × N
      type(damage/heal/buff/debuff/stat_modify),
      base_value, per_level_value, element, scaling.
```

```
Class (클래스 정의) [C]
│   전사, 마법사, 도적 등.
│
├─ 기본 스탯 및 레벨당 성장치
│     base_hp/mp/attack/defense/magic_attack/magic_defense/speed.
│     hp_per_level, mp_per_level, attack_per_level 등.
│
├─ 장비 제한
│     equip_weapon_types, equip_armor_weight.
│
└─ Learnable Skill (습득 가능 스킬) × N
      스킬 ID(→ ref), 습득 레벨, 자동 습득 여부.
```

```
Recipe (제작법) [C]
│   craft_type, station_required, level_required, success_rate.
│
├─ Ingredient (재료) × N
│     아이템 ID(→ ref), 수량.
│
└─ Result (결과물)
      아이템 ID(→ ref), 수량.
```

```
Buff Definition (버프 정의) [C]
│   category(buff/debuff), duration, max_stacks, tick_interval.
│   dispellable 여부.
│
└─ Effect (효과) × N
      type(stat_modify/dot/hot/immunity/stun),
      target_stat, value, per_stack 여부.
```

```
Achievement (업적 정의) [C]
│   hidden 여부.
│
├─ Condition (달성 조건) × N
│     type(kill_count/quest_clear/level_reach/collect...),
│     target(→ ref), quantity.
│
└─ Rewards (보상)
      exp, gold, title, items.
```

### 7. 소셜

```
Guild (길드) [P]
│   name, leader. player.db guilds/guild_members 테이블.
│   data JSON: description, max_members, level, exp, gold, storage.
│
└─ Guild Member (길드원) [P] × N
      character(→ ref), rank(master/officer/member), contribution.
```

```
Party (파티) [R]
│   임시 그룹. 최대 인원.
│   ECS 인메모리에만 존재 (영속성 없음).
│
└─ Party Member (파티원) × N
      character(→ ref), role(leader/member).
```

---

## 2D 엔티티

MUD와 **공유되는 엔티티**(캐릭터, 아이템, 오브젝트, 시스템, 소셜)는 생략하고,
2D 전용이거나 구조가 다른 엔티티만 기술한다.

### 1. 공간 (MUD의 Zone/Room 대신)

```
Tile Type (타일 타입 정의) [C]
      tile_types.json.
      이름, 이동 가능 여부, 스프라이트,
      속성(speed_mult, damage_per_sec, swim_required 등).
```

```
Map (맵) [C]
│   2D 타일 기반 공간. width × height.
│   content/maps/{map_id}.json 하나가 Map 하나.
│
├─ tile_index (타일 인덱스 매핑)
│     숫자 인덱스 → 타일 타입 ID(→ ref).
│
├─ Layer (타일 레이어) × N
│   │   ground/decoration/collision 등 레이어 분리.
│   │   z_order(렌더 순서).
│   │
│   └─ tiles (2D 정수 배열)
│         tile_index의 인덱스 번호. -1 = 빈 타일.
│
├─ Spawn (스폰 규칙) × N
│     entity_kind + entity_id(→ ref), 좌표(x, y),
│     max_count, respawn_sec, radius.
│
├─ Portal (포탈) × N
│     좌표(x, y), target_map + target_x/y, bidirectional.
│
├─ Trigger (트리거 영역) × N
│     영역(x, y, w, h), event_type, event_data, once.
│
├─ Environment (환경 요소) × N
│   │
│   ├─ Light (광원) — type=light
│   │     좌표, 반경, 색상, 강도, flicker.
│   │
│   ├─ Sound (사운드) — type=sound
│   │     좌표, sound_file, 반경, loop.
│   │
│   └─ Particle (파티클) — type=particle
│         좌표, particle_type, rate.
│
└─ Patrol Path (순찰 경로) × N
      entity_kind + entity_id(→ ref), is_loop.
      └─ Waypoint (경유지) × N
            좌표(x, y), wait_sec.
```

### 2. 캐릭터 — 2D 추가 사항

```
Player (플레이어) [R] [P] — MUD 구조 + 아래 추가
│
├─ Position (위치)
│     map_id(→ ref), x, y 좌표.
│
├─ Sprite (스프라이트)
│     스프라이트시트 참조, 방향(direction).
│
└─ (나머지는 MUD와 동일: Equipment, Inventory, Buff, Skill, Quest Log)

NPC / Monster [C] — MUD 구조 + 아래 추가
│
├─ sprite — 스프라이트 참조.
├─ aggro_radius, chase_radius, return_radius — 어그로 범위.
└─ (순찰 경로는 Map에 정의)
```

### 3. 전투/이펙트 — 2D 전용

```
Projectile (투사체) [R]
      발사자, 방향/속도, 데미지, 스프라이트,
      관통 여부, 최대 사거리.
      런타임에만 존재 (ECS 인메모리).

AoE Zone (범위 효과 지역) [R]
      중심 좌표, 반경, 지속 시간,
      효과(데미지/힐/감속 등), 틱 간격.
      런타임에만 존재 (ECS 인메모리).
```

### 4. 날씨 — 2D 전용

```
Weather Zone (날씨 영역) [R]
      맵 참조, type(rain/snow/fog),
      강도, 시각/게임플레이 효과.
      런타임에만 존재하거나, Map JSON의 environment에 정의.
```

---

## 엔티티 참조 관계 요약

### MUD

```
[콘텐츠 정의]

Zone
└─ Room
   ├─ Exit → Room (같은/다른 Zone)
   ├─ Spawn → Monster / NPC / Object
   └─ Portal → Room

Monster → Loot Table → Item (정의)
        → Skill (정의)

NPC → Dialogue
    → Shop → Item (정의)
    → Skill (정의) — trainable_skills
    → Quest (정의) — quest_offers

Loot Table ← 여러 Monster가 공유
Shop       ← 여러 NPC가 공유
Dialogue   ← 여러 NPC가 공유

Quest → Stage → Objective → Monster / NPC / Item (대상)
Class → Skill (습득 목록)
Recipe → Item (재료/결과)
Skill → Buff (효과에서 참조)
Achievement → Monster / Quest / Item (조건 대상)

[런타임 + 영속]

Player (ECS / save_data)
├─ Equipment → Item (정의)
├─ Inventory → Item (정의)
├─ Active Buff → Buff (정의)
├─ Skill → Skill (정의)
├─ Quest Log → Quest (정의)
├─ Pet → Pet (정의)
└─ Friend List

Guild (DB) → Character
Party (ECS) → Character
```

### 2D

```
[콘텐츠 정의]

Tile Type (독립)

Map
├─ tile_index → Tile Type
├─ Layer → tiles (숫자 배열)
├─ Spawn → Monster / NPC / Object
├─ Portal → Map
├─ Trigger
├─ Environment (Light / Sound / Particle)
└─ Patrol Path → Monster / NPC

[런타임]

Player (MUD와 동일 + Position, Sprite)
NPC / Monster (MUD 정의 + sprite, aggro 범위)
Projectile (ECS 인메모리)
AoE Zone (ECS 인메모리)
Weather Zone (ECS 인메모리)
```

---

## 계층별 분류

### 콘텐츠 정의 [C] — content/*.json

| 파일 | 엔티티 |
|------|--------|
| monsters.json | Monster (ai, skills 내장) |
| npcs.json | NPC (trainable_skills, quest_offers 내장) |
| items.json | Item (kind별 속성 내장) |
| objects.json | Object (kind별 속성 내장) |
| pets.json | Pet |
| skills.json | Skill (effects 내장) |
| quests.json | Quest (stages, objectives, rewards 내장) |
| classes.json | Class (base stats, skills 내장) |
| recipes.json | Recipe (ingredients, result 내장) |
| buffs.json | Buff (effects 내장) |
| achievements.json | Achievement (conditions, rewards 내장) |
| loot_tables.json | Loot Table (entries 내장) |
| shops.json | Shop (entries 내장) |
| dialogues.json | Dialogue (nodes, choices 내장) |
| tile_types.json | Tile Type (2D 전용) |
| zones/{id}.json | Zone + Room + Exit + Spawn + Portal (MUD 전용) |
| maps/{id}.json | Map + Layer + Spawn + Portal + Trigger + Environment (2D 전용) |

### 런타임 상태 [R] — ECS 인메모리

| 엔티티 | 생성 시점 | 제거 시점 |
|--------|-----------|-----------|
| Player | 로그인 | 로그아웃 |
| Monster (인스턴스) | 스폰 | 사망/디스폰 |
| NPC (인스턴스) | 서버 시작 | 서버 종료 |
| Object (인스턴스) | 서버 시작/리스폰 | — |
| Pet (인스턴스) | 소환 | 해제/사망 |
| Party | 생성 | 해산 |
| Projectile | 발사 | 충돌/사거리 초과 (2D) |
| AoE Zone | 스킬 사용 | 지속시간 만료 (2D) |
| Weather Zone | 이벤트/맵 설정 | 이벤트 종료 (2D) |

### 영속 데이터 [P] — player.db

| 테이블 | 엔티티 |
|--------|--------|
| accounts | 계정 (auth_mode=account 시) |
| characters | 캐릭터 (save_data JSON) |
| guilds | 길드 |
| guild_members | 길드원 |
| game_meta | 런타임 설정 |

---

## 공유 / 전용 분류

| 분류 | MUD 전용 | 2D 전용 | 공유 |
|------|----------|---------|------|
| 공간 | Zone, Room, Exit | Map, Layer, Tile, Tile Type, Trigger | Portal, Spawn |
| 캐릭터 | — | Position, Sprite, Patrol Path, Aggro Range | Player, NPC, Monster, Pet |
| 아이템 | — | — | 전부 공유 |
| 오브젝트 | — | — | 전부 공유 |
| 전투 | — | Projectile, AoE Zone | Buff, Skill |
| 환경 | — | Light, Sound, Particle, Weather Zone | — |
| 시스템 | — | — | Quest, Class, Recipe, Achievement |
| 공유 정의 | — | — | Loot Table, Shop, Dialogue |
| 소셜 | — | — | Guild, Party |
