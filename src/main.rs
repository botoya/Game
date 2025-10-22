use ggez::event;
use ggez::input::keyboard::{is_key_pressed, KeyCode};
use ggez::input::mouse::MouseButton;
use ggez::timer;
use ggez::{Context, GameResult};
use ggez::{graphics};
// note: this file uses rand::Rng; add `rand = "0.8"` to Cargo.toml if missing
use ggez::graphics::{DrawParam, Image};
const COIN_SIZE: f32 = 16.0;

const TILE_SIZE: f32 = 32.0;
const GRAVITY: f32 = 1200.0;
const MOVE_SPEED: f32 = 200.0;
const JUMP_V: f32 = -420.0;

// 关卡数据和特殊方块位置（格子坐标）
const LEVEL: [&str; 7] = [
    "............................",
    "............................",
    "............................",
    "...........##...............",
    "..................##........",
    "......##....................",
    "############################",
];

const SPECIAL_POSITIONS: &[(usize, usize)] = &[(8usize, 2usize), (15usize, 2usize)];

enum Screen {
    Menu,
    GameOver,
    Playing,
}

struct Player {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    vx: f32,
    vy: f32,
    on_ground: bool,
}

impl Player {
    fn rect(&self) -> graphics::Rect {
        graphics::Rect::new(self.x, self.y, self.w, self.h)
    }
}

struct GameState {
    screen: Screen,
    player: Player,
    tiles: Vec<graphics::Rect>, // 平台块位置
    tile_img: Image,
    player_img: Image,
    special_img: Image,
    // special_blocks now stores grid positions (col,row)
    special_blocks: Vec<(usize, usize)>,
    // coins: rect + its grid position (col,row)
    coins: Vec<(graphics::Rect, (usize, usize))>,
    coin_img: Image,
    score: i32,
    coin_spawn_timer: f32,
    coin_spawn_interval: f32,
    // level vertical offset used to compute grid rows
    level_offset_y: f32,
    // positions where coin has been collected; won't respawn there
    consumed_coin_positions: Vec<(usize, usize)>,
    // monsters (enemies)
    monsters: Vec<Monster>,
    monster_img: Image,
}

// 小怪兽结构体：带有巡逻范围
struct Monster {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    vx: f32,
    range_min: f32,
    range_max: f32,
}

impl Monster {
    fn rect(&self) -> graphics::Rect {
        graphics::Rect::new(self.x, self.y, self.w, self.h)
    }
}

