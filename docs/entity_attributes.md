# Project G — 엔티티 속성 정의서

> 작성일: 2026-02-20
> 상태: 초안

## 범례

- **필수**: 엔티티 생성 시 반드시 존재해야 함
- **선택**: 없으면 기본값 적용
- 타입 표기: `str`, `int`, `float`, `bool`, `id(대상)`, `enum(값1|값2)`, `json`, `list(타입)`

---

## 1. 공간 — MUD

### Zone (지역)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("town", "dungeon_1f") |
| name | str | O | — | 표시 이름 ("평화로운 마을") |
| description | str | | "" | 지역 설명 |
| level_min | int | | 0 | 권장 최소 레벨 |
| level_max | int | | 0 | 권장 최대 레벨 (0=무제한) |
| pvp_enabled | bool | | false | PvP 허용 여부 |
| respawn_room_id | id(Room) | | null | 이 지역 사망 시 부활 방 |

### Room (방)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("town_square") |
| zone_id | id(Zone) | O | — | 소속 지역 |
| name | str | O | — | 방 이름 ("마을 광장") |
| description | str | O | — | 방 설명 텍스트 |
| dark | bool | | false | 어둠 (조명 없으면 못 봄) |
| safe_zone | bool | | false | 안전 지대 (전투 불가) |
| underwater | bool | | false | 수중 (호흡 필요) |
| no_recall | bool | | false | 귀환 마법 사용 불가 |
| no_summon | bool | | false | 소환 불가 |
| heal_rate | float | | 1.0 | HP/MP 자연회복 배율 |
| properties | json | | {} | 기타 확장 속성 |

### Exit (출구)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| room_id | id(Room) | O | — | 소속 방 |
| direction | str | O | — | 방향 ("north", "south", "up", "portal_1") |
| target_room_id | id(Room) | O | — | 도착 방 |
| hidden | bool | | false | 숨겨진 출구 (수색 필요) |
| locked | bool | | false | 잠긴 출구 |
| key_template_id | id(Template) | | null | 필요한 열쇠 템플릿 |
| level_min | int | | 0 | 통과 최소 레벨 |
| quest_required | id(Quest) | | null | 통과 조건 퀘스트 (완료 상태) |
| message_blocked | str | | "갈 수 없다." | 통과 불가 시 메시지 |
| message_pass | str | | null | 통과 시 메시지 |

### Room Spawn (스폰 규칙)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | int | O | — | 자동 증가 |
| room_id | id(Room) | O | — | 스폰 방 |
| template_id | id(Template) | O | — | 스폰 대상 템플릿 |
| max_count | int | | 1 | 최대 동시 존재 수 |
| respawn_sec | int | | 0 | 리스폰 간격 (0=리스폰 없음) |
| active | bool | | true | 활성 여부 |
| condition | json | | null | 스폰 조건 (시간대, 이벤트 등) |

### Portal (포탈)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| source_room_id | id(Room) | O | — | 출발 방 |
| target_room_id | id(Room) | O | — | 도착 방 |
| bidirectional | bool | | false | 양방향 여부 |
| level_min | int | | 0 | 최소 레벨 |
| quest_required | id(Quest) | | null | 필요 퀘스트 |
| cost_gold | int | | 0 | 이동 비용 (골드) |
| cooldown_sec | int | | 0 | 재사용 대기 |
| message_enter | str | | null | 진입 시 메시지 |

---

## 2. 공간 — 2D

### Map (맵)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("field_01") |
| name | str | O | — | 맵 이름 ("평원 지대") |
| width | int | O | — | 맵 너비 (타일 수) |
| height | int | O | — | 맵 높이 (타일 수) |
| origin_x | int | | 0 | 원점 X |
| origin_y | int | | 0 | 원점 Y |
| pvp_enabled | bool | | false | PvP 허용 여부 |
| ambient_light | float | | 1.0 | 환경 조명 (0.0~1.0) |
| bgm | str | | null | 배경 음악 파일 경로 |
| properties | json | | {} | 기타 확장 속성 |

### Tile Type (타일 타입)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("grass", "wall") |
| name | str | O | — | 표시 이름 ("풀밭") |
| walkable | bool | | true | 이동 가능 여부 |
| sprite | str | | "" | 스프라이트 파일/프레임 |
| speed_mult | float | | 1.0 | 이동 속도 배율 (0.5=느림) |
| damage_per_sec | float | | 0 | 지속 데미지 (용암, 독 등) |
| swim_required | bool | | false | 수영 필요 |
| fly_required | bool | | false | 비행 필요 |
| transparent | bool | | true | 시야 통과 여부 |
| properties | json | | {} | 기타 확장 속성 |

### Tile Layer (타일 레이어)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| name | str | O | — | 레이어 이름 ("ground", "collision", "decoration") |
| z_order | int | O | — | 렌더 순서 (낮을수록 아래) |
| visible | bool | | true | 렌더링 여부 |

