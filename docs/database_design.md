# Project G — 데이터 설계

> 작성일: 2026-02-20
> 개정일: 2026-02-20
> 상태: 초안 v3

## 개요

게임 콘텐츠는 **JSON 파일**, 플레이어 영속성은 **SQLite**로 분리한다.
하나의 엔진이 MUD/2D 양쪽 모드를 지원하되, 콘텐츠는 게임별로 분리한다.

## 설계 원칙

- **콘텐츠 = 파일**: 몬스터, 아이템, 스킬, 퀘스트 등 게임 정의 데이터는 JSON 파일로 관리
- **DB = 플레이어만**: SQLite는 계정, 캐릭터 세이브, 길드 등 런타임 영속 데이터 전용
- **시작 시 일괄 로드**: 서버 시작 시 JSON 파일 전체를 메모리에 올리고, 런타임에는 인메모리 참조
- **파일 = 진실의 원천**: 게임메이커가 JSON 파일을 편집, git으로 버전 관리 가능
- **소프트 참조**: 엔티티 간 참조는 문자열 ID. 로드 시 앱 레벨에서 검증
- **댕글링 참조 방어**: 콘텐츠 ID가 삭제되어도 save_data에 남은 참조는 로그인 시 검증 → 없는 항목 제거 + 로그 기록

### 왜 콘텐츠에 DB를 쓰지 않나

| 비교 | JSON 파일 | SQLite 테이블 |
|------|-----------|---------------|
| 규모 | 수백~수천 건 (충분) | 수만 건 이상에 유리 |
| 편집 | 텍스트 에디터, git diff | 전용 도구 필요 |
| 버전 관리 | git으로 자연스러움 | DB 덤프 필요 |
| 스키마 변경 | 필드 추가/삭제 자유 | 마이그레이션 필요 |
| 검색 | 메모리에 올려서 검색 | SQL WHERE 절 |
| 배포 | 파일 복사 | 파일 복사 (동일) |

콘텐츠 데이터는 규모가 작고(몬스터 200종, 아이템 500종 정도), 서버 시작 시 전체 로드하므로 파일로 충분하다.

---

## 디렉토리 구조

```
games/my_mud/
├── game.toml                    ← 게임 설정
├── content/
│   ├── monsters.json            ← 몬스터 정의
│   ├── npcs.json                ← NPC 정의
│   ├── items.json               ← 아이템 정의
│   ├── objects.json             ← 오브젝트 정의 (문/상자/함정/표지판...)
│   ├── pets.json                ← 펫 정의
│   ├── skills.json              ← 스킬 정의
│   ├── quests.json              ← 퀘스트 정의
│   ├── classes.json             ← 클래스 정의
│   ├── recipes.json             ← 제작법 정의
│   ├── buffs.json               ← 버프/디버프 정의
│   ├── achievements.json        ← 업적 정의
│   ├── loot_tables.json         ← 드롭 테이블
│   ├── shops.json               ← 상점 정의
│   ├── dialogues.json           ← 대화 트리
│   ├── tile_types.json          ← 2D 타일 타입 정의
│   ├── zones/                   ← MUD: zone별 파일
│   │   ├── riverdale.json       (zone + rooms + exits + spawns + portals)
│   │   └── dark_forest.json
│   └── maps/                    ← 2D: map별 파일
│       ├── field_01.json        (map + layers + tiles + spawns + portals + triggers)
│       └── town_01.json
├── player.db                    ← SQLite (플레이어 영속성만)
└── scripts/                     ← Lua 스크립트
```

---

## 데이터 흐름

```
서버 시작
  ├─ game.toml 읽기 → 모드(mud/grid), 인증 방식 결정
  ├─ content/*.json 전체 로드 → ContentRegistry (인메모리 HashMap)
  ├─ content/zones/*.json 또는 content/maps/*.json → Space 구성
  ├─ player.db 연결 (없으면 생성)
  └─ Lua 스크립트 로드

런타임 (인메모리)
  ├─ 플레이어 로그인 → characters.save_data → ECS 엔티티 복원
  ├─ 플레이어 로그아웃 → ECS 컴포넌트 → JSON → characters.save_data 저장
  ├─ 리스폰 타이머 → ContentRegistry에서 정의 참조 → ECS 엔티티 생성
  └─ 길드 변경 → player.db에 즉시 반영

서버 종료
  └─ 접속 중인 플레이어 전부 save_data 저장
```

## 데이터 계층 정리

| 계층 | 저장소 | 데이터 | 변경 주체 |
|------|--------|--------|-----------|
| 설정 | `game.toml` | 모드, 인증, 서버 설정 | 게임메이커 |
| 정의 | `content/*.json` | 몬스터, 아이템, 스킬, 공간 등 | 게임메이커 |
| 로직 | `scripts/*.lua` | 전투, AI, 명령어, 이벤트 | 게임메이커 |
| 상태 | ECS 인메모리 | 현재 HP, 위치, 인벤토리, 쿨다운 | 게임 로직 (매 틱) |
| 영속 | `player.db` | 캐릭터 세이브, 길드 | 로그인/로그아웃, 길드 변경 시 |

