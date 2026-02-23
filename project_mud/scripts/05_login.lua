-- 05_login.lua: Login flow (auth mode + quick-play mode)
-- Manages the entire login state machine via on_input/on_connect/on_disconnect hooks.
-- Auth mode: name -> password -> character selection -> race/class -> playing
-- Quick-play mode: name -> race/class -> playing (no DB)

-- Per-session login sub-state tracking
local login_state = {}

-- Current tick (tracked via on_tick for lingering disconnect_tick)
local current_tick = 0

-- Race/Class lists for selection (loaded from content)
local RACE_LIST = {"인간", "엘프", "드워프", "오크"}
local CLASS_LIST = {"전사", "마법사", "도적", "성직자"}

-- Default components for new characters
local CHARACTER_DEFAULTS = {
    Health = {current = 100, max = 100},
    Attack = 10,
    Defense = 5,
}

-- Find the first room in the world (used as spawn point)
local function find_starting_room()
    local rooms = space:all_rooms()
    if rooms and #rooms > 0 then
        return rooms[1]
    end
    return nil
end

-- Place entity in a room, falling back to starting room if the saved room doesn't exist
local function place_in_room(entity, room_id)
    if room_id and space:room_exists(room_id) then
        space:place_entity(entity, room_id)
    else
        local room = find_starting_room()
        if room then
            space:place_entity(entity, room)
        end
    end
end

-- Show race selection UI
local function show_race_selection(session_id)
    local lines = {colors.bold .. colors.cyan .. "=== 종족 선택 ===" .. colors.reset}
    for i, race_id in ipairs(RACE_LIST) do
        local def = get_race_def(race_id)
        local desc = def and def.description or ""
        local bonuses = ""
        if def then
            local parts = {}
            if def.hp_bonus ~= 0 then table.insert(parts, "HP" .. (def.hp_bonus > 0 and "+" or "") .. tostring(def.hp_bonus)) end
            if def.attack_bonus ~= 0 then table.insert(parts, "ATK" .. (def.attack_bonus > 0 and "+" or "") .. tostring(def.attack_bonus)) end
            if def.defense_bonus ~= 0 then table.insert(parts, "DEF" .. (def.defense_bonus > 0 and "+" or "") .. tostring(def.defense_bonus)) end
            if def.racial_skill then table.insert(parts, "고유: " .. def.racial_skill) end
            if #parts > 0 then bonuses = " (" .. table.concat(parts, ", ") .. ")" end
        end
        table.insert(lines, string.format("  %d. %s%s", i, race_id, bonuses))
    end
    table.insert(lines, "")
    table.insert(lines, "번호를 입력하세요:")
    output:send(session_id, table.concat(lines, "\n"))
end

-- Show class selection UI
local function show_class_selection(session_id)
    local lines = {colors.bold .. colors.cyan .. "=== 직업 선택 ===" .. colors.reset}
    for i, class_id in ipairs(CLASS_LIST) do
        local def = get_class_def(class_id)
        local bonuses = ""
        if def then
            local parts = {}
            if def.hp_bonus ~= 0 then table.insert(parts, "HP" .. (def.hp_bonus > 0 and "+" or "") .. tostring(def.hp_bonus)) end
            if def.attack_bonus ~= 0 then table.insert(parts, "ATK" .. (def.attack_bonus > 0 and "+" or "") .. tostring(def.attack_bonus)) end
            if def.defense_bonus ~= 0 then table.insert(parts, "DEF" .. (def.defense_bonus > 0 and "+" or "") .. tostring(def.defense_bonus)) end
            if def.starting_skills and #def.starting_skills > 0 then
                table.insert(parts, "스킬: " .. table.concat(def.starting_skills, ", "))
            end
            if #parts > 0 then bonuses = " (" .. table.concat(parts, ", ") .. ")" end
        end
        table.insert(lines, string.format("  %d. %s%s", i, class_id, bonuses))
    end
    table.insert(lines, "")
    table.insert(lines, "번호를 입력하세요:")
    output:send(session_id, table.concat(lines, "\n"))
end

