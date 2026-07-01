/**
 * Tetris - Game Engine
 * Main game controller: state management, game loop, piece handling.
 */
import { STATE } from './constants.js';
import { Board } from './board.js';
import { Tetromino } from './tetromino.js';
import { InputHandler } from './input.js';
import { Renderer } from './renderer.js';
import { ScoreManager } from './score.js';

export class Game {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {HTMLCanvasElement} previewCanvas
   */
  constructor(canvas, previewCanvas) {
    // Core components
    this.board = new Board();
    this.score = new ScoreManager();
    this.renderer = new Renderer(canvas, previewCanvas);
    this.input = new InputHandler(this);

    // Game state
    this.state = STATE.IDLE;

    // Pieces
    this.currentPiece = null;
    this.nextPiece = null;

    // Timing
    this.lastDropTime = 0;
    this.lastFrameTime = 0;
    this.animationFrameId = null;

    // Bag randomizer (7-bag system)
    this._bag = [];

    // Lock delay
    this._lockDelay = 0;
    this._lockDelayMax = 500; // ms
    this._lockMoves = 0;
    this._lockMovesMax = 15;

    // Setup input callbacks
    this._setupInput();

    // Bind the loop
    this._loop = this._loop.bind(this);
  }

  /**
   * Start the game for the first time.
   */
  start() {
    if (this.state === STATE.PLAYING) return;

    this.board.reset();
    this.score.reset();
    this._bag = [];
    this.currentPiece = null;
    this.nextPiece = null;

    this.nextPiece = new Tetromino(this._getNextFromBag());
    this._spawnPiece();

    this.state = STATE.PLAYING;
    this.lastDropTime = performance.now();
    this.lastFrameTime = performance.now();

    if (!this.animationFrameId) {
      this.animationFrameId = requestAnimationFrame(this._loop);
    }

    // Ensure input is attached
    this.input.attach();
  }

  /**
   * Restart the game (from game over or any state).
   */
  restart() {
    this.state = STATE.IDLE;
    this.start();
  }

  /**
   * Pause the game.
   */
  pause() {
    if (this.state === STATE.PLAYING) {
      this.state = STATE.PAUSED;
      this.input.resetKeys();
    } else if (this.state === STATE.PAUSED) {
      this.state = STATE.PLAYING;
      this.lastDropTime = performance.now();
    }
  }

  /**
   * End the game.
   */
  gameOver() {
    this.state = STATE.GAME_OVER;
    this.input.resetKeys();
    this.input.detach();
  }

  /**
   * Toggle pause.
   */
  togglePause() {
    this.pause();
  }

  // ─── Bag Randomizer (7-bag system) ───

  _refillBag() {
    this._bag = ['I', 'O', 'T', 'S', 'Z', 'J', 'L'];
    for (let i = this._bag.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [this._bag[i], this._bag[j]] = [this._bag[j], this._bag[i]];
    }
  }

  _getNextFromBag() {
    if (this._bag.length === 0) {
      this._refillBag();
    }
    return this._bag.pop();
  }

  /**
   * Spawn a new piece from the next queue.
   * @returns {boolean} True if spawn succeeded, false if game over
   */
  _spawnPiece() {
    this.currentPiece = this.nextPiece || new Tetromino(this._getNextFromBag());
    this.currentPiece.resetPosition();
    this.nextPiece = new Tetromino(this._getNextFromBag());

    // Reset lock delay
    this._lockDelay = 0;
    this._lockMoves = 0;

    // Check if the new piece can be placed
    if (!this.board.isValidPosition(this.currentPiece.getShape(), this.currentPiece.x, this.currentPiece.y)) {
      this.gameOver();
      return false;
    }

    return true;
  }

  // ─── Input Setup ───

  _setupInput() {
    this.input.on('moveLeft', () => this._movePiece(-1, 0));
    this.input.on('moveRight', () => this._movePiece(1, 0));
    this.input.on('softDrop', () => this._softDrop());
    this.input.on('hardDrop', () => this._hardDrop());
    this.input.on('rotateCW', () => this._rotate(1));
    this.input.on('rotateCCW', () => this._rotate(-1));
    this.input.on('pause', () => this.togglePause());
  }

  // ─── Piece Movement ───

  /**
   * Move the current piece by (dx, dy).
   * @param {number} dx - Column change
   * @param {number} dy - Row change
   * @returns {boolean} Whether the move succeeded
   */
  _movePiece(dx, dy) {
    if (this.state !== STATE.PLAYING || !this.currentPiece) return false;

    const shape = this.currentPiece.getShape();
    const newX = this.currentPiece.x + dx;
    const newY = this.currentPiece.y + dy;

    if (this.board.isValidPosition(shape, newX, newY)) {
      this.currentPiece.x = newX;
      this.currentPiece.y = newY;

      // Reset lock delay on horizontal moves
      if (dy === 0) {
        this._lockMoves++;
        if (this._lockMoves <= this._lockMovesMax) {
          this._lockDelay = 0;
        }
      }

      return true;
    }

    return false;
  }