---

## 게임 설정 (game.toml)

```toml
[game]
name = "리버델 전기"
mode = "mud"           # "mud" | "grid"
auth_mode = "character" # "character" | "account"

[server]
tick_rate = 10
max_players = 100
ws_port = 4001
telnet_port = 4000     # MUD 전용

[grid]                  # mode = "grid" 일 때만
width = 512
height = 512
aoi_radius = 32
```

> **game.toml vs game_meta 테이블 구분**: game.toml은 서버 시작 전에 결정되는 설정 (모드, 포트, 틱레이트). game_meta는 런타임 중 변경되거나 DB와 함께 관리해야 하는 값 (schema_version, 서버 상태 플래그).

---

## 콘텐츠 파일 포맷

### monsters.json

```json
[
    {
        "id": "goblin_warrior",
        "name": "고블린 전사",
        "description": "작고 사악한 전사",
        "grade": "normal",
        "level": 5,
        "loot_table": "goblin_loot",
        "hp_max": 80,
        "attack": 12,
        "defense": 5,
        "magic_attack": 0,
        "magic_defense": 0,
        "speed": 10,
        "exp_reward": 25,
        "gold_reward": {"min": 5, "max": 15},
        "aggressive": true,
        "assist": false,
        "flee_hp_percent": 0,
        "ai": {
            "type": "aggressive",
            "attack_style": "melee",
            "skill_use_chance": 0.3,
            "target_priority": "nearest"
        },
        "skills": ["slash"],
        "sprite": "goblin_warrior.png",
        "aggro_radius": 5,
        "chase_radius": 10,
        "return_radius": 15
    },
    {
        "id": "skeleton_knight",
        "name": "해골 기사",
        "description": "오래된 갑옷을 입은 해골",
        "grade": "elite",
        "level": 12,
        "loot_table": "skeleton_loot",
        "hp_max": 250,
        "attack": 28,
        "defense": 20,
        "exp_reward": 80,
        "gold_reward": {"min": 20, "max": 50},
        "aggressive": true,
        "ai": {"type": "aggressive", "attack_style": "melee"},
        "skills": ["heavy_slash", "defend"]
    }
]
```

### npcs.json

```json
[
    {
        "id": "weapon_merchant",
        "name": "무기 상인 그레고리",
        "description": "든든한 무기를 팔고 있다",
        "role": "merchant",
        "shop": "weapon_shop",
        "dialogue": "weapon_merchant_talk",
        "greeting": "어서 오게, 좋은 무기가 많다네.",
        "immortal": true,
        "sprite": "npc_merchant.png",
        "direction": "down"
    },
    {
        "id": "combat_trainer",
        "name": "교관 마르쿠스",
        "role": "trainer",
        "dialogue": "trainer_talk",
        "trainable_skills": [
            {"skill": "slash", "cost_gold": 0, "level_required": 1},
            {"skill": "whirlwind", "cost_gold": 100, "level_required": 10, "class_required": "warrior"}
        ],
        "quest_offers": ["slay_goblin"],
        "immortal": true
    }
]
```

> NPC의 `trainable_skills`, `quest_offers`는 NPC에 직접 내장. 역방향 조회("이 스킬을 가르치는 NPC")는 ContentRegistry에서 인덱스 구축.

### items.json

```json
[
    {
        "id": "iron_sword",
        "name": "철검",
        "description": "평범한 철제 검",
        "item_kind": "weapon",
        "rarity": "common",
        "base_price": 50,
        "weapon_type": "sword",
        "attack_bonus": 8,
        "attack_speed": 1.0,
        "range": 1,
        "durability_max": 100,
        "level_required": 3,
        "weight": 2.5,
        "icon": "iron_sword.png"
    },
    {
        "id": "health_potion",
        "name": "체력 물약",
        "description": "HP를 50 회복한다",
        "item_kind": "consumable",
        "rarity": "common",
        "base_price": 15,
        "heal_hp": 50,
        "stackable": true,
        "max_stack": 20,
        "cooldown_sec": 5,
        "cooldown_group": "potion",
        "icon": "health_potion.png"
    },
    {
        "id": "leather_armor",
        "name": "가죽 갑옷",
        "item_kind": "armor",
        "rarity": "common",
        "base_price": 80,
        "slot": "body",
        "defense_bonus": 10,
        "hp_bonus": 20,
        "durability_max": 150,
        "level_required": 2,
        "icon": "leather_armor.png"
    }
]
```

### objects.json