impl GameState {
    // new 需要 Context 用来加载图片资源
    fn new(ctx: &mut Context) -> GameResult<Self> {
        let level = LEVEL;

        let mut tiles = Vec::new();
        // 使关卡底部对齐到窗口底部：计算整个关卡像素高度，然后从窗口高度减去它作为起始偏移
        let (_win_w, win_h) = graphics::drawable_size(ctx);
        let rows = level.len() as f32;
        let level_px_h = rows * TILE_SIZE;
        // 如果关卡高度比窗口高，offset_y 允许为负，从而保持原始布局
        let offset_y = win_h - level_px_h;

        for (row, line) in level.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if ch == '#' {
                    let y = offset_y + (row as f32) * TILE_SIZE;
                    let r = graphics::Rect::new(
                        col as f32 * TILE_SIZE,
                        y,
                        TILE_SIZE,
                        TILE_SIZE,
                    );
                    tiles.push(r);
                }
            }
        }

    // 加载资源（确保 resources/stock.png、player.png、special_block.png、coin.png 存在）
    let tile_img = Image::new(ctx, "/stock.png")?;
    let player_img = Image::new(ctx, "/player.png")?;
    let special_img = Image::new(ctx, "/special_block.png")?;
    let coin_img = Image::new(ctx, "/coin.png")?;
    // 怪物素材（resources/boast.png）
    let monster_img = Image::new(ctx, "/boast.png")?;

        let player = Player {
            x: 50.0,
            y: 50.0,
            w: 24.0,
            h: 30.0,
            vx: 0.0,
            vy: 0.0,
            on_ground: false,
        };

        // 在靠近地面的地方生成一个巡逻怪
        let mut monsters = Vec::new();
        // 找到第一个靠近底部的 tile，用其上方生成怪物
        if let Some(t) = tiles.iter().find(|t| t.y >= (offset_y + (level.len() as f32 - 1.0) * TILE_SIZE) - 1.0) {
            let mx = t.x + 4.0;
            let my = t.y - 24.0;
            monsters.push(Monster { x: mx, y: my, w: 24.0, h: 24.0, vx: 60.0, range_min: t.x, range_max: t.x + TILE_SIZE * 4.0 });
        }

    // 添加几个特殊方块（示例放在第 3 行和第 4 行的特定列）
    let special_positions = SPECIAL_POSITIONS;
    let mut special_blocks = Vec::new();
    for (col, row) in special_positions.iter() {
            let y = offset_y + (*row as f32) * TILE_SIZE;
            let rect = graphics::Rect::new(*col as f32 * TILE_SIZE, y, TILE_SIZE, TILE_SIZE);
            // 同时把它加入 tiles（保证为实心方块）
            tiles.push(rect);
            special_blocks.push((*col, *row));
        }

        Ok(Self {
            screen: Screen::Menu,
            player,
            tiles,
            tile_img,
            player_img,
            special_blocks,
            special_img,
            coins: Vec::new(),
            coin_img,
            score: 0,
            coin_spawn_timer: 0.0,
            coin_spawn_interval: 5.0,
            level_offset_y: offset_y,
            consumed_coin_positions: Vec::new(),
            monsters,
            monster_img,
        })
    }

    // 重置一局（用于开始新游戏）
    fn reset_game(&mut self) {
        // 重新构建 tiles 与 special_blocks
        self.tiles.clear();
        let level = LEVEL;
        for (row, line) in level.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if ch == '#' {
                    let y = self.level_offset_y + (row as f32) * TILE_SIZE;
                    let r = graphics::Rect::new(col as f32 * TILE_SIZE, y, TILE_SIZE, TILE_SIZE);
                    self.tiles.push(r);
                }
            }
        }
        self.special_blocks.clear();
        for (col, row) in SPECIAL_POSITIONS.iter() {
            self.special_blocks.push((*col, *row));
            // 同步把特殊方块的矩形也加入 tiles，保证可碰撞
            let y = self.level_offset_y + (*row as f32) * TILE_SIZE;
            let rect = graphics::Rect::new(*col as f32 * TILE_SIZE, y, TILE_SIZE, TILE_SIZE);
            self.tiles.push(rect);
        }
        self.coins.clear();
        self.consumed_coin_positions.clear();
        self.score = 0;
        self.coin_spawn_timer = 0.0;
        self.player = Player { x:50.0, y:50.0, w:24.0, h:30.0, vx:0.0, vy:0.0, on_ground:false };
        // 重新生成怪物（简单策略：放在第一个底部 tile 上方）
        self.monsters.clear();
        if let Some(t) = self.tiles.iter().find(|t| t.y >= (self.level_offset_y + (LEVEL.len() as f32 - 1.0) * TILE_SIZE) - 1.0) {
            let mx = t.x + 4.0;
            let my = t.y - 24.0;
            self.monsters.push(Monster { x: mx, y: my, w: 24.0, h: 24.0, vx: 60.0, range_min: t.x, range_max: t.x + TILE_SIZE * 4.0 });
        }
    }

    // 重置玩家到初始状态（用于结束一把返回菜单）
    fn reset_player(&mut self) {
        self.player = Player {
            x: 50.0,
            y: 50.0,
            w: 24.0,
            h: 30.0,
            vx: 0.0,
            vy: 0.0,
            on_ground: false,
        };
        // 失败重置时也把怪物位置重置为初始
        for m in &mut self.monsters {
            m.vx = m.vx.abs();
            // 将怪物放回 range_min
            m.x = m.range_min + 4.0;
        }
    }

    // 简单 AABB 碰撞检测
    fn rect_intersect(a: &graphics::Rect, b: &graphics::Rect) -> bool {
        a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
    }
}

