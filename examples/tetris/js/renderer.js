/**
 * Tetris - Renderer
 * Canvas-based rendering for the game board, pieces, ghost piece, and HUD.
 */
import { COLS, ROWS, CELL_SIZE, COLORS, TYPES } from './constants.js';

// Grid appearance
const GRID_LINE_COLOR = '#333333';
const BG_COLOR = '#111111';
const GHOST_ALPHA = 0.3;
const BOARD_BORDER_COLOR = '#555555';
const BOARD_BORDER_WIDTH = 2;

export class Renderer {
  /**
   * @param {HTMLCanvasElement} canvas - Main game canvas
   * @param {HTMLCanvasElement} previewCanvas - Next piece preview canvas
   */
  constructor(canvas, previewCanvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');

    this.previewCanvas = previewCanvas;
    this.previewCtx = previewCanvas.getContext('2d');

    // Set canvas dimensions
    canvas.width = COLS * CELL_SIZE;
    canvas.height = ROWS * CELL_SIZE;

    // Preview canvas size: 4 cells wide, 4 cells tall
    const previewSize = 4 * CELL_SIZE;
    previewCanvas.width = previewSize;
    previewCanvas.height = previewSize;

    // Reference to the game (set externally)
    this.game = null;
  }

  /**
   * Main render call. Renders everything for the current game state.
   * @param {object} game - The Game instance
   */
  render(game) {
    this.game = game;
    this._clearCanvas();

    // Draw board background
    this._drawBoard();

    // Draw locked cells
    this._drawLockedCells(game.board.grid);

    // Draw ghost piece
    if (game.currentPiece && game.state !== 'gameover') {
      this._drawGhostPiece(game.currentPiece, game.board);
    }

    // Draw current piece
    if (game.currentPiece && game.state !== 'gameover') {
      this._drawPiece(game.currentPiece);
    }

    // Draw grid lines
    this._drawGridLines();

    // Draw board border
    this._drawBorder();

    // Draw overlay for special states
    if (game.state === 'idle') {
      this._drawOverlay('TETRIS', 'Press ENTER to start');
    } else if (game.state === 'paused') {
      this._drawOverlay('PAUSED', 'Press P to resume');
    } else if (game.state === 'gameover') {
      this._drawOverlay('GAME OVER', `Score: ${game.score.score}\nPress ENTER to restart`);
    }

    // Draw next piece preview
    if (game.nextPiece) {
      this._drawPreview(game.nextPiece);
    }

    // Update HTML-based HUD
    this._updateHUD(game.score);
  }

  /**
   * Clear the main canvas.
   */
  _clearCanvas() {
    this.ctx.fillStyle = BG_COLOR;
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
  }

  /**
   * Draw the board background.
   */
  _drawBoard() {
    this.ctx.fillStyle = '#1a1a1a';
    this.ctx.fillRect(0, 0, COLS * CELL_SIZE, ROWS * CELL_SIZE);
  }

  /**
   * Draw locked cells from the grid.
   * @param {number[][]} grid
   */
  _drawLockedCells(grid) {
    for (let row = 0; row < ROWS; row++) {
      for (let col = 0; col < COLS; col++) {
        const cell = grid[row][col];
        if (cell !== 0) {
          const type = TYPES[cell - 1];
          this._drawCell(col, row, COLORS[type]);
        }
      }
    }
  }

  /**
   * Draw a single cell at (col, row) with a given color.
   * Adds a subtle 3D highlight/shadow effect.
   * @param {number} col
   * @param {number} row
   * @param {string} color
   * @param {number} [alpha=1]
   */
  _drawCell(col, row, color, alpha = 1) {
    const x = col * CELL_SIZE;
    const y = row * CELL_SIZE;
    const size = CELL_SIZE;
    const inset = 1; // 1px gap between cells

    this.ctx.globalAlpha = alpha;

    // Main fill
    this.ctx.fillStyle = color;
    this.ctx.fillRect(x + inset, y + inset, size - inset * 2, size - inset * 2);

    // Highlight (top-left)
    this.ctx.fillStyle = 'rgba(255, 255, 255, 0.25)';
    this.ctx.fillRect(x + inset, y + inset, size - inset * 2, 2);
    this.ctx.fillRect(x + inset, y + inset, 2, size - inset * 2);

    // Shadow (bottom-right)
    this.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    this.ctx.fillRect(x + inset, y + size - inset - 2, size - inset * 2, 2);
    this.ctx.fillRect(x + size - inset - 2, y + inset, 2, size - inset * 2);

    this.ctx.globalAlpha = 1;
  }

  /**
   * Draw a tetromino piece at its current position.
   * @param {object} piece - Tetromino instance
   */
  _drawPiece(piece) {
    const shape = piece.getShape();
    const color = piece.color;

    for (let row = 0; row < shape.length; row++) {
      for (let col = 0; col < shape[row].length; col++) {
        if (shape[row][col] !== 0) {
          const boardX = piece.x + col;
          const boardY = piece.y + row;

          // Only draw cells that are within the visible board
          if (boardY >= 0 && boardY < ROWS && boardX >= 0 && boardX < COLS) {
            this._drawCell(boardX, boardY, color);
          }
        }
      }
    }
  }