```json
[
    {
        "id": "town_gate",
        "name": "마을 정문",
        "object_kind": "door",
        "initial_state": "open",
        "sprite_open": "gate_open.png",
        "sprite_closed": "gate_closed.png"
    },
    {
        "id": "treasure_chest_01",
        "name": "낡은 보물 상자",
        "object_kind": "chest",
        "locked": true,
        "key_item": "rusty_key",
        "respawn_sec": 300,
        "items": [
            {"item": "health_potion", "quantity": 2, "chance": 1.0},
            {"item": "iron_sword", "quantity": 1, "chance": 0.3}
        ]
    },
    {
        "id": "spike_trap",
        "name": "가시 함정",
        "object_kind": "trap",
        "hidden": true,
        "damage": 30,
        "trigger_type": "step",
        "rearm_sec": 60
    },
    {
        "id": "town_sign",
        "name": "마을 표지판",
        "object_kind": "sign",
        "text": "리버델 마을에 오신 것을 환영합니다!"
    },
    {
        "id": "village_campfire",
        "name": "모닥불",
        "object_kind": "campfire",
        "heal_rate": 2.0,
        "radius": 3
    },
    {
        "id": "blacksmith_anvil",
        "name": "대장간 모루",
        "object_kind": "crafting_station",
        "craft_type": "forge"
    }
]
```

### pets.json

```json
[
    {
        "id": "wolf_pup",
        "name": "아기 늑대",
        "hp_max": 60,
        "attack": 8,
        "defense": 4,
        "skills": ["bite"],
        "sprite": "wolf_pup.png",
        "follow_distance": 2
    }
]
```

### skills.json

```json
[
    {
        "id": "fireball",
        "name": "파이어볼",
        "description": "화염 구체를 발사한다",
        "type": "active",
        "max_level": 5,
        "mp_cost": 15,
        "mp_cost_per_level": 3,
        "cooldown_sec": 3.0,
        "cast_time_sec": 0.5,
        "range": 5,
        "target_type": "single_enemy",
        "icon": "fireball.png",
        "projectile_sprite": "fireball_proj.png",
        "projectile_speed": 8.0,
        "effects": [
            {
                "type": "damage",
                "base_value": 30,
                "per_level_value": 8,
                "element": "fire",
                "scaling_stat": "magic_attack",
                "scaling_ratio": 0.8
            },
            {
                "type": "debuff",
                "buff": "burning",
                "duration_sec": 5
            }
        ]
    },
    {
        "id": "slash",
        "name": "베기",
        "description": "검으로 강하게 벤다",
        "type": "active",
        "mp_cost": 5,
        "cooldown_sec": 1.5,
        "range": 1,
        "target_type": "single_enemy",
        "effects": [
            {"type": "damage", "base_value": 15, "per_level_value": 5, "scaling_stat": "attack", "scaling_ratio": 1.2}
        ]
    }
]
```

### quests.json

단계(stages), 목표(objectives), 보상(rewards) 모두 내장.

```json
[
    {
        "id": "slay_goblin",
        "name": "고블린 퇴치",
        "description": "마을 북쪽의 고블린을 처치하라",
        "level_min": 1,
        "repeatable": false,
        "stages": [
            {
                "description": "고블린 처치",
                "objectives": [
                    {"type": "kill", "target": "goblin_warrior", "quantity": 5, "description": "고블린 전사 5마리 처치"}
                ]
            },
            {
                "description": "교관에게 보고",
                "objectives": [
                    {"type": "talk", "target": "combat_trainer", "quantity": 1, "description": "교관 마르쿠스에게 보고"}
                ]
            }
        ],
        "rewards": {
            "exp": 100,
            "gold": 50,
            "items": [
                {"item": "health_potion", "quantity": 3}
            ]
        }
    }
]
```

### classes.json

기본 스탯, 성장치, 습득 스킬 모두 내장.

```json
[
    {
        "id": "warrior",
        "name": "전사",
        "description": "근접 전투에 특화된 클래스",
        "base_hp": 120, "base_mp": 20,
        "base_attack": 15, "base_defense": 10,
        "base_magic_attack": 3, "base_magic_defense": 5,
        "base_speed": 10,
        "hp_per_level": 12, "mp_per_level": 2,
        "attack_per_level": 3, "defense_per_level": 2,
        "equip_weapon_types": ["sword", "axe", "mace"],
        "equip_armor_weight": "mail",
        "sprite": "warrior.png",
        "skills": [
            {"skill": "slash", "learn_level": 1, "auto_learn": true},
            {"skill": "defend", "learn_level": 3, "auto_learn": true},
            {"skill": "whirlwind", "learn_level": 10, "auto_learn": false}
        ]
    }
]
```

### recipes.json

