-- 01_world_setup.lua: Create the starter world on first boot (skipped on snapshot restore)

hooks.on_init(function()
    -- Skip if world already exists (snapshot restore)
    if space:room_count() > 0 then
        log.info("World already loaded from snapshot, skipping creation")
        return
    end

    log.info("Creating starter world...")

    -- Create room entities
    local spawn_room = ecs:spawn()
    local market_square = ecs:spawn()
    local dark_alley = ecs:spawn()
    local weapon_shop = ecs:spawn()
    local dungeon_entrance = ecs:spawn()
    local dungeon_floor1 = ecs:spawn()

    -- Set room names and descriptions
    ecs:set(spawn_room, "Name", "시작의 방")
    ecs:set(spawn_room, "Description", "따뜻하고 환한 방입니다. 벽에 안내문이 붙어 있습니다: '환영합니다, 모험가여!'")

    ecs:set(market_square, "Name", "시장 광장")
    ecs:set(market_square, "Description", "활기찬 시장 광장입니다. 상인들이 물건을 팔고 있습니다.")

    ecs:set(dark_alley, "Name", "어두운 골목")
    ecs:set(dark_alley, "Description", "좁고 어두운 골목입니다. 쥐들이 달아나는 소리가 들립니다.")

    ecs:set(weapon_shop, "Name", "무기 상점")
    ecs:set(weapon_shop, "Description", "벽에 칼, 도끼, 활이 진열되어 있습니다. 주인이 당신을 지켜보고 있습니다.")

    ecs:set(dungeon_entrance, "Name", "던전 입구")
    ecs:set(dungeon_entrance, "Description", "어두운 계단이 지하로 내려갑니다. 차가운 바람이 올라오고 있습니다.")

    ecs:set(dungeon_floor1, "Name", "던전 1층")
    ecs:set(dungeon_floor1, "Description", "축축한 석조 방입니다. 횃불이 벽에서 흔들리고 있습니다.")

    -- Register rooms with exits
    -- 시작의 방 <-> 시장 광장 (east/west)
    -- 시장 광장 <-> 어두운 골목 (east/west)
    -- 시장 광장 <-> 무기 상점 (south/north)
    -- 무기 상점 <-> 던전 입구 (south/north)
    -- 던전 입구 <-> 던전 1층 (east/west)

    space:register_room(spawn_room, {east = market_square})
    space:register_room(market_square, {west = spawn_room, east = dark_alley, south = weapon_shop})
    space:register_room(dark_alley, {west = market_square})
    space:register_room(weapon_shop, {north = market_square, south = dungeon_entrance})
    space:register_room(dungeon_entrance, {north = weapon_shop, east = dungeon_floor1})
    space:register_room(dungeon_floor1, {west = dungeon_entrance})

    -- Create Goblin NPC in Dungeon Floor 1
    local goblin = ecs:spawn()
    ecs:set(goblin, "Name", "고블린")
    ecs:set(goblin, "Description", "으르렁거리는 고블린이 녹슨 단검을 들고 있습니다.")
    ecs:set(goblin, "NpcTag", true)
    ecs:set(goblin, "Health", {current = 30, max = 30})
    ecs:set(goblin, "Attack", 8)
    ecs:set(goblin, "Defense", 2)
    space:place_entity(goblin, dungeon_floor1)

    -- Create Potion item in Market Square
    local potion = ecs:spawn()
    ecs:set(potion, "Name", "치유 물약")
    ecs:set(potion, "Description", "체력을 회복시켜주는 빨간 액체가 담긴 작은 병입니다.")
    ecs:set(potion, "ItemTag", true)
    space:place_entity(potion, market_square)

    log.info("Starter world created: 6 rooms, 1 NPC, 1 item")
end)
