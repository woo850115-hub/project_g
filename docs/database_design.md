# Project G — 데이터베이스 설계

> 작성일: 2026-02-20
> 상태: 초안 (핵심 스키마 확정, 세부 사항 추가 예정)

## 개요

게임메이커 및 플레이어 데이터 영속성을 위한 SQLite 기반 DB 설계.
하나의 엔진이 MUD/2D 양쪽 모드를 지원하되, 콘텐츠는 게임별로 분리한다.

## 설계 원칙

- **엔진은 공유, 콘텐츠는 분리**: 게임마다 독립된 `game.db` 파일
- **런타임은 인메모리**: DB는 서버 시작 시 로드 + 로그인/로그아웃 시 저장. 틱 루프에서 DB를 치지 않음
- **ECS 미러링**: `template_components` 테이블이 ECS 컴포넌트 구조를 그대로 반영. 새 컴포넌트 추가 시 스키마 변경 불필요
- **SQLite 선택 이유**: 임베디드 (별도 서버 불필요), 파일 하나로 백업/배포, 소규모~중규모 충분

## 디렉토리 구조

```
project_g/
├── games/
│   ├── my_mud/
│   │   ├── game.db           ← MUD 게임 콘텐츠 + 플레이어 데이터
│   │   └── scripts/          ← Lua 게임 스크립트
│   └── my_2d_rpg/
│       ├── game.db           ← 2D 게임 콘텐츠 + 플레이어 데이터
│       └── scripts_grid/
└── rust_mud_engine/           ← 엔진 바이너리
```

## 데이터 흐름

```
서버 시작
  ├─ game.db에서 rooms/maps 로드 → Space 구성
  ├─ game.db에서 spawn_rules 로드 → 템플릿으로 엔티티 생성
  └─ Lua 스크립트 로드

런타임 (ECS 인메모리)
  ├─ 플레이어 로그인 → characters.save_data → ECS 엔티티 복원
  ├─ 플레이어 로그아웃 → ECS 컴포넌트 → JSON → characters.save_data 저장
  └─ 리스폰 타이머 → spawn_rules 참조해서 재생성

서버 종료
  └─ 접속 중인 플레이어 전부 save_data 저장
```

## 스키마

### 게임 메타데이터

```sql
CREATE TABLE game_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

필수 키:

| key | 설명 | 값 예시 |
|-----|------|---------|
| `mode` | 게임 모드 | `"mud"` / `"grid"` |
| `name` | 게임 이름 | `"나의 MUD"` |
| `version` | 콘텐츠 버전 | `"1"` |
| `auth_mode` | 인증 방식 | `"character"` / `"account"` |

- `auth_mode = "character"`: 캐릭터 = 계정 (클래식 MUD 방식)
- `auth_mode = "account"`: 계정 + 캐릭터 분리 (MMO 방식)

### 엔티티 템플릿 (MUD/2D 공용)

```sql
-- 템플릿 = "이런 엔티티가 존재할 수 있다"
CREATE TABLE templates (
    id          TEXT PRIMARY KEY,       -- "goblin", "health_potion"
    kind        TEXT NOT NULL,          -- "npc", "item", "object"
    name        TEXT NOT NULL,          -- "고블린"
    description TEXT DEFAULT ''
);

-- 템플릿의 컴포넌트 데이터 (ECS 미러)
CREATE TABLE template_components (
    template_id    TEXT NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
    component_type TEXT NOT NULL,       -- "Health", "Attack", "Defense"
    data           TEXT NOT NULL,       -- JSON: {"current":50,"max":50}
    PRIMARY KEY (template_id, component_type)
);
```

예시 데이터:

```sql
INSERT INTO templates VALUES ('goblin', 'npc', '고블린', '작고 사악한 생물');
INSERT INTO template_components VALUES ('goblin', 'Health',  '{"current":50,"max":50}');
INSERT INTO template_components VALUES ('goblin', 'Attack',  '8');
INSERT INTO template_components VALUES ('goblin', 'Defense', '2');
INSERT INTO template_components VALUES ('goblin', 'NpcTag',  'true');

INSERT INTO templates VALUES ('health_potion', 'item', '체력 물약', 'HP를 30 회복한다');
INSERT INTO template_components VALUES ('health_potion', 'ItemTag', 'true');
INSERT INTO template_components VALUES ('health_potion', 'HealEffect', '{"amount":30}');
```

### MUD 전용 공간 (mode="mud")

```sql
CREATE TABLE zones (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT DEFAULT ''
);

CREATE TABLE rooms (
    id          TEXT PRIMARY KEY,
    zone_id     TEXT REFERENCES zones(id),
    name        TEXT NOT NULL,
    description TEXT NOT NULL,
    properties  TEXT DEFAULT '{}'       -- JSON: {"dark":true, "safe_zone":true}
);

CREATE TABLE room_exits (
    room_id   TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    direction TEXT NOT NULL,            -- "north", "south", "up", "portal_1"
    target_id TEXT NOT NULL REFERENCES rooms(id),
    PRIMARY KEY (room_id, direction)
);

