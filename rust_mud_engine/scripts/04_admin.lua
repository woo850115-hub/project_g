-- 04_admin.lua: Admin commands (permission-gated via on_admin hooks)
-- Permission levels: 0=Player, 1=Builder, 2=Admin, 3=Owner

-- /kick <player_name> — Disconnect a player (Admin+)
hooks.on_admin("kick", 2, function(ctx)
    local target_name = ctx.args
    if target_name == "" then
        output:send(ctx.session_id, "사용법: /kick <플레이어이름>")
        return true
    end

    local playing = sessions:playing_list()
    for _, info in ipairs(playing) do
        local name = ecs:get(info.entity, "Name")
        if name and name:lower() == target_name:lower() then
            output:send(info.session_id, "관리자에 의해 접속이 종료되었습니다.")
            output:send(ctx.session_id, target_name .. " 님을 추방했습니다.")
            return true
        end
    end

    output:send(ctx.session_id, target_name .. " 님을 찾을 수 없습니다.")
    return true
end)

-- /announce <message> — Broadcast to all players (Admin+)
hooks.on_admin("announce", 2, function(ctx)
    local message = ctx.args
    if message == "" then
        output:send(ctx.session_id, "사용법: /announce <메시지>")
        return true
    end

    local playing = sessions:playing_list()
    for _, info in ipairs(playing) do
        output:send(info.session_id, "[공지] " .. message)
    end
    return true
end)

-- /teleport <player_name> <room_name> — Teleport a player to a room (Admin+)
hooks.on_admin("teleport", 2, function(ctx)
    local parts = {}
    for word in ctx.args:gmatch("%S+") do
        table.insert(parts, word)
    end

    if #parts < 2 then
        output:send(ctx.session_id, "사용법: /teleport <플레이어이름> <방이름>")
        return true
    end

    local target_name = parts[1]
    local room_name = table.concat(parts, " ", 2)

    -- Find target player entity
    local target_entity = nil
    local target_sid = nil
    local playing = sessions:playing_list()
    for _, info in ipairs(playing) do
        local name = ecs:get(info.entity, "Name")
        if name and name:lower() == target_name:lower() then
            target_entity = info.entity
            target_sid = info.session_id
            break
        end
    end

    if not target_entity then
        output:send(ctx.session_id, target_name .. " 님을 찾을 수 없습니다.")
        return true
    end

    -- Find target room
    local all_rooms = space:all_rooms()
    local target_room = nil
    for _, room_id in ipairs(all_rooms) do
        local rname = ecs:get(room_id, "Name")
        if rname and rname:lower() == room_name:lower() then
            target_room = room_id
            break
        end
    end

    if not target_room then
        output:send(ctx.session_id, "'" .. room_name .. "' 방을 찾을 수 없습니다.")
        return true
    end

    space:move_entity(target_entity, target_room)
    output:send(target_sid, "관리자에 의해 '" .. room_name .. "'(으)로 이동되었습니다.")
    output:send(ctx.session_id, target_name .. " 님을 '" .. room_name .. "'(으)로 이동시켰습니다.")
    return true
end)

-- /stats — Show server statistics (Builder+)
hooks.on_admin("stats", 1, function(ctx)
    local playing = sessions:playing_list()
    local player_count = #playing
    local room_count = space:room_count()

    local msg = "=== 서버 통계 ===\n"
    msg = msg .. "접속자 수: " .. player_count .. "\n"
    msg = msg .. "방 수: " .. room_count .. "\n"
    msg = msg .. "=== 접속자 목록 ===\n"
    for _, info in ipairs(playing) do
        local name = ecs:get(info.entity, "Name") or "???"
        local hp = ecs:get(info.entity, "Health")
        local hp_str = hp and (hp.current .. "/" .. hp.max) or "N/A"
        msg = msg .. "  " .. name .. " (HP: " .. hp_str .. ")\n"
    end

    output:send(ctx.session_id, msg)
    return true
end)

-- /help — Show admin help (Builder+)
hooks.on_admin("help", 1, function(ctx)
    local msg = "=== 관리자 명령어 ===\n"
    msg = msg .. "  /stats          — 서버 통계 (Builder+)\n"
    msg = msg .. "  /help           — 관리자 도움말 (Builder+)\n"
    msg = msg .. "  /kick <이름>    — 플레이어 추방 (Admin+)\n"
    msg = msg .. "  /announce <msg> — 전체 공지 (Admin+)\n"
    msg = msg .. "  /teleport <이름> <방> — 텔레포트 (Admin+)\n"
    output:send(ctx.session_id, msg)
    return true
end)
