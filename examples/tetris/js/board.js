/**
 * Tetris - Board
 * Manages the 10x20 grid, collision detection, line clearing.
 */
import { COLS, ROWS } from './constants.js';

export class Board {
  constructor() {
    this.grid = this._createEmptyGrid();
  }

  /**
   * Create an empty 2D grid filled with 0.
   * @returns {number[][]}
   */
  _createEmptyGrid() {
    return Array.from({ length: ROWS }, () => Array(COLS).fill(0));
  }

  /**
   * Reset the board to empty.
   */
  reset() {
    this.grid = this._createEmptyGrid();
  }

  /**
   * Get the value at a position.
   * @param {number} col
   * @param {number} row
   * @returns {number}
   */
  getCell(col, row) {
    if (row < 0 || row >= ROWS || col < 0 || col >= COLS) return -1; // out of bounds
    return this.grid[row][col];
  }

  /**
   * Check if placing a piece shape at (x, y) is valid.
   * @param {number[][]} shape - 2D array of the piece
   * @param {number} x - column offset
   * @param {number} y - row offset
   * @returns {boolean}
   */
  isValidPosition(shape, x, y) {
    for (let row = 0; row < shape.length; row++) {
      for (let col = 0; col < shape[row].length; col++) {
        if (shape[row][col] !== 0) {
          const boardX = x + col;
          const boardY = y + row;

          // Check bounds
          if (boardX < 0 || boardX >= COLS || boardY >= ROWS) {
            return false;
          }

          // Allow above the board (negative row) as long as not conflicting
          if (boardY < 0) continue;

          // Check collision with locked cells
          if (this.grid[boardY][boardX] !== 0) {
            return false;
          }
        }
      }
    }
    return true;
  }

  /**
   * Lock a piece into the grid.
   * @param {object} piece - Tetromino instance with shape, x, y, type
   */
  lockPiece(piece) {
    const shape = piece.getShape();
    const typeId = piece.typeId;

    for (let row = 0; row < shape.length; row++) {
      for (let col = 0; col < shape[row].length; col++) {
        if (shape[row][col] !== 0) {
          const boardX = piece.x + col;
          const boardY = piece.y + row;

          if (boardY >= 0 && boardY < ROWS && boardX >= 0 && boardX < COLS) {
            this.grid[boardY][boardX] = typeId;
          }
        }
      }
    }
  }

  /**
   * Check for completed lines and clear them.
   * Iterates from bottom to top to maintain correct indices during splice.
   * @returns {number} Number of lines cleared
   */
  clearFullLines() {
    const fullLines = [];

    for (let row = 0; row < ROWS; row++) {
      if (this.grid[row].every(cell => cell !== 0)) {
        fullLines.push(row);
      }
    }

    if (fullLines.length === 0) return 0;

    // Remove full lines from bottom to top to preserve indices
    for (let i = fullLines.length - 1; i >= 0; i--) {
      this.grid.splice(fullLines[i], 1);
    }

    // Add empty lines at the top
    for (let i = 0; i < fullLines.length; i++) {
      this.grid.unshift(Array(COLS).fill(0));
    }

    return fullLines.length;
  }

  /**
   * Check if any cell in the top 2 rows is filled.
   * Used to detect game over condition after locking a piece.
   * @returns {boolean}
   */
  isTopBlocked() {
    for (let row = 0; row < 2; row++) {
      for (let col = 0; col < COLS; col++) {
        if (this.grid[row][col] !== 0) {
          return true;
        }
      }
    }
    return false;
  }

  /**
   * Get the ghost piece row (where the piece would land).
   * @param {object} piece - Tetromino instance
   * @returns {number} The y-coordinate where the piece would land
   */
  getGhostY(piece) {
    let ghostY = piece.y;
    const shape = piece.getShape();

    while (this.isValidPosition(shape, piece.x, ghostY + 1)) {
      ghostY++;
    }
    return ghostY;
  }

  /**
   * Create a deep clone of the board grid.
   * @returns {number[][]}
   */
  cloneGrid() {
    return this.grid.map(row => [...row]);
  }
}
