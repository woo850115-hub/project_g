# Project G — 엔티티 정의서

> 작성일: 2026-02-20
> 상태: 초안

들여쓰기(└─)는 부모 엔티티에 **소속**되는 하위 엔티티를 나타낸다.
소속 관계는 ECS에서 참조 컴포넌트(FK)로 표현된다.

---

## MUD 엔티티

### 1. 공간

```
Zone (지역)
│   그룹 단위. 마을, 숲, 던전 1층 등.
│
└─ Room (방)
   │   플레이어가 존재하는 이산 공간 단위.
   │   이름, 설명, 속성(어둠/안전지대 등).
   │
   ├─ Exit (출구)
   │     방과 방을 연결. 방향(north/south/up...) + 대상 방.
   │     조건부 가능 (열쇠 필요, 레벨 제한).
   │
   ├─ Room Spawn (스폰 규칙)
   │     이 방에 어떤 템플릿을 몇 마리, 몇 초마다 리스폰할지.
   │
   └─ Portal (포탈)
         다른 존/방으로의 특수 이동. 일방향/양방향.
```

### 2. 캐릭터

```
Player (플레이어)
│   유저가 조작하는 캐릭터. 계정과 1:1(MUD) 또는 N:1(2D).
│
├─ Equipment (장비 슬롯)
│   │   착용 중인 아이템 참조. 슬롯별 1개.
│   │
│   ├─ Weapon Slot (무기)
│   ├─ Head Slot (머리)
│   ├─ Body Slot (몸통)
│   ├─ Legs Slot (다리)
│   ├─ Feet Slot (발)
│   ├─ Hands Slot (손)
│   ├─ Accessory Slot 1 (악세서리)
│   └─ Accessory Slot 2 (악세서리)
│
├─ Inventory (인벤토리)
│   │   소지 아이템 목록.
│   │
│   └─ Item (아이템) × N
│
├─ Active Buff (활성 버프/디버프) × N
│     효과 종류, 남은 시간, 중첩 횟수.
│
├─ Skill List (습득 스킬) × N
│     스킬 참조, 쿨다운 상태, 레벨.
│
├─ Quest Log (퀘스트 진행) × N
│     퀘스트 참조, 현재 단계, 목표 달성도.
│
└─ Friend List (친구 목록) × N
      대상 캐릭터 참조, 메모.
```

```
NPC (비전투 NPC)
│   상인, 교관, 퀘스트 발주자, 은행원, 경비병 등.
│   역할(role)에 따라 하위 데이터가 달라짐.
│
├─ Dialogue (대화 트리)
│   │   NPC와의 대화 분기.
│   │
│   └─ Dialogue Node (대화 노드) × N
│       │   대사 텍스트, 선택지.
│       │
│       └─ Dialogue Choice (선택지) × N
│             다음 노드 참조, 조건(퀘스트 상태/아이템 보유 등).
│
├─ Shop (상점) — role=merchant일 때
│   │
│   └─ Shop Entry (판매 항목) × N
│         아이템 템플릿 참조, 가격, 재고(-1=무한).
│
├─ Train List (훈련 목록) — role=trainer일 때
│   │
│   └─ Trainable Skill (훈련 가능 스킬) × N
│         스킬 참조, 비용, 선행 조건.
│
└─ Quest Offer (퀘스트 제공) × N
      퀘스트 참조, 수락 조건.
```

```
Monster (몬스터)
│   적대 전투 대상. 일반/정예/보스 등급.
│
├─ Loot Table (드롭 테이블)
│   │
│   └─ Loot Entry (드롭 항목) × N
│         아이템 템플릿 참조, 확률(%), 수량 범위.
│
├─ Skill List (사용 스킬) × N
│     AI가 사용하는 스킬. 사용 조건(HP 50% 이하 등).
│
└─ AI Behavior (행동 패턴)
      공격 방식, 어그로 규칙, 도주 조건.
```

```
Pet (펫)
│   플레이어 소유 동반자.
│
├─ Owner (소유자)
│     플레이어 참조.
│
└─ Skill List (펫 스킬) × N
```

### 3. 아이템