impl event::EventHandler for GameState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        match self.screen {
            Screen::Menu => {
                // 菜单无每帧逻辑（可加入动画）
            }
            Screen::GameOver => {
                // 游戏结束时暂停一切游戏逻辑
            }
            Screen::Playing => {
                let dt = timer::delta(ctx).as_secs_f32();
                let (_win_w, win_h) = graphics::drawable_size(ctx);

                // 输入
                let mut move_x = 0.0;
                if is_key_pressed(ctx, KeyCode::Left) || is_key_pressed(ctx, KeyCode::A) {
                    move_x -= 1.0;
                }
                if is_key_pressed(ctx, KeyCode::Right) || is_key_pressed(ctx, KeyCode::D) {
                    move_x += 1.0;
                }
                if (is_key_pressed(ctx, KeyCode::Space)
                    || is_key_pressed(ctx, KeyCode::W)
                    || is_key_pressed(ctx, KeyCode::Up))
                    && self.player.on_ground
                {
                    self.player.vy = JUMP_V;
                    self.player.on_ground = false;
                }

                // 水平速度
                self.player.vx = move_x * MOVE_SPEED;

                // 应用重力
                self.player.vy += GRAVITY * dt;

                // 先移动水平并检测水平碰撞
                self.player.x += self.player.vx * dt;
                let mut prect = self.player.rect();
                for tile in &self.tiles {
                    if GameState::rect_intersect(&prect, tile) {
                        if self.player.vx > 0.0 {
                            self.player.x = tile.x - self.player.w;
                        } else if self.player.vx < 0.0 {
                            self.player.x = tile.x + tile.w;
                        }
                        self.player.vx = 0.0;
                        prect = self.player.rect();
                    }
                }

                // 然后移动垂直并检测垂直碰撞
                self.player.y += self.player.vy * dt;
                prect = self.player.rect();

                // 地面随机刷新金币（周期性）
                self.coin_spawn_timer += dt;
                if self.coin_spawn_timer >= self.coin_spawn_interval {
                    self.coin_spawn_timer = 0.0;
                    // 找到底部的 tiles（y 接近窗口底部）
                    let ground_tiles: Vec<&graphics::Rect> = self
                        .tiles
                        .iter()
                        .filter(|t| t.y >= win_h - TILE_SIZE - 1.0)
                        .collect();
                    if !ground_tiles.is_empty() {
                        // 选择中间的一个地面块刷金币，避免引入 rand 依赖
                        let idx = ground_tiles.len() / 2;
                        let t = ground_tiles[idx];
                        let coin_x = t.x + (TILE_SIZE - COIN_SIZE) / 2.0;
                        let coin_y = t.y - COIN_SIZE - 2.0;
                        // 计算格子坐标
                        let col = (t.x / TILE_SIZE) as usize;
                        let row = ((t.y - self.level_offset_y) / TILE_SIZE) as usize;
                        let exists = self.coins.iter().any(|(c, _)| (c.x - coin_x).abs() < 0.1 && (c.y - coin_y).abs() < 0.1);
                        let consumed = self.consumed_coin_positions.iter().any(|(cc, rr)| *cc == col && *rr == row);
                        if !exists && !consumed {
                            self.coins.push((graphics::Rect::new(coin_x, coin_y, COIN_SIZE, COIN_SIZE), (col, row)));
                        }
                    }
                }
                self.player.on_ground = false;
                for tile in &self.tiles {
                    if GameState::rect_intersect(&prect, tile) {
                        if self.player.vy > 0.0 {
                            self.player.y = tile.y - self.player.h;
                            self.player.vy = 0.0;
                            self.player.on_ground = true;
                        } else if self.player.vy < 0.0 {
                            // 从下面顶到方块的处理：若是特殊方块，生成金币
                            self.player.y = tile.y + tile.h;
                            // 检查是否为特殊方块（比较格子坐标）
                            let col = (tile.x / TILE_SIZE) as usize;
                            let row = ((tile.y - self.level_offset_y) / TILE_SIZE) as usize;
                            let is_special = self.special_blocks.iter().any(|(sc, sr)| *sc == col && *sr == row);
                            if is_special {
                                let coin_x = tile.x + (TILE_SIZE - COIN_SIZE) / 2.0;
                                let coin_y = tile.y - COIN_SIZE - 2.0;
                                // 只有当该位置没有金币且未被消耗时才生成
                                let exists = self.coins.iter().any(|(c, _)| (c.x - coin_x).abs() < 0.1 && (c.y - coin_y).abs() < 0.1);
                                let consumed = self.consumed_coin_positions.iter().any(|(cc, rr)| *cc == col && *rr == row);
                                if !exists && !consumed {
                                    self.coins.push((graphics::Rect::new(coin_x, coin_y, COIN_SIZE, COIN_SIZE), (col, row)));
                                }
                                // 把这个特殊方块变回普通瓷块（从 special_blocks 中移除）
                                self.special_blocks.retain(|(sc, sr)| !(*sc == col && *sr == row));
                            }
                            self.player.vy = 0.0;
                        }
                        prect = self.player.rect();
                    }
                }

                // 限制在窗口内（简单处理）
                let (w, h) = graphics::drawable_size(ctx);
                if self.player.x < 0.0 {
                    self.player.x = 0.0;
                }
                if self.player.x + self.player.w > w {
                    self.player.x = w - self.player.w;
                }
                if self.player.y + self.player.h > h {
                    self.player.y = h - self.player.h;
                    self.player.vy = 0.0;
                    self.player.on_ground = true;
                }

                // 固定帧率
                while timer::check_update_time(ctx, 60) {
                    // nothing
                }

                // 拾取金币检测：玩家与金币相交则得分并移除金币
                let pre_player = self.player.rect();
                self.coins.retain(|(coin_rect, grid)| {
                    if GameState::rect_intersect(&pre_player, coin_rect) {
                        self.score += 10;
                        // 触发下一周期立即刷新的机会：把计时器设为间隔
                        self.coin_spawn_timer = self.coin_spawn_interval;
                        // 记录该格子已被消耗，未来不再刷新
                        self.consumed_coin_positions.push(*grid);
                        false
                    } else {
                        true
                    }
                });

                // 更新怪物巡逻与与玩家碰撞检测
                for m in &mut self.monsters {
                    // 移动
                    m.x += m.vx * dt;
                    if m.x < m.range_min {
                        m.x = m.range_min;
                        m.vx = m.vx.abs();
                    } else if m.x + m.w > m.range_max {
                        m.x = m.range_max - m.w;
                        m.vx = -m.vx.abs();
                    }
                    // 简单重力作用 (保持在 tile 上方)
                    // 检查是否站在某个 tile 上
                    let mut on_tile = false;
                    for tile in &self.tiles {
                        let mut mrect = m.rect();
                        mrect.y += 1.0; // 向下检测
                        if GameState::rect_intersect(&mrect, tile) {
                            on_tile = true;
                            // 将怪物固定在地面上
                            m.y = tile.y - m.h;
                            break;
                        }
                    }
                    if !on_tile {
                        // 自由落体
                        m.y += GRAVITY * dt;
                    }
                    // 玩家碰撞检测 -> 进入 GameOver
                    if GameState::rect_intersect(&self.player.rect(), &m.rect()) {
                        self.screen = Screen::GameOver;
                    }
                }
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let bg = graphics::Color::from_rgb(100, 149, 237);
        graphics::clear(ctx, bg);

        match self.screen {
            Screen::Menu => {
                // 菜单背景和标题
                let title = graphics::Text::new("Super Mario");
                let (w, h) = graphics::drawable_size(ctx);

                // 画标题（靠上）
                graphics::draw(
                    ctx,
                    &title,
                    DrawParam::default().dest([w / 2.0 - 140.0, h / 4.0]),
                )?;

                // Start 按钮位置与尺寸
                let btn_w = 160.0;
                let btn_h = 48.0;
                let bx = w / 2.0 - btn_w / 2.0;
                let by = h / 2.0 - btn_h / 2.0;

                // 按钮矩形
                let rect = graphics::Rect::new(bx, by, btn_w, btn_h);
                let mesh = graphics::Mesh::new_rectangle(
                    ctx,
                    graphics::DrawMode::fill(),
                    rect,
                    graphics::Color::from_rgb(80, 160, 80),
                )?;
                graphics::draw(ctx, &mesh, DrawParam::default())?;

                // 按钮文字
                let label = graphics::Text::new("START");
                graphics::draw(ctx, &label, DrawParam::default().dest([bx + 28.0, by + 12.0]))?;
            }
            Screen::Playing => {
                // 画瓷砖（使用图片，按 TILE_SIZE 缩放）
                for tile in &self.tiles {
                    let sx = TILE_SIZE / (self.tile_img.width() as f32);
                    let sy = TILE_SIZE / (self.tile_img.height() as f32);
                    graphics::draw(
                        ctx,
                        &self.tile_img,
                        DrawParam::default()
                            .dest([tile.x, tile.y])
                            .scale([sx, sy]),
                    )?;
                }

                // 画玩家（使用图片，按 player.w/player.h 缩放）
                let sx = self.player.w / (self.player_img.width() as f32);
                let sy = self.player.h / (self.player_img.height() as f32);
                graphics::draw(
                    ctx,
                    &self.player_img,
                    DrawParam::default()
                        .dest([self.player.x, self.player.y])
                        .scale([sx, sy]),
                )?;

                // 画特殊方块（special_blocks 存储格子坐标）
                for (col, row) in &self.special_blocks {
                    let bx = (*col as f32) * TILE_SIZE;
                    let by = self.level_offset_y + (*row as f32) * TILE_SIZE;
                    let sx = TILE_SIZE / (self.special_img.width() as f32);
                    let sy = TILE_SIZE / (self.special_img.height() as f32);
                    graphics::draw(
                        ctx,
                        &self.special_img,
                        DrawParam::default().dest([bx, by]).scale([sx, sy]),
                    )?;
                }

                // 画金币
                for (coin_rect, _) in &self.coins {
                    let sx = COIN_SIZE / (self.coin_img.width() as f32);
                    let sy = COIN_SIZE / (self.coin_img.height() as f32);
                    graphics::draw(
                        ctx,
                        &self.coin_img,
                        DrawParam::default().dest([coin_rect.x, coin_rect.y]).scale([sx, sy]),
                    )?;
                }

                // HUD 文本：位置和分数
                let text = graphics::Text::new(format!(
                    "pos=({:.0},{:.0}) on_ground={}  score={} ",
                    self.player.x, self.player.y, self.player.on_ground, self.score
                ));
                graphics::draw(ctx, &text, DrawParam::default().dest([8.0, 8.0]))?;

                // 退出按钮（右上）——现在为“结束当前一把并返回菜单”
                let (w, _) = graphics::drawable_size(ctx);
                let btn_w = 80.0;
                let btn_h = 28.0;
                let bx = w - btn_w - 8.0;
                let by = 8.0;
                let rect = graphics::Rect::new(bx, by, btn_w, btn_h);
                let mesh = graphics::Mesh::new_rectangle(
                    ctx,
                    graphics::DrawMode::fill(),
                    rect,
                    graphics::Color::from_rgb(200, 80, 80),
                )?;
                graphics::draw(ctx, &mesh, DrawParam::default())?;
                let label = graphics::Text::new("QUIT");
                graphics::draw(ctx, &label, DrawParam::default().dest([bx + 18.0, by + 6.0]))?;

                // 绘制怪物
                for m in &self.monsters {
                    let sx = m.w / (self.monster_img.width() as f32);
                    let sy = m.h / (self.monster_img.height() as f32);
                    graphics::draw(ctx, &self.monster_img, DrawParam::default().dest([m.x, m.y]).scale([sx, sy]))?;
                }
            }
            Screen::GameOver => {
                let (w, h) = graphics::drawable_size(ctx);
                let title = graphics::Text::new("Game Over");
                graphics::draw(ctx, &title, DrawParam::default().dest([w / 2.0 - 80.0, h / 4.0]))?;

                // 两个按钮：Restart 和 Quit
                let btn_w = 140.0;
                let btn_h = 44.0;
                let bx = w / 2.0 - btn_w / 2.0;
                let by = h / 2.0 - btn_h / 2.0;
                let rect = graphics::Rect::new(bx, by, btn_w, btn_h);
                let mesh = graphics::Mesh::new_rectangle(ctx, graphics::DrawMode::fill(), rect, graphics::Color::from_rgb(200, 80, 80))?;
                graphics::draw(ctx, &mesh, DrawParam::default())?;
                let label = graphics::Text::new("Restart");
                graphics::draw(ctx, &label, DrawParam::default().dest([bx + 36.0, by + 10.0]))?;

                let bx2 = bx;
                let by2 = by + btn_h + 12.0;
                let rect2 = graphics::Rect::new(bx2, by2, btn_w, btn_h);
                let mesh2 = graphics::Mesh::new_rectangle(ctx, graphics::DrawMode::fill(), rect2, graphics::Color::from_rgb(120, 120, 120))?;
                graphics::draw(ctx, &mesh2, DrawParam::default())?;
                let label2 = graphics::Text::new("Quit");
                graphics::draw(ctx, &label2, DrawParam::default().dest([bx2 + 56.0, by2 + 10.0]))?;
            }
        }

        graphics::present(ctx)?;
        Ok(())
    }