```json
[
    {
        "id": "craft_iron_sword",
        "name": "철검 제작",
        "craft_type": "forge",
        "station_required": "forge",
        "result": {"item": "iron_sword", "quantity": 1},
        "level_required": 5,
        "success_rate": 0.9,
        "ingredients": [
            {"item": "iron_ore", "quantity": 3},
            {"item": "coal", "quantity": 1}
        ]
    }
]
```

### buffs.json

```json
[
    {
        "id": "burning",
        "name": "화상",
        "description": "불에 타고 있다",
        "category": "debuff",
        "duration_sec": 10,
        "max_stacks": 3,
        "tick_interval_sec": 2,
        "dispellable": true,
        "icon": "burning.png",
        "effects": [
            {"type": "dot", "target_stat": "hp", "value": -5, "per_stack": true},
            {"type": "stat_modify", "target_stat": "defense", "value": -3}
        ]
    }
]
```

### achievements.json

```json
[
    {
        "id": "goblin_slayer",
        "name": "고블린 슬레이어",
        "description": "고블린 100마리를 처치하라",
        "hidden": false,
        "conditions": [
            {"type": "kill_count", "target": "goblin_warrior", "quantity": 100}
        ],
        "rewards": {
            "exp": 500,
            "gold": 100,
            "title": "고블린 슬레이어"
        }
    }
]
```

### loot_tables.json

여러 몬스터가 하나의 테이블을 공유할 수 있도록 분리 파일.

```json
[
    {
        "id": "goblin_loot",
        "name": "고블린 공통 드롭",
        "entries": [
            {"item": "goblin_ear", "chance": 0.8, "quantity_min": 1, "quantity_max": 2},
            {"item": "iron_sword", "chance": 0.05, "quantity_min": 1, "quantity_max": 1},
            {"item": "health_potion", "chance": 0.15, "quantity_min": 1, "quantity_max": 1}
        ]
    }
]
```

### shops.json

여러 NPC가 하나의 상점을 공유할 수 있도록 분리 파일.

```json
[
    {
        "id": "weapon_shop",
        "name": "무기 상점",
        "buy_rate": 1.0,
        "sell_rate": 0.5,
        "entries": [
            {"item": "iron_sword", "stock": -1},
            {"item": "steel_sword", "stock": -1, "level_required": 10},
            {"item": "health_potion", "stock": -1, "price_override": 20}
        ]
    }
]
```

### dialogues.json

대화 트리 전체를 하나의 객체로 표현.

```json
[
    {
        "id": "weapon_merchant_talk",
        "name": "무기 상인 대화",
        "start_node": "greeting",
        "nodes": {
            "greeting": {
                "speaker": "무기 상인 그레고리",
                "text": "어서 오게, 좋은 무기가 많다네.",
                "choices": [
                    {"text": "물건을 보여주세요", "action": {"type": "open_shop"}},
                    {"text": "이 마을에 대해 알려주세요", "next": "town_info"},
                    {"text": "안녕히 계세요"}
                ]
            },
            "town_info": {
                "text": "북쪽 숲에 고블린이 나타나고 있다네. 교관 마르쿠스에게 물어보게.",
                "condition": {"type": "quest_not_started", "quest": "slay_goblin"},
                "choices": [
                    {"text": "알겠습니다"}
                ]
            }
        }
    }
]
```

### tile_types.json (2D 전용)

```json
[
    {"id": "grass", "name": "풀밭", "walkable": true, "sprite": "grass.png", "speed_mult": 1.0},
    {"id": "water", "name": "물", "walkable": false, "sprite": "water.png", "swim_required": true},
    {"id": "stone", "name": "돌바닥", "walkable": true, "sprite": "stone.png", "speed_mult": 1.0},
    {"id": "lava", "name": "용암", "walkable": true, "sprite": "lava.png", "damage_per_sec": 10, "speed_mult": 0.5}
]
```

---

## 공간 파일 — MUD

`content/zones/{zone_id}.json` — zone 하나에 소속된 rooms, exits, spawns, portals을 모두 포함.

```json
{
    "id": "riverdale",
    "name": "리버델 마을",
    "description": "평화로운 강변 마을",
    "level_min": 1,
    "level_max": 10,
    "pvp_enabled": false,
    "respawn_room": "town_square",
    "rooms": [
        {
            "id": "town_square",
            "name": "마을 광장",
            "description": "넓은 광장 중앙에 분수가 있다. 사람들이 오가고 있다.",
            "safe_zone": true,
            "exits": [
                {"direction": "north", "target": "weapon_shop"},
                {"direction": "south", "target": "south_gate"},
                {"direction": "east", "target": "dark_forest:forest_entrance"}
            ],
            "spawns": [
                {"entity_kind": "npc", "entity_id": "weapon_merchant"},
                {"entity_kind": "object", "entity_id": "town_sign"}
            ]
        },
        {
            "id": "weapon_shop",
            "name": "무기점",
            "description": "벽에 각종 무기가 걸려 있다.",
            "exits": [
                {"direction": "south", "target": "town_square"}
            ],
            "spawns": [
                {"entity_kind": "npc", "entity_id": "weapon_merchant"}
            ]
        },
        {
            "id": "south_gate",
            "name": "남문",
            "description": "마을의 남쪽 출입구다.",
            "exits": [
                {"direction": "north", "target": "town_square"},
                {"direction": "south", "target": "dark_forest:forest_entrance", "level_min": 3, "message_blocked": "아직 너무 위험하다."}
            ],
            "spawns": [
                {"entity_kind": "object", "entity_id": "town_gate"}
            ]
        }
    ],
    "portals": [
        {
            "id": "town_portal",
            "source_room": "town_square",
            "target_room": "dungeon_01:entrance",
            "bidirectional": false,
            "level_min": 10,
            "cost_gold": 50,
            "message_enter": "포탈을 통해 던전으로 이동한다..."
        }
    ]
}
```