### Tile (타일)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| map_id | id(Map) | O | — | 소속 맵 |
| layer_id | id(Tile Layer) | O | — | 소속 레이어 |
| x | int | O | — | X 좌표 |
| y | int | O | — | Y 좌표 |
| tile_type_id | id(Tile Type) | O | — | 타일 타입 |

### Map Spawn (2D 스폰 규칙)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | int | O | — | 자동 증가 |
| map_id | id(Map) | O | — | 소속 맵 |
| template_id | id(Template) | O | — | 스폰 대상 템플릿 |
| x | int | O | — | 스폰 X |
| y | int | O | — | 스폰 Y |
| radius | int | | 0 | 스폰 분산 반경 (0=정확한 좌표) |
| max_count | int | | 1 | 최대 동시 존재 수 |
| respawn_sec | int | | 0 | 리스폰 간격 |
| active | bool | | true | 활성 여부 |
| condition | json | | null | 스폰 조건 |

### Map Portal (2D 포탈)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| x | int | O | — | 포탈 X |
| y | int | O | — | 포탈 Y |
| target_map_id | id(Map) | O | — | 도착 맵 |
| target_x | int | O | — | 도착 X |
| target_y | int | O | — | 도착 Y |
| width | int | | 1 | 포탈 영역 너비 |
| height | int | | 1 | 포탈 영역 높이 |
| bidirectional | bool | | false | 양방향 |
| sprite | str | | null | 포탈 시각 효과 |
| level_min | int | | 0 | 최소 레벨 |

### Map Trigger (트리거 영역)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| x | int | O | — | 영역 시작 X |
| y | int | O | — | 영역 시작 Y |
| width | int | O | — | 영역 너비 |
| height | int | O | — | 영역 높이 |
| event_type | enum(script\|cutscene\|dialogue\|combat) | O | — | 발동 이벤트 종류 |
| event_data | json | O | — | 이벤트 상세 데이터 |
| once | bool | | false | 1회만 발동 |
| condition | json | | null | 발동 조건 |

---

## 3. 캐릭터 — 공용

### Player (플레이어)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| name | str | O | — | 캐릭터 이름 (고유) |
| level | int | | 1 | 현재 레벨 |
| exp | int | | 0 | 현재 경험치 |
| exp_next | int | | 100 | 다음 레벨 필요 경험치 |
| class_id | id(Class) | | null | 직업 (null=무직) |
| hp_current | int | O | — | 현재 HP |
| hp_max | int | O | — | 최대 HP |
| mp_current | int | | 0 | 현재 MP |
| mp_max | int | | 0 | 최대 MP |
| attack | int | O | — | 기본 공격력 |
| defense | int | O | — | 기본 방어력 |
| magic_attack | int | | 0 | 마법 공격력 |
| magic_defense | int | | 0 | 마법 방어력 |
| speed | int | | 10 | 이동/행동 속도 |
| critical_rate | float | | 0.05 | 크리티컬 확률 (0.0~1.0) |
| critical_damage | float | | 1.5 | 크리티컬 데미지 배율 |
| gold | int | | 0 | 소지 골드 |
| inventory_capacity | int | | 20 | 인벤토리 최대 슬롯 |
| dead | bool | | false | 사망 상태 |
| play_time | int | | 0 | 누적 플레이 시간 (초) |
| — 2D 전용 — | | | | |
| map_id | id(Map) | | null | 현재 맵 |
| x | int | | 0 | 현재 X |
| y | int | | 0 | 현재 Y |
| sprite | str | | "default" | 스프라이트시트 경로 |
| direction | enum(up\|down\|left\|right) | | down | 바라보는 방향 |
| — MUD 전용 — | | | | |
| room_id | id(Room) | | null | 현재 방 |

### Equipment (장비 슬롯)

플레이어에 소속. 슬롯별 아이템 참조.

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| player_id | id(Player) | O | — | 소속 플레이어 |
| slot | enum(weapon\|head\|body\|legs\|feet\|hands\|acc1\|acc2) | O | — | 장비 슬롯 |
| item_id | id(Item) | | null | 착용 아이템 (null=비어있음) |

### Active Buff (활성 버프/디버프)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| entity_id | id(캐릭터) | O | — | 버프 대상 |
| buff_def_id | id(Buff Definition) | O | — | 버프 정의 참조 |
| remaining_sec | float | O | — | 남은 시간 (초) |
| stacks | int | | 1 | 현재 중첩 수 |
| source_id | id(캐릭터) | | null | 부여자 (PvP 판별용) |

### Learned Skill (습득 스킬)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| player_id | id(Player) | O | — | 소속 플레이어 |
| skill_id | id(Skill) | O | — | 스킬 정의 참조 |
| skill_level | int | | 1 | 스킬 레벨 |
| cooldown_remaining | float | | 0 | 남은 쿨다운 (초) |

