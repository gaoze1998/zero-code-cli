/**
 * Tetris - Tetromino
 * Represents a single tetromino piece with rotation states.
 * Uses SRS (Super Rotation System) with wall kicks.
 */
import {
  TYPES, SHAPES, COLORS, SPAWN,
  WALL_KICKS_JLSZT, WALL_KICKS_I,
} from './constants.js';

export class Tetromino {
  /**
   * Create a new tetromino of the given type.
   * @param {string} type - One of 'I', 'O', 'T', 'S', 'Z', 'J', 'L'
   */
  constructor(type) {
    if (!TYPES.includes(type)) {
      throw new Error(`Invalid tetromino type: ${type}`);
    }

    this.type = type;
    this.typeId = TYPES.indexOf(type) + 1; // 1-7
    this.color = COLORS[type];

    // Rotation state: 0 = spawn, 1 = CW, 2 = 180, 3 = CCW
    this.rotation = 0;

    // Compute all 4 rotation states from the base shape
    this.rotationStates = this._computeRotationStates(SHAPES[type]);

    // Spawn position
    const spawn = SPAWN[type];
    this.x = spawn.x;
    this.y = spawn.y;
  }

  /**
   * Compute all 4 rotation states for a shape.
   * @param {number[][]} baseShape - The 0-degree shape matrix
   * @returns {number[][][]} Array of 4 shape matrices
   */
  _computeRotationStates(baseShape) {
    const states = [baseShape];

    // For O piece, all rotations are the same
    if (this.type === 'O') {
      return [baseShape, baseShape, baseShape, baseShape];
    }

    let current = baseShape;
    for (let i = 1; i < 4; i++) {
      current = this._rotateCW(current);
      states.push(current);
    }

    return states;
  }

  /**
   * Rotate a matrix 90 degrees clockwise.
   * @param {number[][]} matrix
   * @returns {number[][]}
   */
  _rotateCW(matrix) {
    const n = matrix.length;
    const rotated = Array.from({ length: n }, () => Array(n).fill(0));

    for (let row = 0; row < n; row++) {
      for (let col = 0; col < n; col++) {
        rotated[col][n - 1 - row] = matrix[row][col];
      }
    }

    return rotated;
  }

  /**
   * Get the current shape matrix based on rotation state.
   * @returns {number[][]}
   */
  getShape() {
    return this.rotationStates[this.rotation];
  }

  /**
   * Get a specific rotation state's shape.
   * @param {number} rotation - 0-3
   * @returns {number[][]}
   */
  getShapeAt(rotation) {
    return this.rotationStates[((rotation % 4) + 4) % 4];
  }

  /**
   * Get wall kick offsets for a given rotation transition.
   * @param {number} fromRotation - Current rotation state
   * @param {number} toRotation - Target rotation state
   * @returns {number[][]} Array of [colOffset, rowOffset] pairs
   */
  getWallKicks(fromRotation, toRotation) {
    const key = `${fromRotation}>${toRotation}`;
    const kicks = this.type === 'I' ? WALL_KICKS_I : WALL_KICKS_JLSZT;
    return kicks[key] || [[0, 0]];
  }

  /**
   * Reset position to spawn.
   */
  resetPosition() {
    const spawn = SPAWN[this.type];
    this.x = spawn.x;
    this.y = spawn.y;
    this.rotation = 0;
  }

  /**
   * Clone this tetromino.
   * @returns {Tetromino}
   */
  clone() {
    const copy = new Tetromino(this.type);
    copy.x = this.x;
    copy.y = this.y;
    copy.rotation = this.rotation;
    return copy;
  }

  /**
   * Static factory: create a random tetromino.
   * @param {function} [rng] - Optional random number generator (0-1)
   * @returns {Tetromino}
   */
  static random(rng = Math.random) {
    const type = TYPES[Math.floor(rng() * TYPES.length)];
    return new Tetromino(type);
  }
}