-- Apply race and class bonuses to an entity (base stats + bonuses + skills)
local function apply_race_class(entity, race_id, class_id)
    -- Base stats
    local base_hp = 100
    local base_atk = 10
    local base_def = 5

    local race_def = get_race_def(race_id)
    local class_def = get_class_def(class_id)

    if race_def then
        base_hp = base_hp + (race_def.hp_bonus or 0)
        base_atk = base_atk + (race_def.attack_bonus or 0)
        base_def = base_def + (race_def.defense_bonus or 0)
    end

    if class_def then
        base_hp = base_hp + (class_def.hp_bonus or 0)
        base_atk = base_atk + (class_def.attack_bonus or 0)
        base_def = base_def + (class_def.defense_bonus or 0)
    end

    ecs:set(entity, "Health", {current = base_hp, max = base_hp})
    ecs:set(entity, "Attack", base_atk)
    ecs:set(entity, "Defense", base_def)
    ecs:set(entity, "Race", race_id)
    ecs:set(entity, "Class", class_id)
    ecs:set(entity, "Level", {level = 1, exp = 0, exp_next = 100})

    -- Collect skills (class starting skills + racial skill)
    local skills = {}
    if class_def and class_def.starting_skills then
        for _, s in ipairs(class_def.starting_skills) do
            table.insert(skills, s)
        end
    end
    if race_def and race_def.racial_skill then
        -- Avoid duplicate
        local already = false
        for _, s in ipairs(skills) do
            if s == race_def.racial_skill then already = true; break end
        end
        if not already then
            table.insert(skills, race_def.racial_skill)
        end
    end

    ecs:set(entity, "Skills", {learned = skills})
end

-- Spawn a new entity for quick-play mode (no auth/DB)
local function spawn_quick_play(session_id, name, race_id, class_id)
    local entity = ecs:spawn()
    ecs:set(entity, "Name", name)
    ecs:set(entity, "PlayerTag", true)
    ecs:set(entity, "Inventory", {items = {}})

    apply_race_class(entity, race_id, class_id)

    place_in_room(entity, nil)

    sessions:start_playing(session_id, entity)
    sessions:set_name(session_id, name)

    log.info("Quick-play: '" .. name .. "' joined (" .. race_id .. "/" .. class_id .. ")")
    return entity
end

-- Spawn (or rebind) a character from DB data
local function spawn_character(session_id, char_detail, account)
    -- Check for lingering entity first (seamless reconnection)
    local linger = sessions:find_lingering(char_detail.id)
    if linger then
        local entity = sessions:rebind_lingering(session_id, char_detail.id)
        if entity then
            sessions:set_name(session_id, char_detail.name)
            sessions:set_permission(session_id, account.permission)
            output:send(session_id, colors.green .. "이전 세션에 재접속했습니다." .. colors.reset)
            log.info("Player '" .. char_detail.name .. "' reconnected (rebind lingering)")
            return entity
        end
    end

    -- Spawn new entity
    local entity = ecs:spawn()
    ecs:set(entity, "Name", char_detail.name)
    ecs:set(entity, "PlayerTag", true)

    -- Restore components from DB (or apply defaults)
    local comps = char_detail.components
    if comps and type(comps) == "table" then
        if comps.Health then
            ecs:set(entity, "Health", comps.Health)
        else
            ecs:set(entity, "Health", {current = 100, max = 100})
        end
        if comps.Attack then
            ecs:set(entity, "Attack", comps.Attack)
        else
            ecs:set(entity, "Attack", 10)
        end
        if comps.Defense then
            ecs:set(entity, "Defense", comps.Defense)
        else
            ecs:set(entity, "Defense", 5)
        end
        -- Restore new RPG components
        if comps.Race then
            ecs:set(entity, "Race", comps.Race)
        end
        if comps.Class then
            ecs:set(entity, "Class", comps.Class)
        end
        if comps.Level then
            ecs:set(entity, "Level", comps.Level)
        else
            ecs:set(entity, "Level", {level = 1, exp = 0, exp_next = 100})
        end
        if comps.Skills then
            ecs:set(entity, "Skills", comps.Skills)
        else
            ecs:set(entity, "Skills", {learned = {}})
        end
    else
        ecs:set(entity, "Health", {current = 100, max = 100})
        ecs:set(entity, "Attack", 10)
        ecs:set(entity, "Defense", 5)
        ecs:set(entity, "Level", {level = 1, exp = 0, exp_next = 100})
        ecs:set(entity, "Skills", {learned = {}})
    end

    ecs:set(entity, "Inventory", {items = {}})

    place_in_room(entity, char_detail.room_id)

    -- Bind to session
    sessions:start_playing(session_id, entity)
    sessions:set_name(session_id, char_detail.name)
    sessions:set_account_id(session_id, account.id)
    sessions:set_character_id(session_id, char_detail.id)
    sessions:set_permission(session_id, account.permission)

    log.info("Player '" .. char_detail.name .. "' entered the game")
    return entity