### Quest Progress (퀘스트 진행)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| player_id | id(Player) | O | — | 소속 플레이어 |
| quest_id | id(Quest) | O | — | 퀘스트 참조 |
| stage_index | int | | 0 | 현재 단계 인덱스 |
| objective_progress | json | O | — | 목표별 달성도 {"kill_goblin": 3, "collect_herb": 5} |
| status | enum(active\|completed\|failed) | | active | 진행 상태 |
| accepted_at | str | O | — | 수락 시각 |

### Friend Entry (친구)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| player_id | id(Player) | O | — | 소속 플레이어 |
| friend_name | str | O | — | 친구 캐릭터 이름 |
| note | str | | "" | 메모 |
| added_at | str | O | — | 등록 시각 |

### NPC

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 템플릿 ID |
| name | str | O | — | 이름 ("무기 상인 그레고리") |
| role | enum(merchant\|trainer\|quest_giver\|banker\|guard\|generic) | O | — | 역할 |
| greeting | str | | "" | 처음 상호작용 시 대사 |
| immortal | bool | | true | 불사 여부 |
| — 2D 전용 — | | | | |
| sprite | str | | "npc_default" | 스프라이트 |
| direction | enum(up\|down\|left\|right) | | down | 바라보는 방향 |

### Dialogue (대화 트리)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 대화 트리 ID |
| npc_id | id(NPC) | O | — | 소속 NPC |

### Dialogue Node (대화 노드)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 노드 ID |
| dialogue_id | id(Dialogue) | O | — | 소속 대화 트리 |
| speaker | str | | "" | 화자 이름 (NPC 이름 또는 "system") |
| text | str | O | — | 대사 텍스트 |
| condition | json | | null | 이 노드 표시 조건 |

### Dialogue Choice (대화 선택지)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| node_id | id(Dialogue Node) | O | — | 소속 노드 |
| order | int | O | — | 표시 순서 |
| text | str | O | — | 선택지 텍스트 |
| next_node_id | id(Dialogue Node) | | null | 다음 노드 (null=대화 종료) |
| action | json | | null | 선택 시 효과 {"give_quest":"quest_01"} |
| condition | json | | null | 이 선택지 표시 조건 |

### Shop (상점)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 상점 ID |
| npc_id | id(NPC) | O | — | 소속 NPC |
| buy_rate | float | | 1.0 | 구매가 배율 |
| sell_rate | float | | 0.5 | 판매가 배율 (아이템 기본가 대비) |

### Shop Entry (판매 항목)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| shop_id | id(Shop) | O | — | 소속 상점 |
| template_id | id(Template) | O | — | 아이템 템플릿 |
| price_override | int | | null | 가격 오버라이드 (null=템플릿 기본가 × buy_rate) |
| stock | int | | -1 | 재고 (-1=무한) |
| level_required | int | | 0 | 구매 최소 레벨 |

### Trainable Skill (훈련 가능 스킬)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| npc_id | id(NPC) | O | — | 소속 NPC (trainer) |
| skill_id | id(Skill) | O | — | 스킬 정의 |
| cost_gold | int | | 0 | 습득 비용 |
| level_required | int | | 0 | 습득 최소 레벨 |
| class_required | id(Class) | | null | 필요 클래스 |
| prerequisite_skill | id(Skill) | | null | 선행 스킬 |

### Monster (몬스터)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 템플릿 ID |
| name | str | O | — | 이름 ("고블린 전사") |
| grade | enum(normal\|elite\|boss\|raid) | | normal | 등급 |
| level | int | | 1 | 레벨 |
| hp_max | int | O | — | 최대 HP |
| mp_max | int | | 0 | 최대 MP |
| attack | int | O | — | 공격력 |
| defense | int | O | — | 방어력 |
| magic_attack | int | | 0 | 마법 공격력 |
| magic_defense | int | | 0 | 마법 방어력 |
| speed | int | | 10 | 이동/행동 속도 |
| exp_reward | int | | 0 | 처치 시 경험치 |
| gold_reward_min | int | | 0 | 최소 드롭 골드 |
| gold_reward_max | int | | 0 | 최대 드롭 골드 |
| aggressive | bool | | false | 선공 여부 |
| assist | bool | | false | 주변 동일 종 도움 여부 |
| flee_hp_percent | float | | 0 | 도주 HP 비율 (0=도주 안 함) |
| — 2D 전용 — | | | | |
| sprite | str | | "mob_default" | 스프라이트 |
| aggro_radius | int | | 5 | 어그로 감지 반경 |
| chase_radius | int | | 10 | 추적 최대 반경 |
| return_radius | int | | 15 | 복귀 반경 (스폰 지점 기준) |
| move_speed | float | | 1.0 | 이동 속도 배율 |

