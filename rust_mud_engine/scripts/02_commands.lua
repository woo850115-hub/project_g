-- 02_commands.lua: All player command handlers via on_action hooks

-- look
hooks.on_action("look", function(ctx)
    local room = space:entity_room(ctx.entity)
    if not room then
        output:send(ctx.session_id, "현재 위치를 알 수 없습니다.")
        return true
    end
    output:send(ctx.session_id, format_room(room, ctx.entity))
    return true
end)

-- move
hooks.on_action("move", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local dir = ctx.args  -- "north", "south", "east", "west"

    -- Dead check
    if ecs:has(entity, "Dead") then
        output:send(session_id, "죽은 상태로는 이동할 수 없습니다.")
        return true
    end

    local current_room = space:entity_room(entity)
    if not current_room then
        output:send(session_id, "현재 위치를 알 수 없습니다.")
        return true
    end

    -- Find target room from exits
    local exits = space:exits(current_room)
    if not exits or not exits[dir] then
        local dir_ko = DIRECTION_KO[dir] or dir
        output:send(session_id, dir_ko .. "쪽으로는 출구가 없습니다.")
        return true
    end

    local target_room = exits[dir]

    -- Move entity
    local ok, err = pcall(function()
        space:move_entity(entity, target_room)
    end)
    if not ok then
        output:send(session_id, "이동 불가: " .. tostring(err))
        return true
    end

    local player_name = get_name(entity)
    local dir_ko = DIRECTION_KO[dir] or dir
    local opposite = DIRECTION_OPPOSITE[dir]
    local opposite_ko = DIRECTION_KO[opposite] or opposite

    -- Notify old room occupants
    broadcast_room(current_room, player_name .. "님이 " .. dir_ko .. "쪽으로 떠났습니다.", entity)

    -- Notify new room occupants
    broadcast_room(target_room, player_name .. "님이 " .. opposite_ko .. "쪽에서 도착했습니다.", entity)

    -- Show new room to mover
    output:send(session_id, format_room(target_room, entity))

    -- Fire on_enter_room hooks
    hooks.fire_enter_room(entity, target_room, current_room)

    return true
end)

-- attack
hooks.on_action("attack", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local target_name = ctx.args

    if ecs:has(entity, "Dead") then
        output:send(session_id, "죽은 상태로는 싸울 수 없습니다.")
        return true
    end

    local room = space:entity_room(entity)
    if not room then
        output:send(session_id, "현재 위치를 알 수 없습니다.")
        return true
    end

    -- Find target by name in room
    local occupants = space:room_occupants(room)
    local target = nil
    local target_name_lower = string.lower(target_name)
    for _, occ in ipairs(occupants) do
        if occ ~= entity and not ecs:has(occ, "Dead") then
            local name = ecs:get(occ, "Name")
            if name and string.find(string.lower(name), target_name_lower, 1, true) then
                target = occ
                break
            end
        end
    end

    if not target then
        output:send(session_id, "여기에 '" .. target_name .. "'이(가) 보이지 않습니다.")
        return true
    end

    local tname = get_name(target)
    ecs:set(entity, "CombatTarget", target)
    output:send(session_id, tname .. "을(를) 공격합니다!")

    return true
end)

-- get (pick up item)
hooks.on_action("get", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local item_name = ctx.args

    if ecs:has(entity, "Dead") then
        output:send(session_id, "죽은 상태로는 아이템을 주울 수 없습니다.")
        return true
    end

    local room = space:entity_room(entity)
    if not room then
        output:send(session_id, "현재 위치를 알 수 없습니다.")
        return true
    end

    -- Find item in room
    local occupants = space:room_occupants(room)
    local target_item = nil
    local item_name_lower = string.lower(item_name)
    for _, occ in ipairs(occupants) do
        if ecs:has(occ, "ItemTag") then
            local name = ecs:get(occ, "Name")
            if name and string.find(string.lower(name), item_name_lower, 1, true) then
                target_item = occ
                break
            end
        end
    end

    if not target_item then
        output:send(session_id, "여기에 '" .. item_name .. "'이(가) 보이지 않습니다.")
        return true
    end

    -- Remove item from room
    space:remove_entity(target_item)

    -- Add to player inventory
    local inv = ecs:get(entity, "Inventory")
    if not inv then
        inv = {items = {}}
    end
    table.insert(inv.items, target_item)
    ecs:set(entity, "Inventory", inv)

    local iname = get_name(target_item)
    output:send(session_id, iname .. "을(를) 주웠습니다.")

    return true
end)

-- drop
hooks.on_action("drop", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local item_name = ctx.args

    if ecs:has(entity, "Dead") then
        output:send(session_id, "죽은 상태로는 아이템을 버릴 수 없습니다.")
        return true
    end

    local room = space:entity_room(entity)
    if not room then
        output:send(session_id, "현재 위치를 알 수 없습니다.")
        return true
    end

    local inv = ecs:get(entity, "Inventory")
    if not inv or not inv.items or #inv.items == 0 then
        output:send(session_id, "아무것도 가지고 있지 않습니다.")
        return true
    end

    -- Find item in inventory
    local found_idx = nil
    local found_item = nil
    local item_name_lower = string.lower(item_name)
    for i, item_id in ipairs(inv.items) do
        local name = ecs:get(item_id, "Name")
        if name and string.find(string.lower(name), item_name_lower, 1, true) then
            found_idx = i
            found_item = item_id
            break
        end
    end

    if not found_item then
        output:send(session_id, "'" .. item_name .. "'을(를) 가지고 있지 않습니다.")
        return true
    end

    -- Remove from inventory
    table.remove(inv.items, found_idx)
    ecs:set(entity, "Inventory", inv)

    -- Place item in room
    space:place_entity(found_item, room)

    local iname = get_name(found_item)
    output:send(session_id, iname .. "을(를) 버렸습니다.")

    return true
end)

-- inventory
hooks.on_action("inventory", function(ctx)
    output:send(ctx.session_id, format_inventory(ctx.entity))
    return true
end)

-- say
hooks.on_action("say", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local msg = ctx.args

    local name = get_name(entity)

    output:send(session_id, "당신이 말합니다: " .. msg)

    local room = space:entity_room(entity)
    if room then
        local occupants = space:room_occupants(room)
        for _, occ in ipairs(occupants) do
            if occ ~= entity then
                local sid = sessions:session_for(occ)
                if sid then
                    output:send(sid, name .. "님이 말합니다: " .. msg)
                end
            end
        end
    end

    return true
end)

-- who
hooks.on_action("who", function(ctx)
    local playing = sessions:playing_list()
    local lines = {"접속 중인 플레이어 (" .. tostring(#playing) .. ")명:"}
    for _, entry in ipairs(playing) do
        if entry.name then
            table.insert(lines, "  - " .. entry.name)
        end
    end
    output:send(ctx.session_id, table.concat(lines, "\n"))
    return true
end)

-- help
hooks.on_action("help", function(ctx)
    output:send(ctx.session_id, HELP_TEXT)
    return true
end)

-- unknown command
hooks.on_action("unknown", function(ctx)
    if ctx.args and ctx.args ~= "" then
        output:send(ctx.session_id, "알 수 없는 명령어: " .. ctx.args)
    else
        output:send(ctx.session_id, "알 수 없는 명령어입니다.")
    end
    return true
end)