end

-- Show character selection menu
local function enter_character_selection(session_id, state)
    local ok, chars = pcall(function()
        return auth:list_characters(state.account.id)
    end)
    if not ok then
        output:send(session_id, colors.red .. "캐릭터 목록 조회 실패: " .. tostring(chars) .. colors.reset)
        return
    end

    state.characters = chars
    state.step = "character_select"

    local lines = {colors.bold .. "=== 캐릭터 선택 ===" .. colors.reset}

    if #chars > 0 then
        for i, c in ipairs(chars) do
            table.insert(lines, string.format("  %d. %s", i, c.name))
        end
        table.insert(lines, "")
        table.insert(lines, "번호를 입력하거나, 새 캐릭터 이름을 입력하세요:")
    else
        table.insert(lines, "캐릭터가 없습니다. 새 캐릭터 이름을 입력하세요:")
    end

    output:send(session_id, table.concat(lines, "\n"))
end

-- Handle character selection input
local function handle_character_selection(session_id, line, state)
    -- Try numeric selection
    local num = tonumber(line)
    if num and state.characters and num >= 1 and num <= #state.characters then
        local selected = state.characters[math.floor(num)]
        local ok, char_detail = pcall(function()
            return auth:load_character(selected.id)
        end)
        if ok then
            spawn_character(session_id, char_detail, state.account)
            login_state[session_id] = nil
        else
            output:send(session_id, colors.red .. "캐릭터 로드 실패: " .. tostring(char_detail) .. colors.reset)
        end
        return
    end

    -- Treat as new character name — start race selection
    local name = line
    if #name < 2 then
        output:send(session_id, "이름은 2글자 이상이어야 합니다.")
        return
    end

    state.new_char_name = name
    state.step = "race_select"
    show_race_selection(session_id)
end

-- Welcome banner
local WELCOME_MSG = colors.bold .. colors.cyan
    .. "========================================\n"
    .. "     환영합니다, 모험가여!\n"
    .. "========================================"
    .. colors.reset .. "\n"
    .. "이름을 입력하세요: "

-------------------------------------------------------
-- Hook registrations
-------------------------------------------------------

hooks.on_tick(function(tick)
    current_tick = tick
end)

hooks.on_connect(function(session_id)
    login_state[session_id] = {step = "name"}
    output:send(session_id, WELCOME_MSG)
end)