### AI Behavior (행동 패턴)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| monster_id | id(Monster) | O | — | 소속 몬스터 |
| ai_type | enum(passive\|aggressive\|patrol\|stationary\|scripted) | | passive | AI 타입 |
| attack_style | enum(melee\|ranged\|magic\|hybrid) | | melee | 공격 방식 |
| skill_use_chance | float | | 0.3 | 틱당 스킬 사용 확률 |
| target_priority | enum(nearest\|lowest_hp\|highest_damage\|random) | | nearest | 타겟 우선순위 |
| script_id | str | | null | 커스텀 AI 스크립트 (Lua) |

### Loot Table (드롭 테이블)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 드롭 테이블 ID |
| monster_id | id(Monster) | O | — | 소속 몬스터 |

### Loot Entry (드롭 항목)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| loot_table_id | id(Loot Table) | O | — | 소속 테이블 |
| template_id | id(Template) | O | — | 아이템 템플릿 |
| chance | float | O | — | 드롭 확률 (0.0~1.0) |
| quantity_min | int | | 1 | 최소 수량 |
| quantity_max | int | | 1 | 최대 수량 |

### Patrol Path (순찰 경로) — 2D 전용

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 경로 ID |
| entity_id | id(Monster\|NPC) | O | — | 소속 엔티티 |
| loop | bool | | true | 순환 반복 여부 |

### Waypoint (경유지) — 2D 전용

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| path_id | id(Patrol Path) | O | — | 소속 경로 |
| order | int | O | — | 순서 |
| x | int | O | — | X 좌표 |
| y | int | O | — | Y 좌표 |
| wait_sec | float | | 0 | 도착 후 대기 시간 |

### Pet (펫)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 템플릿 ID |
| name | str | O | — | 펫 이름 |
| owner_id | id(Player) | O | — | 소유자 |
| level | int | | 1 | 펫 레벨 |
| hp_max | int | O | — | 최대 HP |
| attack | int | | 0 | 공격력 |
| defense | int | | 0 | 방어력 |
| loyalty | int | | 100 | 충성도 (0~100) |
| summoned | bool | | false | 소환 상태 |
| — 2D 전용 — | | | | |
| sprite | str | | "pet_default" | 스프라이트 |
| follow_distance | int | | 2 | 주인과의 추적 거리 |

---

## 4. 아이템 — 공용

### Item 공통 속성

모든 아이템 종류(kind)가 공유하는 속성.

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 템플릿 ID ("iron_sword") |
| kind | enum(weapon\|armor\|accessory\|consumable\|material\|quest\|key\|container\|currency) | O | — | 아이템 종류 |
| name | str | O | — | 표시 이름 ("철검") |
| description | str | | "" | 설명 텍스트 |
| rarity | enum(common\|uncommon\|rare\|epic\|legendary) | | common | 희귀도 |
| base_price | int | | 0 | 기본 가격 (골드) |
| weight | float | | 0 | 무게 (0=무게 시스템 미사용 시 무시) |
| stackable | bool | | false | 중첩 가능 여부 |
| max_stack | int | | 1 | 최대 중첩 수 (stackable=true일 때) |
| level_required | int | | 0 | 착용/사용 최소 레벨 |
| class_required | id(Class) | | null | 착용/사용 필요 클래스 |
| tradeable | bool | | true | 거래 가능 여부 |
| droppable | bool | | true | 바닥에 버리기 가능 여부 |
| — 2D 전용 — | | | | |
| icon | str | | null | 인벤토리 아이콘 |
| ground_sprite | str | | null | 바닥 드롭 스프라이트 |

### Weapon (무기) — kind=weapon 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| weapon_type | enum(sword\|axe\|mace\|dagger\|bow\|staff\|wand) | O | — | 무기 종류 |
| attack_bonus | int | O | — | 공격력 보너스 |
| magic_attack_bonus | int | | 0 | 마법 공격력 보너스 |
| attack_speed | float | | 1.0 | 공격 속도 배율 |
| critical_bonus | float | | 0 | 크리티컬 확률 추가 |
| range | int | | 1 | 사거리 (1=근접) |
| element | enum(none\|fire\|ice\|lightning\|poison\|holy\|dark) | | none | 속성 |
| durability_max | int | | 0 | 최대 내구도 (0=내구도 없음) |

### Armor (방어구) — kind=armor 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| slot | enum(head\|body\|legs\|feet\|hands) | O | — | 장비 슬롯 |
| defense_bonus | int | O | — | 방어력 보너스 |
| magic_defense_bonus | int | | 0 | 마법 방어력 보너스 |
| hp_bonus | int | | 0 | HP 보너스 |
| mp_bonus | int | | 0 | MP 보너스 |
| speed_bonus | int | | 0 | 속도 보너스 |
| element_resist | enum(none\|fire\|ice\|lightning\|poison\|holy\|dark) | | none | 속성 저항 |
| durability_max | int | | 0 | 최대 내구도 |

### Accessory (악세서리) — kind=accessory 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| slot | enum(acc1\|acc2) | O | — | 장비 슬롯 |
| effects | json | O | — | 효과 목록 [{"stat":"hp_max","value":20}, {"stat":"critical_rate","value":0.05}] |

