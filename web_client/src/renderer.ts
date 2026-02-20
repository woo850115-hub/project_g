import { Application, Container, Graphics, Text, TextStyle } from "pixi.js";
import type { GameState, EntityState } from "./state";

const CELL_SIZE = 16;
const LERP_FACTOR = 0.18;
const SELF_COLOR = 0x0f9b58;
const OTHER_COLOR = 0x4285f4;
const ENTITY_RADIUS = 6;
const GRID_LINE_COLOR = 0x222233;
const BG_COLOR = 0x0f0f23;

interface EntitySprite {
  container: Container;
  circle: Graphics;
  label: Text;
}

export class GameRenderer {
  private app: Application;
  private worldContainer!: Container;
  private gridGraphics!: Graphics;
  private entitySprites: Map<number, EntitySprite> = new Map();
  private initialized = false;

  constructor() {
    this.app = new Application();
  }

  async init(canvasContainer: HTMLElement): Promise<void> {
    await this.app.init({
      background: BG_COLOR,
      resizeTo: canvasContainer,
      antialias: true,
    });
    canvasContainer.appendChild(this.app.canvas);

    this.worldContainer = new Container();
    this.app.stage.addChild(this.worldContainer);

    this.gridGraphics = new Graphics();
    this.worldContainer.addChild(this.gridGraphics);

    this.app.ticker.add(this.onFrame);
    this.initialized = true;
  }

  private state: GameState | null = null;

  setGameState(state: GameState): void {
    this.state = state;
  }

  syncEntities(state: GameState): void {
    // Add new sprites
    for (const [id, ent] of state.entities) {
      if (!this.entitySprites.has(id)) {
        this.addEntitySprite(ent);
      }
    }

    // Remove old sprites
    for (const [id, sprite] of this.entitySprites) {
      if (!state.entities.has(id)) {
        this.worldContainer.removeChild(sprite.container);
        sprite.container.destroy({ children: true });
        this.entitySprites.delete(id);
      }
    }
  }

  clear(): void {
    for (const [, sprite] of this.entitySprites) {
      this.worldContainer.removeChild(sprite.container);
      sprite.container.destroy({ children: true });
    }
    this.entitySprites.clear();
  }

  private addEntitySprite(ent: EntityState): void {
    const container = new Container();

    const circle = new Graphics();
    const color = ent.isSelf ? SELF_COLOR : OTHER_COLOR;
    circle.circle(0, 0, ENTITY_RADIUS);
    circle.fill({ color });

    const style = new TextStyle({
      fontFamily: "monospace",
      fontSize: 10,
      fill: 0xcccccc,
    });
    const label = new Text({ text: ent.name, style });
    label.anchor.set(0.5, 0);
    label.y = ENTITY_RADIUS + 2;

    container.addChild(circle);
    container.addChild(label);

    container.x = ent.renderX * CELL_SIZE;
    container.y = ent.renderY * CELL_SIZE;

    this.worldContainer.addChild(container);
    this.entitySprites.set(ent.id, { container, circle, label });
  }

  private onFrame = (): void => {
    if (!this.state || !this.initialized) return;

    // Lerp entity positions
    for (const [id, ent] of this.state.entities) {
      ent.renderX += (ent.x - ent.renderX) * LERP_FACTOR;
      ent.renderY += (ent.y - ent.renderY) * LERP_FACTOR;

      const sprite = this.entitySprites.get(id);
      if (sprite) {
        sprite.container.x = ent.renderX * CELL_SIZE;
        sprite.container.y = ent.renderY * CELL_SIZE;
      }
    }

    // Camera: center on self entity
    const selfEnt = this.findSelfEntity();
    if (selfEnt) {
      const screenW = this.app.screen.width;
      const screenH = this.app.screen.height;
      this.worldContainer.x = screenW / 2 - selfEnt.renderX * CELL_SIZE;
      this.worldContainer.y = screenH / 2 - selfEnt.renderY * CELL_SIZE;
    }

    // Draw grid lines around the camera view
    this.drawGrid();
  };

  private findSelfEntity(): EntityState | null {
    if (!this.state) return null;
    for (const ent of this.state.entities.values()) {
      if (ent.isSelf) return ent;
    }
    return null;
  }

  private drawGrid(): void {
    const g = this.gridGraphics;
    g.clear();

    const screenW = this.app.screen.width;
    const screenH = this.app.screen.height;
    const offsetX = this.worldContainer.x;
    const offsetY = this.worldContainer.y;

    // Calculate visible world bounds
    const worldLeft = -offsetX - CELL_SIZE;
    const worldTop = -offsetY - CELL_SIZE;
    const worldRight = worldLeft + screenW + CELL_SIZE * 2;
    const worldBottom = worldTop + screenH + CELL_SIZE * 2;

    // Snap to grid
    const startX = Math.floor(worldLeft / CELL_SIZE) * CELL_SIZE;
    const startY = Math.floor(worldTop / CELL_SIZE) * CELL_SIZE;

    g.setStrokeStyle({ width: 1, color: GRID_LINE_COLOR, alpha: 0.4 });

    // Vertical lines
    for (let x = startX; x <= worldRight; x += CELL_SIZE) {
      g.moveTo(x, worldTop);
      g.lineTo(x, worldBottom);
    }
    // Horizontal lines
    for (let y = startY; y <= worldBottom; y += CELL_SIZE) {
      g.moveTo(worldLeft, y);
      g.lineTo(worldRight, y);
    }
    g.stroke();
  }
}