  /**
   * Rotate the current piece.
   * @param {number} direction - 1 for CW, -1 for CCW
   * @returns {boolean} Whether the rotation succeeded
   */
  _rotate(direction) {
    if (this.state !== STATE.PLAYING || !this.currentPiece) return false;

    const fromRotation = this.currentPiece.rotation;
    const toRotation = ((fromRotation + direction) % 4 + 4) % 4;

    const kicks = this.currentPiece.getWallKicks(fromRotation, toRotation);

    for (const [dx, dy] of kicks) {
      const newX = this.currentPiece.x + dx;
      const newY = this.currentPiece.y + dy;
      const newShape = this.currentPiece.getShapeAt(toRotation);

      if (this.board.isValidPosition(newShape, newX, newY)) {
        this.currentPiece.rotation = toRotation;
        this.currentPiece.x = newX;
        this.currentPiece.y = newY;

        // Reset lock moves on successful rotation
        this._lockMoves++;
        if (this._lockMoves <= this._lockMovesMax) {
          this._lockDelay = 0;
        }

        return true;
      }
    }

    return false;
  }

  /**
   * Soft drop: move piece down one row.
   */
  _softDrop() {
    if (this.state !== STATE.PLAYING || !this.currentPiece) return;

    if (this._movePiece(0, 1)) {
      this.score.addSoftDrop(1);
      this._lockDelay = 0;
    }
  }

  /**
   * Hard drop: instantly drop the piece to the lowest valid position.
   */
  _hardDrop() {
    if (this.state !== STATE.PLAYING || !this.currentPiece) return;

    let cellsDropped = 0;
    const shape = this.currentPiece.getShape();

    while (this.board.isValidPosition(shape, this.currentPiece.x, this.currentPiece.y + 1)) {
      this.currentPiece.y++;
      cellsDropped++;
    }

    this.score.addHardDrop(cellsDropped);
    this._lockPiece();
  }

  // ─── Piece Locking ───

  /**
   * Lock the current piece into the board.
   */
  _lockPiece() {
    if (!this.currentPiece) return;

    this.board.lockPiece(this.currentPiece);

    // Clear lines
    const linesCleared = this.board.clearFullLines();
    if (linesCleared > 0) {
      this.score.addLineClear(linesCleared);
    }

    // Spawn next piece
    const success = this._spawnPiece();
    if (!success) {
      return;
    }
  }

  // ─── Main Loop ───

  /**
   * The main game loop, driven by requestAnimationFrame.
   * @param {number} timestamp
   */
  _loop(timestamp) {
    if (this.state === STATE.GAME_OVER || this.state === STATE.IDLE) {
      this.renderer.render(this);
      this.animationFrameId = requestAnimationFrame(this._loop);
      return;
    }

    this.lastFrameTime = timestamp;

    // Process input (DAS repeat)
    this.input.update(timestamp);

    if (this.state === STATE.PLAYING) {
      this._update(timestamp);
    }

    // Render
    this.renderer.render(this);

    // Continue loop
    this.animationFrameId = requestAnimationFrame(this._loop);
  }

  /**
   * Update game logic (gravity, lock delay).
   * @param {number} timestamp
   */
  _update(timestamp) {
    if (!this.currentPiece) return;

    const dropInterval = this.score.dropInterval;

    // Check if the piece can move down
    const canMoveDown = this.board.isValidPosition(
      this.currentPiece.getShape(),
      this.currentPiece.x,
      this.currentPiece.y + 1
    );

    if (canMoveDown) {
      // Normal gravity: auto-drop at the drop interval
      if (timestamp - this.lastDropTime >= dropInterval) {
        this.currentPiece.y++;
        this.lastDropTime = timestamp;
        this._lockDelay = 0;
      }
    } else {
      // Piece is resting on something - start lock delay timer
      if (this._lockDelay === 0) {
        this._lockDelay = timestamp;
      }

      // Lock when lock delay expires
      if (timestamp - this._lockDelay >= this._lockDelayMax) {
        this._lockPiece();
        this.lastDropTime = timestamp;
        this._lockDelay = 0;
      }
    }
  }

  /**
   * Clean up the game loop and input handlers.
   */
  destroy() {
    if (this.animationFrameId) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
    this.input.detach();
  }
}