```
Item (아이템)
│   인벤토리/장비/바닥/상자에 존재.
│   종류(kind)에 따라 하위 속성이 달라짐.
│
├─ Weapon (무기) — kind=weapon
│     공격력, 공격 속도, 무기 타입(검/창/활...).
│
├─ Armor (방어구) — kind=armor
│     방어력, 슬롯(head/body/legs/feet/hands).
│
├─ Accessory (악세서리) — kind=accessory
│     특수 효과(HP+10, 크리티컬+5% 등).
│
├─ Consumable (소비품) — kind=consumable
│     사용 효과(HP 회복, 버프 부여), 중첩 수량.
│
├─ Material (재료) — kind=material
│     제작 재료, 중첩 수량.
│
├─ Quest Item (퀘스트 아이템) — kind=quest
│     퀘스트 참조, 거래/드롭 불가.
│
├─ Key (열쇠) — kind=key
│     대상 문/상자 참조.
│
├─ Container (가방) — kind=container
│   │   인벤토리 확장.
│   │
│   └─ Item (내부 아이템) × N
│
└─ Currency (화폐) — kind=currency
      골드, 특수 재화. 중첩 수량.
```

### 4. 오브젝트 (방 내 설치물)

```
Door (문)
│   열림/닫힘/잠김 상태. 통과 조건.
│
└─ Required Key (필요 열쇠)
      열쇠 아이템 템플릿 참조.

Chest (상자)
│   열기/잠금 상태. 아이템 보관.
│
├─ Required Key (필요 열쇠)
│
└─ Item (내부 아이템) × N

Trap (함정)
      활성/비활성 상태. 데미지, 상태이상 효과.
      발동 조건 (밟기, 시간 등).

Sign (표지판)
      읽기 텍스트.

Campfire (모닥불)
      HP 회복 효과, 범위.

Crafting Station (제작대)
│   제작 가능한 레시피 필터(대장간/연금술/요리 등).
│
└─ Available Recipe (사용 가능 레시피) × N

Bulletin Board (게시판)
│
└─ Post (게시글) × N
      제목, 내용, 작성자, 작성일.
```

### 5. 시스템

```
Quest (퀘스트)
│   시작 조건, 반복 여부.
│
├─ Quest Stage (단계) × N
│   │   순차 진행.
│   │
│   └─ Quest Objective (목표) × N
│         타입(kill/collect/visit/talk),
│         대상 템플릿, 필요 수량.
│
└─ Quest Reward (보상)
      경험치, 골드, 아이템 × N.

Skill (스킬 정의)
│   이름, 설명, 타입(액티브/패시브).
│   쿨다운, 마나 소모, 레벨별 수치.
│
└─ Skill Effect (효과) × N
      데미지, 힐, 버프 부여, 상태이상 등.

Class (클래스 정의)
│   전사, 마법사, 도적 등.
│
├─ Base Stat (기본 스탯)
│     HP, 공격, 방어, 마나 초기값 및 성장치.
│
└─ Learnable Skill (습득 가능 스킬) × N
      스킬 참조, 습득 레벨.

Recipe (제작법)
│
├─ Ingredient (재료) × N
│     아이템 템플릿 참조, 수량.
│
└─ Result (결과물)
      아이템 템플릿 참조, 수량, 성공 확률.

Achievement (업적)
│
└─ Condition (달성 조건) × N
      타입(kill_count/quest_clear/level_reach...),
      대상, 수치.

Guild (길드)
│
├─ Guild Member (길드원) × N
│     캐릭터 참조, 직급(마스터/임원/일반).
│
└─ Guild Storage (길드 창고)
   │
   └─ Item × N

Party (파티)
│   임시 그룹. 최대 인원.
│
└─ Party Member (파티원) × N
      캐릭터 참조, 역할(리더/일반).
```

### 6. 버프/이펙트

```
Buff Definition (버프 정의)
│   이름, 아이콘, 최대 지속시간, 최대 중첩.
│
└─ Buff Effect (효과) × N
      타입(stat_modify/dot/hot/immunity...),
      수치, 틱 간격.
```

---

## 2D 엔티티

MUD와 **공유되는 엔티티**(캐릭터, 아이템, 오브젝트, 시스템)는 생략하고,
2D 전용이거나 구조가 다른 엔티티만 기술한다.

### 1. 공간 (MUD의 Zone/Room 대신)

```
Map (맵)
│   2D 타일 기반 공간. 너비×높이, 원점 좌표.
│
├─ Tile Layer (타일 레이어) × N
│   │   지형/장식/충돌 등 레이어 분리.
│   │   렌더 순서(z-order).
│   │
│   └─ Tile (타일) × (width × height)
│         좌표(x, y), 타일 타입 참조.
│
├─ Map Spawn (스폰 규칙) × N
│     템플릿 참조, 좌표(x, y), 수량, 리스폰 시간.
│
├─ Map Portal (포탈) × N
│     좌표(x, y), 대상 맵 + 대상 좌표.
│
└─ Map Trigger (트리거 영역) × N
      영역(x, y, w, h), 발동 이벤트(컷신/대화/전투 등).

Tile Type (타일 타입 정의)
      이름, 이동 가능 여부, 스프라이트 경로,
      속성(이동 속도 배율, 데미지 등).
```

