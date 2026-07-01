/**
 * Tetris - Score Manager
 * Handles scoring, level progression, and line counting.
 */
import {
  LINE_SCORES,
  SOFT_DROP_SCORE,
  HARD_DROP_SCORE,
  LINES_PER_LEVEL,
  MAX_LEVEL,
  getDropInterval,
} from './constants.js';

export class ScoreManager {
  constructor() {
    this.score = 0;
    this.lines = 0;
    this.level = 1;
    this.dropInterval = getDropInterval(this.level);
  }

  /**
   * Reset all values to starting state.
   */
  reset() {
    this.score = 0;
    this.lines = 0;
    this.level = 1;
    this.dropInterval = getDropInterval(this.level);
  }

  /**
   * Add score for line clears.
   * @param {number} lineCount - Number of lines cleared (1-4)
   */
  addLineClear(lineCount) {
    if (lineCount < 1 || lineCount > 4) return;

    const points = (LINE_SCORES[lineCount] || 0) * this.level;
    this.score += points;
    this.lines += lineCount;

    this._updateLevel();
  }

  /**
   * Add score for soft drop.
   * @param {number} cellsDropped - Number of cells dropped
   */
  addSoftDrop(cellsDropped) {
    if (cellsDropped < 0) return;
    this.score += cellsDropped * SOFT_DROP_SCORE;
  }

  /**
   * Add score for hard drop.
   * @param {number} cellsDropped - Number of cells dropped
   */
  addHardDrop(cellsDropped) {
    if (cellsDropped < 0) return;
    this.score += cellsDropped * HARD_DROP_SCORE;
  }

  /**
   * Update level based on total lines cleared.
   */
  _updateLevel() {
    const newLevel = Math.min(
      Math.floor(this.lines / LINES_PER_LEVEL) + 1,
      MAX_LEVEL
    );

    if (newLevel !== this.level) {
      this.level = newLevel;
      this.dropInterval = getDropInterval(this.level);
    }
  }

  /**
   * Get the current game stats as a plain object.
   * @returns {{ score: number, lines: number, level: number }}
   */
  getStats() {
    return {
      score: this.score,
      lines: this.lines,
      level: this.level,
    };
  }
}
