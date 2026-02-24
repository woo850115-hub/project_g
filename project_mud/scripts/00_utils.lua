-- 00_utils.lua: Common helper functions for MUD game scripts

-- ANSI color code table for text formatting
colors = {
    reset     = "\27[0m",
    bold      = "\27[1m",
    dim       = "\27[2m",
    underline = "\27[4m",
    -- Standard foreground
    black   = "\27[30m",
    red     = "\27[31m",
    green   = "\27[32m",
    yellow  = "\27[33m",
    blue    = "\27[34m",
    magenta = "\27[35m",
    cyan    = "\27[36m",
    white   = "\27[37m",
    -- Bright foreground
    bright_red     = "\27[91m",
    bright_green   = "\27[92m",
    bright_yellow  = "\27[93m",
    bright_blue    = "\27[94m",
    bright_magenta = "\27[95m",
    bright_cyan    = "\27[96m",
    bright_white   = "\27[97m",
}

-- Direction mapping tables
DIRECTION_KO = {
    north = "북",
    south = "남",
    east = "동",
    west = "서",
}

DIRECTION_OPPOSITE = {
    north = "south",
    south = "north",
    east = "west",
    west = "east",
}

-- Deterministic direction order
DIRECTION_ORDER = {"north", "south", "east", "west"}

--- Get the Name component of an entity, or a default string.
function get_name(eid)
    local name = ecs:get(eid, "Name")
    if name then
        return name
    end
    return "누군가"
end

--- Broadcast text to all players in a room, optionally excluding one entity.
function broadcast_room(room_id, text, exclude_eid)
    local occupants = space:room_occupants(room_id)
    for _, occ in ipairs(occupants) do
        if occ ~= exclude_eid then
            local sid = sessions:session_for(occ)
            if sid then
                output:send(sid, text)
            end
        end
    end
end

--- Format exits for a room in deterministic order (북, 남, 동, 서).
function format_exits(room_id)
    local exits_table = space:exits(room_id)
    if not exits_table then
        return "없음"
    end

    local dirs = {}
    for _, dir in ipairs(DIRECTION_ORDER) do
        if exits_table[dir] then
            table.insert(dirs, DIRECTION_KO[dir])
        end
    end

    -- Collect and sort custom exits
    local custom = {}
    for key, _ in pairs(exits_table) do
        if not DIRECTION_KO[key] then
            table.insert(custom, key)
        end
    end
    table.sort(custom)
    for _, key in ipairs(custom) do
        table.insert(dirs, key)
    end

    if #dirs == 0 then
        return "없음"
    end
    return table.concat(dirs, ", ")
end

--- Format a full room description for a viewer entity.
function format_room(room_id, viewer)
    local lines = {}

    -- Room name (bold cyan)
    local room_name = ecs:get(room_id, "Name") or "알 수 없는 방"
    table.insert(lines, colors.bold .. colors.cyan .. "== " .. room_name .. " ==" .. colors.reset)

    -- Room description
    local desc = ecs:get(room_id, "Description")
    if desc then
        table.insert(lines, desc)
    end

    -- Exits (green)
    table.insert(lines, colors.green .. "출구: " .. format_exits(room_id) .. colors.reset)

    -- Other entities in the room
    local occupants = space:room_occupants(room_id)
    local others = {}
    for _, occ in ipairs(occupants) do
        if occ ~= viewer and occ ~= room_id then
            local name = ecs:get(occ, "Name") or "무언가"
            if ecs:has(occ, "Dead") then
                table.insert(others, name .. " (죽음)")
            elseif ecs:has(occ, "NpcTag") then
                table.insert(others, name)
            elseif ecs:has(occ, "PlayerTag") then
                table.insert(others, name)
            elseif ecs:has(occ, "ItemTag") then
                table.insert(others, "[" .. name .. "]")
            end
        end
    end

    if #others > 0 then
        table.insert(lines, "주위에: " .. table.concat(others, ", "))
    end

    return table.concat(lines, "\n")
end

--- Format inventory listing for an entity.
function format_inventory(eid)
    local inv = ecs:get(eid, "Inventory")
    if not inv or not inv.items or #inv.items == 0 then
        return "아무것도 가지고 있지 않습니다."
    end
    local lines = {"소지품:"}
    for _, item_id in ipairs(inv.items) do
        local name = ecs:get(item_id, "Name") or "알 수 없는 아이템"
        table.insert(lines, "  - " .. name)
    end
    return table.concat(lines, "\n")
end

--- Get a monster definition from content registry by id.
function get_monster_def(id)
    if content and content.monsters then
        for _, mon in ipairs(content.monsters) do
            if mon.id == id then return mon end
        end
    end
    return nil
end

--- Get an item definition from content registry by id.
function get_item_def(id)
    if content and content.items then
        for _, item in ipairs(content.items) do
            if item.id == id then return item end
        end
    end
    return nil