> 타 zone의 방을 참조할 때는 `"zone_id:room_id"` 표기법 사용. 같은 zone 내는 `"room_id"`만.

---

## 공간 파일 — 2D

`content/maps/{map_id}.json` — map 하나에 소속된 layers, tiles, spawns, portals, triggers, environment를 모두 포함.

```json
{
    "id": "field_01",
    "name": "초원 평야",
    "width": 64,
    "height": 64,
    "pvp_enabled": false,
    "ambient_light": 1.0,
    "bgm": "field_theme.ogg",
    "tile_index": {"0": "grass", "1": "water", "2": "stone", "3": "flower", "4": "rock", "5": "bush"},
    "layers": [
        {
            "name": "ground",
            "z_order": 0,
            "tiles": [
                [0, 0, 0, 1, 1],
                [0, 2, 0, 1, 1],
                [0, 0, 0, 0, 0]
            ]
        },
        {
            "name": "decoration",
            "z_order": 1,
            "tiles": [
                [-1, -1, 3, -1, -1],
                [-1, -1, -1, -1, -1],
                [4, -1, -1, -1, 5]
            ]
        }
    ],
    "spawns": [
        {"entity_kind": "monster", "entity_id": "goblin_warrior", "x": 20, "y": 30, "max_count": 3, "respawn_sec": 60, "radius": 5},
        {"entity_kind": "npc", "entity_id": "combat_trainer", "x": 10, "y": 10}
    ],
    "portals": [
        {"x": 0, "y": 32, "target_map": "town_01", "target_x": 63, "target_y": 32, "bidirectional": true}
    ],
    "triggers": [
        {"x": 30, "y": 30, "width": 5, "height": 5, "event_type": "script", "event_data": {"script": "ambush_event"}, "once": true}
    ],
    "environment": [
        {"type": "light", "x": 10, "y": 10, "radius": 8, "color": "#ffaa44", "flicker": true},
        {"type": "sound", "x": 32, "y": 48, "sound_file": "river.ogg", "radius": 10, "loop": true},
        {"type": "particle", "x": 5, "y": 5, "particle_type": "firefly", "rate": 3}
    ],
    "patrol_paths": [
        {
            "entity_kind": "monster",
            "entity_id": "goblin_warrior",
            "is_loop": true,
            "waypoints": [
                {"x": 20, "y": 30, "wait_sec": 2},
                {"x": 25, "y": 30, "wait_sec": 0},
                {"x": 25, "y": 35, "wait_sec": 3}
            ]
        }
    ]
}
```

> `tiles`는 2D 배열 (행 → 열), 값은 `tile_index` 매핑의 인덱스 번호 (-1 = 빈 타일). 문자열 반복 대신 숫자를 사용해 대형 맵 파일 크기를 최소화한다. 추후 바이너리 포맷으로 전환 가능.

---

## 콘텐츠 로딩 — ContentRegistry

서버 시작 시 모든 JSON 파일을 읽어 인메모리 `ContentRegistry`에 저장.
엔진은 게임별 스키마(MonsterDef, ItemDef 등)를 모른다 — `serde_json::Value`로 범용 처리.
게임 로직(Lua)이 필드를 해석한다.

```rust
/// 엔진 레벨 — 게임 스키마 비의존
pub struct ContentRegistry {
    /// "monsters" → {"goblin_warrior" → Value, ...}
    /// "items"    → {"iron_sword" → Value, ...}
    collections: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl ContentRegistry {
    /// content/ 디렉토리의 모든 JSON 파일을 로드.
    /// 파일명(확장자 제외)이 컬렉션 이름, 각 객체의 "id" 필드가 키.
    pub fn load_dir(path: &Path) -> Result<Self, Error> { ... }

    /// 컬렉션에서 ID로 조회
    pub fn get(&self, collection: &str, id: &str) -> Option<&Value> { ... }

    /// 컬렉션 전체 반환
    pub fn all(&self, collection: &str) -> Option<&HashMap<String, Value>> { ... }
}
```