  /**
   * Draw the ghost piece (transparent preview of where the piece will land).
   * @param {object} piece - Tetromino instance
   * @param {object} board - Board instance
   */
  _drawGhostPiece(piece, board) {
    const ghostY = board.getGhostY(piece);
    if (ghostY === piece.y) return; // Ghost is same position, no need to draw

    const shape = piece.getShape();
    const color = piece.color;

    for (let row = 0; row < shape.length; row++) {
      for (let col = 0; col < shape[row].length; col++) {
        if (shape[row][col] !== 0) {
          const boardX = piece.x + col;
          const boardY = ghostY + row;

          if (boardY >= 0 && boardY < ROWS && boardX >= 0 && boardX < COLS) {
            this._drawCell(boardX, boardY, color, GHOST_ALPHA);
          }
        }
      }
    }
  }

  /**
   * Draw grid lines on the board.
   */
  _drawGridLines() {
    this.ctx.strokeStyle = GRID_LINE_COLOR;
    this.ctx.lineWidth = 0.5;

    for (let col = 0; col <= COLS; col++) {
      const x = col * CELL_SIZE;
      this.ctx.beginPath();
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, ROWS * CELL_SIZE);
      this.ctx.stroke();
    }

    for (let row = 0; row <= ROWS; row++) {
      const y = row * CELL_SIZE;
      this.ctx.beginPath();
      this.ctx.moveTo(0, y);
      this.ctx.lineTo(COLS * CELL_SIZE, y);
      this.ctx.stroke();
    }
  }

  /**
   * Draw the board border.
   */
  _drawBorder() {
    this.ctx.strokeStyle = BOARD_BORDER_COLOR;
    this.ctx.lineWidth = BOARD_BORDER_WIDTH;
    this.ctx.strokeRect(
      BOARD_BORDER_WIDTH / 2,
      BOARD_BORDER_WIDTH / 2,
      COLS * CELL_SIZE - BOARD_BORDER_WIDTH,
      ROWS * CELL_SIZE - BOARD_BORDER_WIDTH
    );
  }

  /**
   * Draw a semi-transparent overlay with a message.
   * @param {string} title - Main title text
   * @param {string} subtitle - Subtitle text (can include \n)
   */
  _drawOverlay(title, subtitle) {
    const ctx = this.ctx;

    // Dim the board
    ctx.fillStyle = 'rgba(0, 0, 0, 0.65)';
    ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    // Title
    ctx.fillStyle = '#ffffff';
    ctx.font = 'bold 36px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(title, this.canvas.width / 2, this.canvas.height / 2 - 30);

    // Subtitle
    ctx.font = '16px monospace';
    ctx.fillStyle = '#cccccc';

    const lines = subtitle.split('\n');
    lines.forEach((line, i) => {
      ctx.fillText(line, this.canvas.width / 2, this.canvas.height / 2 + 10 + i * 24);
    });
  }

  /**
   * Draw the next piece preview.
   * @param {object} piece - Tetromino instance
   */
  _drawPreview(piece) {
    const ctx = this.previewCtx;
    const canvas = this.previewCanvas;

    // Clear
    ctx.fillStyle = '#1a1a1a';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    const shape = piece.getShapeAt(0); // Always show spawn rotation
    const color = piece.color;

    // Center the piece in the preview
    const rows = shape.length;
    const cols = shape[0].length;
    const offsetX = (canvas.width - cols * CELL_SIZE) / 2;
    const offsetY = (canvas.height - rows * CELL_SIZE) / 2;

    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        if (shape[row][col] !== 0) {
          const x = offsetX + col * CELL_SIZE;
          const y = offsetY + row * CELL_SIZE;
          const size = CELL_SIZE;
          const inset = 1;

          ctx.fillStyle = color;
          ctx.fillRect(x + inset, y + inset, size - inset * 2, size - inset * 2);

          // Highlight
          ctx.fillStyle = 'rgba(255, 255, 255, 0.25)';
          ctx.fillRect(x + inset, y + inset, size - inset * 2, 2);
          ctx.fillRect(x + inset, y + inset, 2, size - inset * 2);

          // Shadow
          ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
          ctx.fillRect(x + inset, y + size - inset - 2, size - inset * 2, 2);
          ctx.fillRect(x + size - inset - 2, y + inset, 2, size - inset * 2);
        }
      }
    }
  }

  /**
   * Update HTML-based HUD elements (score, lines, level).
   * @param {object} scoreManager - ScoreManager instance
   */
  _updateHUD(scoreManager) {
    const stats = scoreManager.getStats();

    const scoreEl = document.getElementById('score-value');
    const linesEl = document.getElementById('lines-value');
    const levelEl = document.getElementById('level-value');

    if (scoreEl) scoreEl.textContent = stats.score;
    if (linesEl) linesEl.textContent = stats.lines;
    if (levelEl) levelEl.textContent = stats.level;
  }
}