hooks.on_input(function(session_id, line)
    local state = login_state[session_id]
    if not state then return end

    -- Trim whitespace
    line = line:match("^%s*(.-)%s*$") or ""
    if #line == 0 then return end

    if state.step == "name" then
        if auth then
            -- Auth mode: check if account exists
            local ok, existing = pcall(function()
                return auth:check_account(line)
            end)
            if not ok then
                output:send(session_id, colors.red .. "오류: " .. tostring(existing) .. colors.reset)
                return
            end

            state.username = line
            if existing then
                state.step = "password"
                output:send(session_id, "비밀번호를 입력하세요: ")
            else
                state.step = "password_new"
                output:send(session_id, "새 계정을 만듭니다. 비밀번호를 입력하세요: ")
            end
        else
            -- Quick-play mode: name -> race selection
            state.player_name = line
            state.step = "race_select"
            show_race_selection(session_id)
        end

    elseif state.step == "password" then
        local ok, result = pcall(function()
            return auth:authenticate(state.username, line)
        end)
        if ok then
            state.account = result
            enter_character_selection(session_id, state)
        else
            output:send(session_id, colors.red .. "비밀번호가 틀렸습니다." .. colors.reset .. " 다시 입력하세요: ")
        end

    elseif state.step == "password_new" then
        state.password = line
        state.step = "password_confirm"
        output:send(session_id, "비밀번호를 한번 더 입력하세요: ")

    elseif state.step == "password_confirm" then
        if line == state.password then
            local ok, result = pcall(function()
                return auth:create_account(state.username, line)
            end)
            if ok then
                state.account = result
                state.password = nil
                enter_character_selection(session_id, state)
            else
                output:send(session_id, colors.red .. "계정 생성 실패: " .. tostring(result) .. colors.reset)
                state.step = "name"
                output:send(session_id, "이름을 입력하세요: ")
            end
        else
            output:send(session_id, colors.red .. "비밀번호가 일치하지 않습니다." .. colors.reset)
            state.step = "password_new"
            output:send(session_id, "비밀번호를 입력하세요: ")
        end

    elseif state.step == "character_select" then
        handle_character_selection(session_id, line, state)

    elseif state.step == "race_select" then
        local num = tonumber(line)
        if num and num >= 1 and num <= #RACE_LIST then
            state.selected_race = RACE_LIST[math.floor(num)]
            state.step = "class_select"
            show_class_selection(session_id)
        else
            output:send(session_id, "1~" .. tostring(#RACE_LIST) .. " 사이의 번호를 입력하세요.")
        end

    elseif state.step == "class_select" then
        local num = tonumber(line)
        if num and num >= 1 and num <= #CLASS_LIST then
            local selected_class = CLASS_LIST[math.floor(num)]

            if auth then
                -- Auth mode: create character with race/class stats
                local race_id = state.selected_race
                local class_id = selected_class

                -- Calculate stats for DB storage
                local base_hp = 100
                local base_atk = 10
                local base_def = 5
                local race_def = get_race_def(race_id)
                local class_def = get_class_def(class_id)
                if race_def then
                    base_hp = base_hp + (race_def.hp_bonus or 0)
                    base_atk = base_atk + (race_def.attack_bonus or 0)
                    base_def = base_def + (race_def.defense_bonus or 0)
                end
                if class_def then
                    base_hp = base_hp + (class_def.hp_bonus or 0)
                    base_atk = base_atk + (class_def.attack_bonus or 0)
                    base_def = base_def + (class_def.defense_bonus or 0)
                end

                -- Build skills list
                local skills = {}
                if class_def and class_def.starting_skills then
                    for _, s in ipairs(class_def.starting_skills) do
                        table.insert(skills, s)
                    end
                end
                if race_def and race_def.racial_skill then
                    local already = false
                    for _, s in ipairs(skills) do
                        if s == race_def.racial_skill then already = true; break end
                    end
                    if not already then
                        table.insert(skills, race_def.racial_skill)
                    end
                end

                local char_defaults = {
                    Health = {current = base_hp, max = base_hp},
                    Attack = base_atk,
                    Defense = base_def,
                    Race = race_id,
                    Class = class_id,
                    Level = {level = 1, exp = 0, exp_next = 100},
                    Skills = {learned = skills},
                }

                local ok, result = pcall(function()
                    return auth:create_character(state.account.id, state.new_char_name, char_defaults)
                end)

                if ok then
                    output:send(session_id, colors.green .. "캐릭터 '" .. state.new_char_name .. "'이(가) 생성되었습니다! (" .. race_id .. "/" .. class_id .. ")" .. colors.reset)
                    spawn_character(session_id, result, state.account)
                    login_state[session_id] = nil
                else
                    output:send(session_id, colors.red .. "캐릭터 생성 실패: " .. tostring(result) .. colors.reset)
                end
            else
                -- Quick-play mode: spawn with race/class
                spawn_quick_play(session_id, state.player_name, state.selected_race, selected_class)
                login_state[session_id] = nil
            end
        else
            output:send(session_id, "1~" .. tostring(#CLASS_LIST) .. " 사이의 번호를 입력하세요.")
        end
    end
end)

hooks.on_disconnect(function(session_id)
    -- Clean up login sub-state
    login_state[session_id] = nil

    local session_state = sessions:get_state(session_id)
    if session_state ~= "playing" then
        -- Not yet playing; Rust fallback handles cleanup
        return
    end

    local entity = sessions:get_entity(session_id)
    local character_id = sessions:get_character_id(session_id)
    local account_id = sessions:get_account_id(session_id)
    local name = sessions:get_name(session_id)

    if auth and character_id and account_id and entity then
        -- Auth mode: keep entity in-world for reconnection (lingering)
        sessions:add_lingering(entity, character_id, account_id, current_tick)

        -- Remove session so Rust fallback won't despawn the entity
        sessions:disconnect(session_id)
        sessions:remove_session(session_id)

        log.info("Player '" .. (name or "?") .. "' disconnected (lingering, char_id=" .. tostring(character_id) .. ")")
    end
    -- Non-auth mode: Rust fallback will despawn entity and remove session
end)