**Lua API**:
```lua
-- content:get(collection, id) → table or nil
local goblin = content:get("monsters", "goblin_warrior")
print(goblin.name)       -- "고블린 전사"
print(goblin.loot_table) -- "goblin_loot"

-- content:all(collection) → {id = table, ...}
for id, item in pairs(content:all("items")) do ... end
```

**왜 typed Rust 구조체를 쓰지 않나**:

| 비교 | 동적 (serde_json::Value) | Typed (MonsterDef 등) |
|------|--------------------------|----------------------|
| 엔진-게임 분리 | 엔진이 스키마 모름 (분리 완전) | 엔진이 게임 스키마 의존 (위반) |
| 스키마 변경 | JSON + Lua만 수정, Rust 재컴파일 불필요 | Rust 구조체 수정 + 재컴파일 필요 |
| 다중 게임 | 같은 엔진으로 스키마 다른 게임 운영 가능 | 게임마다 Rust 구조체 세트 필요 |
| 타입 안전성 | 런타임에만 오류 발견 | 로드 시 즉시 타입 오류 발견 |
| 보완 | JSON Schema 검증으로 타입 안전성 확보 가능 | — |

**로드 순서**:
1. `content/` 디렉토리의 모든 `*.json` 파일 로드 → collections에 등록
2. `content/zones/*.json`, `content/maps/*.json` 별도 로드 → 공간 구성
3. 참조 검증 (선택적, 설정 기반 — 아래 참조)

**참조 검증**: 엔진은 "어떤 필드가 어떤 컬렉션의 ID를 참조하는지" 모른다.
두 가지 방식으로 해결:
- **설정 기반**: `content/refs.json`에 참조 규칙 정의 → 엔진이 로드 시 검증
- **Lua 기반**: `hooks.on_init`에서 Lua 스크립트가 검증 로직 실행

```json
// content/refs.json (선택적)
{
    "monsters": {
        "loot_table": "loot_tables",
        "skills[]": "skills"
    },
    "npcs": {
        "shop": "shops",
        "dialogue": "dialogues",
        "quest_offers[]": "quests",
        "trainable_skills[].skill": "skills"
    }
}
```

**역방향 인덱스**: "이 스킬을 가르치는 NPC는?" 같은 역방향 조회는 Lua에서 `on_init` 시점에 직접 구축하거나, 엔진이 refs.json 기반으로 자동 생성.

---

## 플레이어 DB (player.db)

SQLite. 5개 테이블.

### 스키마

```sql
-- 게임 메타 (인증 모드 등 런타임 설정)
CREATE TABLE game_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 계정 (auth_mode = "account" 일 때만 사용)
CREATE TABLE accounts (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at    TEXT DEFAULT (datetime('now')),
    banned        INTEGER DEFAULT 0,
    last_login    TEXT,
    data          TEXT DEFAULT '{}'     -- ban_reason, ban_until, last_ip, max_characters
);

-- 캐릭터 (항상 사용)
CREATE TABLE characters (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER REFERENCES accounts(id) ON DELETE CASCADE,  -- character모드: NULL
    name          TEXT UNIQUE NOT NULL,
    password_hash TEXT,                 -- character모드: 여기에 저장, account모드: NULL
    save_data     TEXT NOT NULL DEFAULT '{}',
    last_login    TEXT,
    play_time     INTEGER DEFAULT 0,   -- 초 단위
    created_at    TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_characters_account ON characters(account_id) WHERE account_id IS NOT NULL;

-- 길드
CREATE TABLE guilds (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT UNIQUE NOT NULL,
    leader_id   INTEGER NOT NULL REFERENCES characters(id),
    created_at  TEXT DEFAULT (datetime('now')),
    data        TEXT DEFAULT '{}'      -- description, max_members, level, exp, gold, storage(길드 창고 아이템 목록)
);

-- 길드원
CREATE TABLE guild_members (
    guild_id    INTEGER NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    char_id     INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    rank        TEXT DEFAULT 'member', -- "master", "officer", "member"
    joined_at   TEXT DEFAULT (datetime('now')),
    data        TEXT DEFAULT '{}',     -- contribution
    PRIMARY KEY (guild_id, char_id)
);

CREATE INDEX idx_guild_members_char ON guild_members(char_id);
```

### save_data 구조

플레이어의 모든 런타임 상태를 JSON으로 직렬화:

