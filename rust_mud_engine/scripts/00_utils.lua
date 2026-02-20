-- 00_utils.lua: Common helper functions for MUD game scripts

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

    -- Room name
    local room_name = ecs:get(room_id, "Name") or "알 수 없는 방"
    table.insert(lines, "== " .. room_name .. " ==")

    -- Room description
    local desc = ecs:get(room_id, "Description")
    if desc then
        table.insert(lines, desc)
    end

    -- Exits
    table.insert(lines, "출구: " .. format_exits(room_id))

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

HELP_TEXT = [[사용 가능한 명령어:
  보기 (ㅂ)           - 주변을 둘러봅니다
  북                  - 북쪽으로 이동
  남                  - 남쪽으로 이동
  동                  - 동쪽으로 이동
  서                  - 서쪽으로 이동
  공격 <대상>         - 대상을 공격합니다
  줍기 <아이템>       - 아이템을 줍습니다
  버리기 <아이템>     - 아이템을 버립니다
  가방 (인벤)         - 소지품을 확인합니다
  말 <내용>           - 말을 합니다
  접속자              - 접속 중인 플레이어 목록
  도움말 (?)          - 이 도움말을 표시합니다
  종료                - 접속을 종료합니다]]