### Consumable (소비품) — kind=consumable 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| heal_hp | int | | 0 | HP 회복량 |
| heal_mp | int | | 0 | MP 회복량 |
| heal_hp_percent | float | | 0 | HP 비율 회복 (0.0~1.0) |
| heal_mp_percent | float | | 0 | MP 비율 회복 |
| buff_id | id(Buff Definition) | | null | 부여 버프 |
| buff_duration | float | | 0 | 버프 지속 시간 (초) |
| cooldown_sec | float | | 0 | 재사용 대기 (초) |
| cooldown_group | str | | null | 쿨다운 공유 그룹 ("potion" 등) |

### Material (재료) — kind=material 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| craft_type | str | | "general" | 제작 분류 ("metal", "herb", "leather") |
| grade | int | | 1 | 등급 (높을수록 고급 재료) |

### Quest Item — kind=quest 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| quest_id | id(Quest) | O | — | 관련 퀘스트 |

※ tradeable=false, droppable=false 강제.

### Key (열쇠) — kind=key 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| target_ids | list(str) | O | — | 열 수 있는 문/상자 ID 목록 |
| consumed_on_use | bool | | false | 사용 시 소멸 여부 |

### Container (가방) — kind=container 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| capacity | int | O | — | 내부 슬롯 수 |

### Currency (화폐) — kind=currency 추가 속성

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| currency_type | str | O | — | 화폐 종류 ("gold", "gem", "event_coin") |

---

## 5. 오브젝트 — 공용

### Door (문)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| name | str | | "문" | 표시 이름 |
| state | enum(open\|closed\|locked) | | closed | 현재 상태 |
| key_template_id | id(Template) | | null | 필요 열쇠 (null=열쇠 불필요) |
| auto_close_sec | int | | 0 | 자동 닫힘 시간 (0=안 닫힘) |
| — MUD — | | | | |
| exit_direction | str | | null | 연결된 출구 방향 |
| — 2D — | | | | |
| x | int | | 0 | X 좌표 |
| y | int | | 0 | Y 좌표 |
| sprite_open | str | | null | 열린 스프라이트 |
| sprite_closed | str | | null | 닫힌 스프라이트 |

### Chest (상자)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| name | str | | "상자" | 표시 이름 |
| locked | bool | | false | 잠김 여부 |
| key_template_id | id(Template) | | null | 필요 열쇠 |
| capacity | int | | 10 | 내부 슬롯 수 |
| respawn_sec | int | | 0 | 내용물 리필 시간 (0=1회성) |
| — 2D — | | | | |
| x | int | | 0 | X 좌표 |
| y | int | | 0 | Y 좌표 |
| sprite | str | | null | 스프라이트 |

### Chest Item (상자 내부 아이템)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| chest_id | id(Chest) | O | — | 소속 상자 |
| template_id | id(Template) | O | — | 아이템 템플릿 |
| quantity | int | | 1 | 수량 |
| chance | float | | 1.0 | 출현 확률 (0.0~1.0) |

### Trap (함정)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| name | str | | "함정" | 표시 이름 |
| active | bool | | true | 활성 상태 |
| hidden | bool | | true | 숨김 여부 (탐지 필요) |
| damage | int | | 0 | 데미지 |
| debuff_id | id(Buff Definition) | | null | 부여 디버프 (독, 감속 등) |
| trigger_type | enum(step\|proximity\|timed) | | step | 발동 방식 |
| rearm_sec | int | | 0 | 재발동 대기 (0=1회성) |
| detect_difficulty | int | | 10 | 탐지 난이도 |
| disarm_difficulty | int | | 10 | 해제 난이도 |
| — 2D — | | | | |
| x | int | | 0 | X 좌표 |
| y | int | | 0 | Y 좌표 |

### Sign (표지판)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| text | str | O | — | 표지판 내용 |
| — 2D — | | | | |
| x | int | | 0 | X 좌표 |
| y | int | | 0 | Y 좌표 |
| sprite | str | | null | 스프라이트 |

### Campfire (모닥불)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| heal_rate | float | | 2.0 | HP 회복 배율 |
| radius | int | | 3 | 효과 범위 (2D: 타일 수, MUD: 같은 방) |
| — 2D — | | | | |
| x | int | | 0 | X 좌표 |
| y | int | | 0 | Y 좌표 |
| sprite | str | | null | 스프라이트 |

### Crafting Station (제작대)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| name | str | O | — | 이름 ("대장간", "연금술 탁자") |
| craft_type | str | O | — | 제작 분류 ("forge", "alchemy", "cooking") |
| — 2D — | | | | |
| x | int | | 0 | X 좌표 |
| y | int | | 0 | Y 좌표 |
| sprite | str | | null | 스프라이트 |

### Bulletin Board (게시판)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| name | str | | "게시판" | 표시 이름 |
| max_posts | int | | 50 | 최대 게시글 수 |

