/**
 * Tetris - Constants and Configuration
 * Defines all game constants: board dimensions, tetromino shapes, colors, SRS wall kick tables, scoring
 */

// ─── Board ───
export const COLS = 10;
export const ROWS = 20;
export const CELL_SIZE = 30;

// ─── Game States ───
export const STATE = {
  IDLE: 'idle',
  PLAYING: 'playing',
  PAUSED: 'paused',
  GAME_OVER: 'gameover',
};

// ─── Tetromino Types ───
export const TYPES = ['I', 'O', 'T', 'S', 'Z', 'J', 'L'];

/**
 * Tetromino shapes defined as 2D arrays.
 * Each shape is represented in its bounding box with the first rotation state (0).
 * Rotation states 0/1/2/3 are computed from these base shapes.
 */
export const SHAPES = {
  I: [
    [0, 0, 0, 0],
    [1, 1, 1, 1],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
  ],
  O: [
    [1, 1],
    [1, 1],
  ],
  T: [
    [0, 1, 0],
    [1, 1, 1],
    [0, 0, 0],
  ],
  S: [
    [0, 1, 1],
    [1, 1, 0],
    [0, 0, 0],
  ],
  Z: [
    [1, 1, 0],
    [0, 1, 1],
    [0, 0, 0],
  ],
  J: [
    [1, 0, 0],
    [1, 1, 1],
    [0, 0, 0],
  ],
  L: [
    [0, 0, 1],
    [1, 1, 1],
    [0, 0, 0],
  ],
};

/**
 * Colors for each tetromino type (classic Tetris palette).
 */
export const COLORS = {
  I: '#00f0f0',
  O: '#f0f000',
  T: '#a000f0',
  S: '#00f000',
  Z: '#f00000',
  J: '#0000f0',
  L: '#f0a000',
};

/**
 * Spawn positions: the top-center of the board.
 * For most pieces, the top-left corner of the bounding box is placed at col=3, row=0.
 * I piece needs col=3, row=0 as well (its actual cells start on row 1).
 * O piece: col=4, row=0.
 */
export const SPAWN = {
  I: { x: 3, y: 0 },
  O: { x: 4, y: 0 },
  T: { x: 3, y: 0 },
  S: { x: 3, y: 0 },
  Z: { x: 3, y: 0 },
  J: { x: 3, y: 0 },
  L: { x: 3, y: 0 },
};

/**
 * SRS Wall Kick Data
 * ===================
 * Offsets to try when a rotation collides.
 * Standard offsets for J, L, S, T, Z pieces.
 * Each entry is an array of [col_offset, row_offset] attempts.
 *
 * Key convention: "0>1" means rotating from state 0 to state 1 (clockwise),
 * "1>0" means rotating from state 1 to state 0 (counterclockwise), etc.
 *
 * Offsets are (col_offset, row_offset) where:
 * - positive col = right
 * - positive row = down
 */
export const WALL_KICKS_JLSZT = {
  '0>1': [[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2]],
  '1>0': [[0, 0], [1, 0], [1, 1], [0, -2], [1, -2]],
  '1>2': [[0, 0], [1, 0], [1, 1], [0, -2], [1, -2]],
  '2>1': [[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2]],
  '2>3': [[0, 0], [1, 0], [1, -1], [0, 2], [1, 2]],
  '3>2': [[0, 0], [-1, 0], [-1, 1], [0, -2], [-1, -2]],
  '3>0': [[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2]],
  '0>3': [[0, 0], [1, 0], [1, 1], [0, -2], [1, -2]],
};

/**
 * I-piece has its own wall kick table (different offsets).
 */
export const WALL_KICKS_I = {
  '0>1': [[0, 0], [-2, 0], [1, 0], [-2, 1], [1, -2]],
  '1>0': [[0, 0], [2, 0], [-1, 0], [2, -1], [-1, 2]],
  '1>2': [[0, 0], [-1, 0], [2, 0], [-1, -2], [2, 1]],
  '2>1': [[0, 0], [1, 0], [-2, 0], [1, 2], [-2, -1]],
  '2>3': [[0, 0], [2, 0], [-1, 0], [2, -1], [-1, 2]],
  '3>2': [[0, 0], [-2, 0], [1, 0], [-2, 1], [1, -2]],
  '3>0': [[0, 0], [1, 0], [-2, 0], [1, 2], [-2, -1]],
  '0>3': [[0, 0], [-1, 0], [2, 0], [-1, -2], [2, 1]],
};

/**
 * Scoring table for line clears.
 */
export const LINE_SCORES = {
  1: 100,
  2: 300,
  3: 500,
  4: 800,
};

/**
 * Soft drop score per cell.
 */
export const SOFT_DROP_SCORE = 1;
export const HARD_DROP_SCORE = 2;

/**
 * Lines needed per level-up.
 */
export const LINES_PER_LEVEL = 10;

/**
 * Maximum level cap.
 */
export const MAX_LEVEL = 15;

/**
 * Drop interval calculation: returns ms between automatic drops.
 * @param {number} level - Current level (1-based)
 * @returns {number} Drop interval in milliseconds
 */
export function getDropInterval(level) {
  return Math.max(100, 1000 - (level - 1) * 80);
}

/**
 * DAS (Delayed Auto Shift) settings for key repeat.
 */
export const DAS_DELAY = 170;  // ms before repeat starts
export const DAS_INTERVAL = 50; // ms between repeats