CREATE TABLE room_spawns (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id     TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    template_id TEXT NOT NULL REFERENCES templates(id),
    max_count   INTEGER DEFAULT 1,
    respawn_sec INTEGER DEFAULT 0       -- 0 = 리스폰 안 함
);
```

### 2D 전용 공간 (mode="grid")

```sql
CREATE TABLE maps (
    id       TEXT PRIMARY KEY,
    name     TEXT NOT NULL,
    width    INTEGER NOT NULL,
    height   INTEGER NOT NULL,
    origin_x INTEGER DEFAULT 0,
    origin_y INTEGER DEFAULT 0,
    properties TEXT DEFAULT '{}'        -- JSON: {"pvp":true}
);

CREATE TABLE tile_types (
    id         TEXT PRIMARY KEY,        -- "grass", "wall", "water"
    walkable   INTEGER NOT NULL DEFAULT 1,
    sprite     TEXT DEFAULT '',          -- "grass_01.png"
    properties TEXT DEFAULT '{}'        -- JSON: {"speed_mult":0.5}
);

CREATE TABLE map_tiles (
    map_id    TEXT NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    x         INTEGER NOT NULL,
    y         INTEGER NOT NULL,
    tile_type TEXT NOT NULL REFERENCES tile_types(id),
    PRIMARY KEY (map_id, x, y)
);

CREATE TABLE map_spawns (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    map_id      TEXT NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    template_id TEXT NOT NULL REFERENCES templates(id),
    x           INTEGER NOT NULL,
    y           INTEGER NOT NULL,
    max_count   INTEGER DEFAULT 1,
    respawn_sec INTEGER DEFAULT 0
);
```

### 계정 + 캐릭터

```sql
-- auth_mode = "account" 일 때만 사용
CREATE TABLE accounts (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at    TEXT DEFAULT (datetime('now')),
    banned        INTEGER DEFAULT 0
);

-- 항상 사용
CREATE TABLE characters (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER,              -- MUD(character모드): NULL, 2D: FK → accounts
    name          TEXT UNIQUE NOT NULL,
    password_hash TEXT,                  -- MUD(character모드): 여기에 저장, 2D: NULL
    save_data     TEXT NOT NULL DEFAULT '{}',
    last_login    TEXT,
    play_time     INTEGER DEFAULT 0,

    FOREIGN KEY (account_id) REFERENCES accounts(id)
);
```

### 인증 모드별 동작

**MUD (auth_mode = "character")**: 캐릭터 = 계정

| 컬럼 | 값 |
|------|-----|
| account_id | NULL (사용 안 함) |
| name | "고블린슬레이어" (로그인 키) |
| password_hash | bcrypt 해시 |
| save_data | `{"Health":{"current":85,"max":100},...}` |

로그인 흐름:
```
접속 → "이름을 입력하세요:" → "고블린슬레이어"
→ DB 조회 (SELECT * FROM characters WHERE name = ?)
  → 없으면: "새 캐릭터입니다. 비밀번호를 설정하세요:"
  → 있으면: "비밀번호를 입력하세요:"
→ 인증 성공 → save_data에서 컴포넌트 복원 → 게임 진입
```

**2D (auth_mode = "account")**: 계정 + 캐릭터 분리

| 컬럼 | 값 |
|------|-----|
| account_id | 1 (accounts FK) |
| name | "전사캐릭" |
| password_hash | NULL (accounts 테이블에 저장) |
| save_data | `{"Health":...,"last_map":"field_01","last_x":120,"last_y":85}` |

로그인 흐름:
```
접속 → 아이디/비밀번호 입력
→ accounts 인증
→ 캐릭터 선택 화면 (SELECT * FROM characters WHERE account_id = ?)
→ 캐릭터 선택 → save_data에서 복원 → 게임 진입
```

## 공유 범위 정리

| 항목 | MUD / 2D 공유 | 비고 |
|------|:---:|------|
| 엔진 (Rust) | O | 동일 바이너리 |
| DB 스키마 구조 | O | 동일 테이블 정의 |
| templates + template_components | O | 패턴 공유, 내용은 게임별 |
| characters 테이블 | O | auth_mode로 동작 분기 |
| rooms / room_exits / room_spawns | MUD 전용 | |
| maps / tile_types / map_tiles / map_spawns | 2D 전용 | |
| accounts 테이블 | 2D 전용 | MUD는 character 모드 시 불필요 |
| Lua 스크립트 | 구조 공유 | 내용은 게임별 |

## TODO

- [ ] Rust 측 SQLite 연동 (rusqlite crate 선정)
- [ ] 데이터 로더 구현 (DB → ECS 엔티티 생성)
- [ ] save_data 직렬화/역직렬화 포맷 확정
- [ ] 비밀번호 해싱 라이브러리 선정 (argon2 / bcrypt)
- [ ] 마이그레이션 전략 (스키마 버전 관리)
- [ ] 게임메이커 웹 UI 설계
- [ ] 퀘스트/대화 시스템 스키마 추가
- [ ] 스폰 규칙 상세화 (조건부 스폰, 시간대별 등)