### Post (게시글)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| board_id | id(Bulletin Board) | O | — | 소속 게시판 |
| author | str | O | — | 작성자 이름 |
| title | str | O | — | 제목 |
| body | str | O | — | 본문 |
| created_at | str | O | — | 작성 시각 |

---

## 6. 시스템 — 공용

### Quest (퀘스트)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("slay_goblin_king") |
| name | str | O | — | 퀘스트 이름 ("고블린 왕 토벌") |
| description | str | O | — | 퀘스트 설명 |
| level_min | int | | 0 | 수락 최소 레벨 |
| level_max | int | | 0 | 수락 최대 레벨 (0=무제한) |
| prerequisite_quest | id(Quest) | | null | 선행 퀘스트 (완료 필요) |
| repeatable | bool | | false | 반복 수행 가능 |
| time_limit_sec | int | | 0 | 시간 제한 (0=무제한) |
| reward_exp | int | | 0 | 보상 경험치 |
| reward_gold | int | | 0 | 보상 골드 |
| reward_items | json | | [] | 보상 아이템 [{"template_id":"iron_sword","quantity":1}] |
| class_required | id(Class) | | null | 필요 클래스 |

### Quest Stage (퀘스트 단계)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| quest_id | id(Quest) | O | — | 소속 퀘스트 |
| stage_index | int | O | — | 단계 순서 (0부터) |
| description | str | | "" | 단계 설명 |

### Quest Objective (퀘스트 목표)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| quest_id | id(Quest) | O | — | 소속 퀘스트 |
| stage_index | int | O | — | 소속 단계 |
| objective_type | enum(kill\|collect\|visit\|talk\|use\|escort\|craft) | O | — | 목표 타입 |
| target_id | str | O | — | 대상 (몬스터 ID, 아이템 ID, 방/맵 ID, NPC ID) |
| quantity | int | | 1 | 필요 수량 |
| description | str | | "" | 목표 설명 텍스트 |

### Skill (스킬 정의)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("fireball") |
| name | str | O | — | 스킬 이름 ("화염구") |
| description | str | O | — | 스킬 설명 |
| type | enum(active\|passive) | O | — | 액티브/패시브 |
| max_level | int | | 5 | 최대 스킬 레벨 |
| mp_cost | int | | 0 | 마나 소모 (레벨 1 기준) |
| mp_cost_per_level | int | | 0 | 레벨당 추가 마나 소모 |
| cooldown_sec | float | | 0 | 쿨다운 (초) |
| cast_time_sec | float | | 0 | 시전 시간 (초, 0=즉시) |
| range | int | | 1 | 사거리 |
| target_type | enum(self\|single_enemy\|single_ally\|aoe_enemy\|aoe_ally\|aoe_all) | | single_enemy | 대상 타입 |
| aoe_radius | int | | 0 | AoE 반경 (target_type=aoe일 때) |
| — 2D 전용 — | | | | |
| icon | str | | null | 스킬 아이콘 |
| animation | str | | null | 시전 애니메이션 |
| projectile_sprite | str | | null | 투사체 스프라이트 (있으면 투사체 생성) |
| projectile_speed | float | | 0 | 투사체 속도 |

### Skill Effect (스킬 효과)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| skill_id | id(Skill) | O | — | 소속 스킬 |
| effect_type | enum(damage\|heal\|buff\|debuff\|dot\|hot\|dispel\|teleport\|summon) | O | — | 효과 타입 |
| base_value | int | | 0 | 기본 수치 (레벨 1) |
| per_level_value | int | | 0 | 레벨당 추가 수치 |
| element | enum(none\|fire\|ice\|lightning\|poison\|holy\|dark) | | none | 속성 |
| buff_id | id(Buff Definition) | | null | 부여할 버프/디버프 |
| duration_sec | float | | 0 | 효과 지속 시간 |
| scaling_stat | enum(none\|attack\|magic_attack) | | none | 스케일링 스탯 |
| scaling_ratio | float | | 0 | 스케일링 비율 (0.5 = 스탯의 50% 추가) |

### Class (클래스 정의)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("warrior") |
| name | str | O | — | 클래스 이름 ("전사") |
| description | str | | "" | 설명 |
| base_hp | int | O | — | 초기 HP |
| base_mp | int | | 0 | 초기 MP |
| base_attack | int | O | — | 초기 공격력 |
| base_defense | int | O | — | 초기 방어력 |
| base_magic_attack | int | | 0 | 초기 마법 공격력 |
| base_magic_defense | int | | 0 | 초기 마법 방어력 |
| base_speed | int | | 10 | 초기 속도 |
| hp_per_level | int | O | — | 레벨당 HP 증가 |
| mp_per_level | int | | 0 | 레벨당 MP 증가 |
| attack_per_level | int | O | — | 레벨당 공격력 증가 |
| defense_per_level | int | O | — | 레벨당 방어력 증가 |
| magic_attack_per_level | int | | 0 | 레벨당 마법 공격력 증가 |
| magic_defense_per_level | int | | 0 | 레벨당 마법 방어력 증가 |
| equip_weapon_types | list(str) | | ["sword"] | 장비 가능 무기 종류 |
| equip_armor_weight | enum(cloth\|leather\|mail\|plate) | | cloth | 장비 가능 방어구 등급 |
| — 2D 전용 — | | | | |
| sprite | str | | null | 클래스별 스프라이트시트 |