end

--- Calculate gold drop from a dead NPC by checking its content loot_table.
--- Looks up the NPC's Name in content.monsters, then sums currency item values.
function calc_gold_drop(dead_entity)
    if not content or not content.monsters or not content.items then
        return 0
    end

    -- Find monster definition by matching Name
    local name = ecs:get(dead_entity, "Name")
    if not name then return 0 end

    local monster_def = nil
    for _, mon in ipairs(content.monsters) do
        if mon.name == name then
            monster_def = mon
            break
        end
    end

    if not monster_def or not monster_def.loot_table then
        return 0
    end

    -- Sum gold from currency items in loot_table
    local gold = 0
    for _, loot_id in ipairs(monster_def.loot_table) do
        local item_def = get_item_def(loot_id)
        if item_def and item_def.item_type == "currency" then
            gold = gold + (item_def.value or 1)
        end
    end

    return gold
end

--- Get a race definition from content registry by id.
function get_race_def(id)
    if content and content.races then
        for _, race in ipairs(content.races) do
            if race.id == id then return race end
        end
    end
    return nil
end

--- Get a class definition from content registry by id.
function get_class_def(id)
    if content and content.classes then
        for _, cls in ipairs(content.classes) do
            if cls.id == id then return cls end
        end
    end
    return nil
end

--- Get a skill definition from content registry by id.
function get_skill_def(id)
    if content and content.skills then
        for _, skill in ipairs(content.skills) do
            if skill.id == id then return skill end
        end
    end
    return nil
end

--- Format a character status display.
function format_status(eid)
    local name = get_name(eid)
    local race = ecs:get(eid, "Race") or "없음"
    local class = ecs:get(eid, "Class") or "없음"
    local hp = ecs:get(eid, "Health")
    local atk = ecs:get(eid, "Attack") or 0
    local def = ecs:get(eid, "Defense") or 0
    local skills_data = ecs:get(eid, "Skills")

    local lines = {}
    table.insert(lines, colors.bold .. colors.cyan .. "=== " .. name .. "의 상태 ===" .. colors.reset)
    table.insert(lines, "종족: " .. colors.yellow .. race .. colors.reset .. "  직업: " .. colors.yellow .. class .. colors.reset)

    local level = ecs:get(eid, "Level") or 1
    local exp = ecs:get(eid, "Experience") or 0
    local entry = level_table and level_table[level]
    local exp_next = entry and entry.exp_required or "?"
    table.insert(lines, "레벨: " .. colors.bright_white .. tostring(level) .. colors.reset .. "  경험치: " .. tostring(exp) .. "/" .. tostring(exp_next))

    if hp then
        table.insert(lines, "체력: " .. colors.green .. tostring(hp.current) .. "/" .. tostring(hp.max) .. colors.reset)
    end

    local mp = ecs:get(eid, "Mana")
    if mp then
        table.insert(lines, "마나: " .. colors.blue .. tostring(mp.current) .. "/" .. tostring(mp.max) .. colors.reset)
    end

    local gold = ecs:get(eid, "Gold") or 0
    table.insert(lines, "골드: " .. colors.yellow .. tostring(gold) .. colors.reset)
    table.insert(lines, "공격력: " .. colors.red .. tostring(atk) .. colors.reset .. "  방어력: " .. colors.blue .. tostring(def) .. colors.reset)

    if skills_data and skills_data.learned and #skills_data.learned > 0 then
        table.insert(lines, "스킬: " .. colors.magenta .. table.concat(skills_data.learned, ", ") .. colors.reset)
    else
        table.insert(lines, "스킬: 없음")
    end

    return table.concat(lines, "\n")
end

HELP_TEXT = [[사용 가능한 명령어:
  보기 (ㅂ)           - 주변을 둘러봅니다
  북                  - 북쪽으로 이동
  남                  - 남쪽으로 이동
  동                  - 동쪽으로 이동
  서                  - 서쪽으로 이동
  <대상> 공격 (ㄱ)    - 대상을 공격합니다
  <아이템> 줍기 (ㅈ)  - 아이템을 줍습니다
  <아이템> 버리기 (ㅂㄹ) - 아이템을 버립니다
  가방 (인벤)         - 소지품을 확인합니다
  골드 (ㄱㄷ)         - 보유 골드를 확인합니다
  상태                - 캐릭터 상태를 확인합니다
  스킬                - 보유 스킬 목록을 확인합니다
  <스킬이름> 스킬     - 스킬을 사용합니다
  <내용> 말 (ㅁ)      - 말을 합니다
  접속자              - 접속 중인 플레이어 목록
  도움말 (ㄷ, ?)      - 이 도움말을 표시합니다
  종료                - 접속을 종료합니다]]