```json
{
    "level": 12,
    "exp": 2340,
    "class": "warrior",
    "hp_current": 85, "hp_max": 150,
    "mp_current": 20, "mp_max": 30,
    "attack": 25, "defense": 18,
    "magic_attack": 5, "magic_defense": 8,
    "speed": 12, "critical_rate": 0.08,
    "gold": 1250,
    "room_id": "town_square",
    "equipment": {
        "weapon": {"item": "iron_sword", "durability": 87},
        "body": {"item": "leather_armor", "durability": 95}
    },
    "inventory": [
        {"item": "health_potion", "quantity": 5},
        {"item": "goblin_ear", "quantity": 3}
    ],
    "skills": [
        {"skill": "slash", "level": 3, "cooldown": 0},
        {"skill": "defend", "level": 2, "cooldown": 0}
    ],
    "buffs": [],
    "quests": [
        {"quest": "slay_goblin", "stage": 0, "progress": {"kill_goblin_warrior": 3}}
    ],
    "friends": ["Alice", "Bob"],
    "achievements": ["first_kill", "level_10"],
    "pet": {"pet": "wolf_pup", "name": "늑대", "hp_current": 50}
}
```

2D 모드에서는 추가 필드:
```json
{
    "map_id": "field_01",
    "x": 120, "y": 85,
    "sprite": "warrior.png",
    "direction": "down"
}
```

### 인증 모드별 동작

**MUD (auth_mode = "character")**: 캐릭터 = 계정

```
접속 → "이름을 입력하세요:" → "고블린슬레이어"
→ DB 조회 (SELECT * FROM characters WHERE name = ?)
  → 없으면: "새 캐릭터입니다. 비밀번호를 설정하세요:"
  → 있으면: "비밀번호를 입력하세요:"
→ 인증 성공 → save_data에서 컴포넌트 복원 → 게임 진입
```

**2D (auth_mode = "account")**: 계정 + 캐릭터 분리

```
접속 → 아이디/비밀번호 입력
→ accounts 인증
→ 캐릭터 선택 화면 (SELECT * FROM characters WHERE account_id = ?)
→ 캐릭터 선택 → save_data에서 복원 → 게임 진입
```

---

## 이전 설계 대비 변경점

| 항목 | v2 (37 테이블 SQLite) | v3 (JSON 파일 + 5 테이블) |
|------|----------------------|--------------------------|
| 콘텐츠 저장 | SQLite 32개 테이블 | JSON 파일 16종 |
| 플레이어 저장 | SQLite 5개 테이블 | SQLite 5개 테이블 (동일) |
| 콘텐츠 편집 | DB 도구 필요 | 텍스트 에디터 / 게임메이커 UI |
| 버전 관리 | DB 덤프/마이그레이션 | git (JSON 파일 직접 커밋) |
| 스키마 변경 | ALTER TABLE / 마이그레이션 | JSON 필드 추가/삭제 (자유) |
| 검색/필터 | SQL WHERE 절 | ContentRegistry 인메모리 검색 |
| 참조 무결성 | FK 제약 | 로드 시 앱 레벨 검증 |
| 콘텐츠 DB 테이블 수 | 32개 | 0개 |
| 총 DB 테이블 수 | 37개 | 5개 |

---

## 콘텐츠 파일 목록

| # | 파일 | 내용 | MUD | 2D |
|---|------|------|:---:|:--:|
| 1 | monsters.json | 몬스터 정의 | O | O |
| 2 | npcs.json | NPC 정의 (상점/대화/퀘스트 참조 포함) | O | O |
| 3 | items.json | 아이템 정의 (종류별 속성 내장) | O | O |
| 4 | objects.json | 오브젝트 정의 (문/상자/함정/표지판 등) | O | O |
| 5 | pets.json | 펫 정의 | O | O |
| 6 | skills.json | 스킬 정의 (효과 내장) | O | O |
| 7 | quests.json | 퀘스트 정의 (단계/목표/보상 내장) | O | O |
| 8 | classes.json | 클래스 정의 (스탯/스킬 내장) | O | O |
| 9 | recipes.json | 제작법 정의 (재료 내장) | O | O |
| 10 | buffs.json | 버프/디버프 정의 (효과 내장) | O | O |
| 11 | achievements.json | 업적 정의 (조건/보상 내장) | O | O |
| 12 | loot_tables.json | 드롭 테이블 (몬스터 간 공유) | O | O |
| 13 | shops.json | 상점 정의 (NPC 간 공유) | O | O |
| 14 | dialogues.json | 대화 트리 (NPC 간 공유) | O | O |
| 15 | tile_types.json | 타일 타입 정의 | | O |
| 16 | zones/{id}.json | MUD 지역 (rooms/exits/spawns/portals) | O | |
| 17 | maps/{id}.json | 2D 맵 (layers/tiles/spawns/portals/triggers/env) | | O |

## DB 테이블 목록

| # | 테이블 | 용도 |
|---|--------|------|
| 1 | game_meta | 런타임 설정 |
| 2 | accounts | 계정 (auth_mode=account 시) |
| 3 | characters | 캐릭터 세이브 |
| 4 | guilds | 길드 |
| 5 | guild_members | 길드원 |

---

## 범위 외 (현재 설계에 미포함)