### Learnable Skill (클래스 습득 가능 스킬)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| class_id | id(Class) | O | — | 소속 클래스 |
| skill_id | id(Skill) | O | — | 스킬 |
| learn_level | int | O | — | 습득 가능 레벨 |
| auto_learn | bool | | false | 레벨 도달 시 자동 습득 |

### Recipe (제작법)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("iron_sword_recipe") |
| name | str | O | — | 제작법 이름 ("철검 제작") |
| craft_type | str | O | — | 제작 분류 ("forge") |
| station_required | str | | null | 필요 제작대 craft_type |
| level_required | int | | 0 | 필요 레벨 |
| skill_required | id(Skill) | | null | 필요 스킬 (예: "대장장이") |
| success_rate | float | | 1.0 | 성공 확률 (0.0~1.0) |
| result_template_id | id(Template) | O | — | 결과 아이템 |
| result_quantity | int | | 1 | 결과 수량 |

### Recipe Ingredient (제작 재료)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| recipe_id | id(Recipe) | O | — | 소속 제작법 |
| template_id | id(Template) | O | — | 재료 아이템 템플릿 |
| quantity | int | O | — | 필요 수량 |

### Achievement (업적)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| name | str | O | — | 업적 이름 ("고블린 학살자") |
| description | str | O | — | 설명 |
| reward_exp | int | | 0 | 보상 경험치 |
| reward_gold | int | | 0 | 보상 골드 |
| reward_title | str | | null | 보상 칭호 |
| reward_item_id | id(Template) | | null | 보상 아이템 |
| hidden | bool | | false | 달성 전 숨김 |

### Achievement Condition (업적 조건)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| achievement_id | id(Achievement) | O | — | 소속 업적 |
| condition_type | enum(kill_count\|collect_count\|quest_clear\|level_reach\|visit\|craft_count\|play_time) | O | — | 조건 타입 |
| target_id | str | | null | 대상 (몬스터/아이템/퀘스트/방 ID) |
| quantity | int | | 1 | 필요 수치 |

### Guild (길드)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | int | O | — | 자동 증가 |
| name | str | O | — | 길드 이름 (고유) |
| description | str | | "" | 길드 소개 |
| leader_id | id(Player) | O | — | 길드장 |
| max_members | int | | 50 | 최대 인원 |
| level | int | | 1 | 길드 레벨 |
| exp | int | | 0 | 길드 경험치 |
| gold | int | | 0 | 길드 자금 |
| created_at | str | O | — | 창설 시각 |

### Guild Member (길드원)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| guild_id | id(Guild) | O | — | 소속 길드 |
| player_id | id(Player) | O | — | 플레이어 |
| rank | enum(master\|officer\|member) | | member | 직급 |
| joined_at | str | O | — | 가입 시각 |
| contribution | int | | 0 | 기여도 |

### Party (파티)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | int | O | — | 자동 증가 |
| leader_id | id(Player) | O | — | 파티장 |
| max_members | int | | 4 | 최대 인원 |
| loot_rule | enum(free_for_all\|round_robin\|leader) | | free_for_all | 전리품 분배 방식 |

### Party Member (파티원)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| party_id | id(Party) | O | — | 소속 파티 |
| player_id | id(Player) | O | — | 플레이어 |

---

## 7. 버프/이펙트 — 공용

### Buff Definition (버프 정의)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 ("poison", "strength_up") |
| name | str | O | — | 표시 이름 ("독", "힘 증가") |
| description | str | | "" | 설명 |
| category | enum(buff\|debuff) | O | — | 분류 |
| duration_sec | float | O | — | 기본 지속 시간 (초) |
| max_stacks | int | | 1 | 최대 중첩 수 |
| tick_interval_sec | float | | 0 | 효과 틱 간격 (DoT/HoT용, 0=즉시 효과만) |
| dispellable | bool | | true | 해제 가능 여부 |
| persist_on_death | bool | | false | 사망 시 유지 여부 |
| — 2D 전용 — | | | | |
| icon | str | | null | 버프 아이콘 |
| particle | str | | null | 적용 시 파티클 |

### Buff Effect (버프 효과)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| buff_def_id | id(Buff Definition) | O | — | 소속 버프 정의 |
| effect_type | enum(stat_modify\|dot\|hot\|immunity\|speed_modify\|stun\|silence\|root) | O | — | 효과 타입 |
| target_stat | str | | null | 대상 스탯 ("attack", "defense", "speed" 등) |
| value | float | O | — | 수치 (절대값 또는 비율) |
| is_percent | bool | | false | true면 비율(%), false면 절대값 |
| per_stack | bool | | true | 중첩당 효과 적용 여부 |