### 2. 캐릭터 — 2D 추가 사항

```
Player (플레이어) — MUD 구조 + 아래 추가
│
├─ Position (위치)
│     맵 참조, x, y 좌표.
│
├─ Sprite (스프라이트)
│     스프라이트시트 참조, 현재 애니메이션 상태.
│
└─ (나머지는 MUD와 동일: Equipment, Inventory, Buff, Skill, Quest Log)

NPC / Monster — MUD 구조 + 아래 추가
│
├─ Position (위치)
│
├─ Sprite (스프라이트)
│
├─ Patrol Path (순찰 경로) — 몬스터/경비병
│   │
│   └─ Waypoint (경유지) × N
│         좌표(x, y), 대기 시간.
│
└─ Aggro Range (어그로 범위)
      감지 반경, 추적 반경, 복귀 반경.
```

### 3. 전투/이펙트 — 2D 전용

```
Projectile (투사체)
      발사자, 방향/속도, 데미지, 스프라이트,
      관통 여부, 최대 사거리.

AoE Zone (범위 효과 지역)
      중심 좌표, 반경, 지속 시간,
      효과(데미지/힐/감속 등), 틱 간격.
```

### 4. 환경 — 2D 전용

```
Light Source (광원)
      좌표, 반경, 색상, 강도.
      동적(횃불 깜빡임) / 정적.

Weather Zone (날씨 영역)
      맵 참조, 타입(비/눈/안개),
      강도, 시각/게임플레이 효과.

Particle Emitter (파티클 생성기)
      좌표, 파티클 타입(불꽃/먼지/연기),
      생성 속도, 범위.

Sound Emitter (사운드 생성기)
      좌표, 사운드 파일, 반경, 반복 여부.
```

---

## 엔티티 소속 관계 요약

### MUD

```
Zone
└─ Room
   ├─ Exit
   ├─ Room Spawn
   ├─ Portal
   ├─ Player
   │  ├─ Equipment → Item
   │  ├─ Inventory → Item
   │  │              └─ Container → Item
   │  ├─ Active Buff
   │  ├─ Skill
   │  ├─ Quest Log → Quest Stage → Objective
   │  └─ Friend List
   ├─ NPC
   │  ├─ Dialogue → Node → Choice
   │  ├─ Shop → Shop Entry → Item Template
   │  ├─ Train List → Trainable Skill
   │  └─ Quest Offer
   ├─ Monster
   │  ├─ Loot Table → Loot Entry → Item Template
   │  ├─ Skill
   │  └─ AI Behavior
   ├─ Pet
   ├─ Item (바닥)
   └─ Object (Door / Chest / Trap / Sign / ...)
      └─ Chest → Item
```

### 2D

```
Map
├─ Tile Layer
│  └─ Tile
├─ Map Spawn
├─ Map Portal
├─ Map Trigger
├─ Player
│  ├─ Position
│  ├─ Sprite
│  └─ (MUD와 동일한 하위 구조)
├─ NPC / Monster
│  ├─ Position
│  ├─ Sprite
│  ├─ Patrol Path → Waypoint
│  ├─ Aggro Range
│  └─ (MUD와 동일한 하위 구조)
├─ Pet
├─ Item (바닥)
├─ Object
├─ Projectile
├─ AoE Zone
├─ Light Source
├─ Particle Emitter
└─ Sound Emitter
```

---

## 공유 / 전용 분류

| 분류 | MUD 전용 | 2D 전용 | 공유 |
|------|----------|---------|------|
| 공간 | Zone, Room, Exit | Map, Tile Layer, Tile, Tile Type | Portal, Spawn Rule |
| 캐릭터 | — | Position, Sprite, Patrol Path, Aggro Range | Player, NPC, Monster, Pet |
| 아이템 | — | — | 전부 공유 |
| 오브젝트 | — | — | 전부 공유 |
| 전투 | — | Projectile, AoE Zone | Buff, Skill |
| 환경 | — | Light, Weather, Particle, Sound | — |
| 시스템 | — | — | Quest, Class, Recipe, Achievement, Guild, Party |