- **게시판 글 (Posts)**: 플레이어가 게시판 오브젝트에 작성하는 글. 필요 시 player.db에 `posts` 테이블 추가.
- **경매장/우편함**: 대규모 MMO 기능. 필요 시 별도 테이블 추가.
- **랭킹/리더보드**: 서버 전체 통계. 필요 시 별도 테이블 또는 외부 서비스.

---

## 엔진-게임 분리 현황

Phase 2.5에서 엔진-게임 분리를 수행했으나, 이후 Phase 4c/4d(Grid MVP)에서 "일단 동작하게" 만든 코드에 분리 위반이 남아 있다. ContentRegistry 구현 시점에 함께 정리.

### 분리 완료

| 영역 | 엔진 crate | 게임 레이어 | 패턴 |
|------|-----------|------------|------|
| ECS 컴포넌트 | scripting (ScriptComponentRegistry) | mud/script_setup.rs, Lua | trait-object 레지스트리 |
| 영속성 | persistence (PersistenceRegistry) | mud/persistence_setup.rs | trait-object 레지스트리 |
| 공간 모델 | space (SpaceModel trait) | mud (RoomGraph), Lua (Grid) | 제네릭 trait |
| 게임 로직 | scripting (Hook 시스템) | scripts/*.lua | Lua 훅 |
| 세션 관리 | session (SessionManager) | — | 엔진 레이어 |

### 분리 필요 (기술 부채)

| # | 위치 | 문제 | 해결 방향 |
|---|------|------|----------|
| 1 | `main.rs` (Grid 모드) | 접속 시 엔티티 스폰 위치(그리드 중앙), Name 컴포넌트 설정이 Rust에 하드코딩 | `on_connect` Lua 훅으로 위임. MUD 모드는 이미 Lua 훅 사용 중 |
| 2 | `main.rs` (Grid 모드) | Move 메시지 처리(dx/dy → 위치 갱신)가 Rust에 하드코딩 | `on_action` Lua 훅으로 위임 |
| 3 | `main.rs` (Grid 모드) | AOI 브로드캐스트 내용(위치+이름)이 Rust에 하드코딩 | `on_tick` 또는 전용 훅에서 Lua가 전송할 데이터 결정 |
| 4 | `net::protocol` | `ClientMessage::Connect { name }` — "이름으로 접속"은 게임 결정 | 범용 `Connect { data: Value }`로 변경, 게임이 해석 |
| 5 | `net::protocol` | `EntityWire`에 `name: String` 필드 — 엔진이 "이름" 개념을 가정 | 범용 `metadata: Value`로 변경, 게임이 채움 |
| 6 | `net::protocol` | `Welcome { grid_config }` — Grid 전용 정보가 엔진 프로토콜에 고정 | 범용 `config: Value`로 변경 |
| 7 | 콘텐츠 로딩 | ContentRegistry 미구현 (Lua 하드코딩으로 대체 중) | 동적 ContentRegistry 구현 (위 섹션 참조) |

> **우선순위**: #7(ContentRegistry) → #1~3(Grid 훅 위임) → #4~6(프로토콜 범용화) 순서.
> #1~3은 Grid 모드에 Lua 게임 스크립트를 작성하면 자연스럽게 해결된다.
> #4~6은 프로토콜 변경이므로 웹 클라이언트도 함께 수정 필요.

---

## TODO

### 콘텐츠 시스템
- [ ] ContentRegistry 구현 — 엔진 레벨 범용 로더 (`serde_json::Value` 기반)
- [ ] Lua API `content:get(collection, id)`, `content:all(collection)` 노출
- [ ] 참조 검증 — `refs.json` 설정 기반 또는 Lua `on_init`에서 검증
- [ ] 기존 Lua 하드코딩 → JSON 콘텐츠 파일 이전

### 플레이어 영속성
- [ ] player.db rusqlite 연동
- [ ] save_data 직렬화/역직렬화 (Lua table ↔ JSON ↔ DB)
- [ ] 비밀번호 해싱 라이브러리 선정 (argon2 / bcrypt)
- [ ] 댕글링 참조 검증 (로그인 시 save_data 내 콘텐츠 ID 유효성 검사)

### 엔진-게임 분리
- [ ] Grid 모드 접속/이동/해제 → Lua 훅 위임 (on_connect/on_action/on_disconnect)
- [ ] Grid 모드 AOI 브로드캐스트 → Lua 또는 설정 기반
- [ ] 프로토콜 범용화 (Connect/Welcome/EntityWire에서 게임 특화 필드 제거)

### 도구/최적화
- [ ] 게임메이커 웹 UI (JSON 파일 CRUD)
- [ ] JSON Schema 검증 (선택적, 콘텐츠 파일 타입 안전성 보완)
- [ ] 대형 맵 타일 데이터 최적화 (바이너리 포맷 검토)