---

## 8. 전투/이펙트 — 2D 전용

### Projectile (투사체)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| source_id | id(캐릭터) | O | — | 발사자 |
| skill_id | id(Skill) | | null | 원본 스킬 |
| x | float | O | — | 현재 X |
| y | float | O | — | 현재 Y |
| dx | float | O | — | 방향 X (정규화) |
| dy | float | O | — | 방향 Y (정규화) |
| speed | float | O | — | 이동 속도 (타일/초) |
| damage | int | O | — | 데미지 |
| element | enum(none\|fire\|ice\|lightning\|poison\|holy\|dark) | | none | 속성 |
| pierce | bool | | false | 관통 여부 |
| max_range | float | O | — | 최대 사거리 |
| traveled | float | | 0 | 이동 거리 |
| sprite | str | O | — | 스프라이트 |
| hit_effect | str | | null | 적중 파티클 |

### AoE Zone (범위 효과 지역)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| source_id | id(캐릭터) | O | — | 생성자 |
| x | int | O | — | 중심 X |
| y | int | O | — | 중심 Y |
| radius | int | O | — | 반경 |
| duration_sec | float | O | — | 지속 시간 |
| remaining_sec | float | O | — | 남은 시간 |
| tick_interval_sec | float | | 1.0 | 효과 틱 간격 |
| effect_type | enum(damage\|heal\|slow\|buff\|debuff) | O | — | 효과 종류 |
| value | int | O | — | 틱당 수치 |
| element | enum(none\|fire\|ice\|lightning\|poison\|holy\|dark) | | none | 속성 |
| affects | enum(enemy\|ally\|all) | | enemy | 대상 |
| sprite | str | | null | 지역 시각 효과 |

---

## 9. 환경 — 2D 전용

### Light Source (광원)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| x | int | O | — | X 좌표 |
| y | int | O | — | Y 좌표 |
| radius | float | O | — | 광원 반경 |
| color | str | | "#ffffff" | 색상 (hex) |
| intensity | float | | 1.0 | 강도 (0.0~1.0) |
| flicker | bool | | false | 깜빡임 효과 |
| dynamic | bool | | false | 동적 광원 (성능 비용 높음) |

### Weather Zone (날씨 영역)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| weather_type | enum(rain\|snow\|fog\|storm\|sandstorm) | O | — | 날씨 종류 |
| intensity | float | | 0.5 | 강도 (0.0~1.0) |
| speed_penalty | float | | 0 | 이동 속도 감소 비율 |
| visibility_mult | float | | 1.0 | 시야 배율 |
| damage_per_sec | float | | 0 | 지속 데미지 (폭풍 등) |

### Particle Emitter (파티클 생성기)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| x | int | O | — | X 좌표 |
| y | int | O | — | Y 좌표 |
| particle_type | str | O | — | 타입 ("fire", "smoke", "sparkle") |
| rate | float | | 10 | 초당 생성 수 |
| radius | float | | 1 | 분산 반경 |
| lifetime_sec | float | | 2.0 | 파티클 수명 |

### Sound Emitter (사운드 생성기)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | str | O | — | 고유 식별자 |
| map_id | id(Map) | O | — | 소속 맵 |
| x | int | O | — | X 좌표 |
| y | int | O | — | Y 좌표 |
| sound_file | str | O | — | 사운드 파일 경로 |
| radius | float | O | — | 가청 반경 |
| volume | float | | 1.0 | 볼륨 (0.0~1.0) |
| loop | bool | | true | 반복 재생 |

---

## 10. 계정/캐릭터 — 공용

### Account (계정) — auth_mode="account"일 때

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | int | O | — | 자동 증가 |
| username | str | O | — | 로그인 ID (고유) |
| password_hash | str | O | — | 비밀번호 해시 |
| created_at | str | O | — | 가입 시각 |
| banned | bool | | false | 차단 여부 |
| ban_reason | str | | null | 차단 사유 |
| ban_until | str | | null | 차단 해제 시각 (null=영구) |
| last_login | str | | null | 마지막 로그인 |
| last_ip | str | | null | 마지막 접속 IP |
| max_characters | int | | 3 | 최대 캐릭터 수 |

### Character (저장 캐릭터)

| 속성 | 타입 | 필수 | 기본값 | 설명 |
|------|------|:----:|--------|------|
| id | int | O | — | 자동 증가 |
| account_id | int | | null | 소속 계정 (character 모드: null) |
| name | str | O | — | 캐릭터 이름 (고유) |
| password_hash | str | | null | 비밀번호 (character 모드에서 사용) |
| save_data | json | O | {} | ECS 컴포넌트 전체 스냅샷 |
| last_login | str | | null | 마지막 접속 시각 |
| play_time | int | | 0 | 누적 플레이 시간 (초) |
| created_at | str | O | — | 생성 시각 |