    // 处理鼠标点击：菜单点击 Start、游戏界面点击 退出（现在返回菜单并重置玩家）
    fn mouse_button_down_event(
        &mut self,
        ctx: &mut Context,
        button: MouseButton,
        x: f32,
        y: f32,
    ) {
        if button != MouseButton::Left {
            return;
        }

        match self.screen {
            Screen::Menu => {
                let (w, h) = graphics::drawable_size(ctx);
                // 如果窗口大小不确定，可用 graphics::drawable_size(ctx) 改为传入 ctx
                // 这里为了简洁保持和 draw 中一样逻辑：假定使用相同计算方式
                // 建议若需要精确点击检测可把 ctx 作为参数并调用 graphics::drawable_size(ctx)
                let btn_w = 160.0;
                let btn_h = 48.0;
                let bx = w / 2.0 - btn_w / 2.0;
                let by = h / 2.0 - btn_h / 2.0;
                if x >= bx && x <= bx + btn_w && y >= by && y <= by + btn_h {
                    // 点击开始按钮 -> 进入游戏
                    self.reset_game();
                    self.screen = Screen::Playing;
                }
            }
            Screen::Playing => {
                let (w, _) = graphics::drawable_size(ctx);
                let btn_w = 80.0;
                let btn_h = 28.0;
                let bx = w - btn_w - 8.0;
                let by = 8.0;
                if x >= bx && x <= bx + btn_w && y >= by && y <= by + btn_h {
                    // 点击退出按钮 -> 结束本局，返回菜单并重置玩家
                    self.screen = Screen::Menu;
                    self.reset_player();
                }
            }
            Screen::GameOver => {
                let (w, h) = graphics::drawable_size(ctx);
                let btn_w = 140.0;
                let btn_h = 44.0;
                let bx = w / 2.0 - btn_w / 2.0;
                let by = h / 2.0 - btn_h / 2.0;
                // Restart
                if x >= bx && x <= bx + btn_w && y >= by && y <= by + btn_h {
                    self.reset_game();
                    self.screen = Screen::Playing;
                    return;
                }
                // Quit (下方)
                let bx2 = bx;
                let by2 = by + btn_h + 12.0;
                if x >= bx2 && x <= bx2 + btn_w && y >= by2 && y <= by2 + btn_h {
                    self.screen = Screen::Menu;
                    self.reset_player();
                }
            }
        }
    }
}

fn main() -> GameResult {
    // 把资源目录加入 Context（相对路径为项目根）
    let resource_dir = std::path::PathBuf::from("./resources");
    let cb = ggez::ContextBuilder::new("platformer", "example")
        .add_resource_path(resource_dir)
        .window_mode(ggez::conf::WindowMode::default().dimensions(800.0, 400.0));
    let (mut ctx, event_loop) = cb.build()?;
    let state = GameState::new(&mut ctx)?;
    event::run(ctx, event_loop, state)
}
