/**
 * Tetris - Input Handler
 * Manages keyboard input with DAS (Delayed Auto Shift) support.
 */
import { DAS_DELAY, DAS_INTERVAL } from './constants.js';

/**
 * Map of key codes to game actions.
 */
const KEY_MAP = {
  ArrowLeft: 'moveLeft',
  ArrowRight: 'moveRight',
  ArrowDown: 'softDrop',
  ArrowUp: 'rotateCW',
  KeyZ: 'rotateCCW',
  Space: 'hardDrop',
  KeyP: 'pause',
};

/**
 * Actions that should only fire once per key press (no repeat).
 */
const TAP_ACTIONS = new Set(['hardDrop', 'rotateCW', 'rotateCCW', 'pause']);

export class InputHandler {
  /**
   * @param {object} game - Reference to the Game instance for callbacks
   */
  constructor(game) {
    this.game = game;

    // Currently pressed keys with timing info
    this._keys = {}; // { code: { pressed: bool, time: timestamp, handled: bool } }

    // Callback functions registered by the game
    this._callbacks = {};

    // Bound event handlers
    this._onKeyDown = this._handleKeyDown.bind(this);
    this._onKeyUp = this._handleKeyUp.bind(this);
  }

  /**
   * Register a callback for a named action.
   * @param {string} action - e.g., 'moveLeft', 'rotateCW'
   * @param {function} callback
   */
  on(action, callback) {
    this._callbacks[action] = callback;
  }

  /**
   * Start listening for keyboard events.
   */
  attach() {
    document.addEventListener('keydown', this._onKeyDown);
    document.addEventListener('keyup', this._onKeyUp);
  }

  /**
   * Stop listening for keyboard events.
   */
  detach() {
    document.removeEventListener('keydown', this._onKeyDown);
    document.removeEventListener('keyup', this._onKeyUp);
    this._keys = {};
  }

  /**
   * Handle keydown events.
   * @param {KeyboardEvent} e
   */
  _handleKeyDown(e) {
    const action = KEY_MAP[e.code];
    if (!action) return;

    e.preventDefault();

    // Store key state with timestamp
    if (!this._keys[e.code]) {
      this._keys[e.code] = {
        pressed: true,
        time: performance.now(),
        dasTriggered: false,
      };
    }

    // Fire the action
    this._fireAction(action, e.code);
  }

  /**
   * Handle keyup events.
   * @param {KeyboardEvent} e
   */
  _handleKeyUp(e) {
    if (KEY_MAP[e.code]) {
      e.preventDefault();
    }
    delete this._keys[e.code];
  }

  /**
   * Fire an action callback if registered.
   * @param {string} action
   * @param {string} code
   */
  _fireAction(action, code) {
    const cb = this._callbacks[action];
    if (cb) {
      cb();
    }

    // For tap actions, immediately mark as handled to prevent repeats
    if (TAP_ACTIONS.has(action) && this._keys[code]) {
      this._keys[code].handled = true;
    }
  }

  /**
   * Update method: called every frame to handle DAS repeat.
   * This processes held keys for repeatable actions.
   * @param {number} now - Current timestamp from requestAnimationFrame
   */
  update(now) {
    for (const code of Object.keys(this._keys)) {
      const key = this._keys[code];
      if (!key.pressed) continue;

      const action = KEY_MAP[code];
      if (!action) continue;

      // Skip tap-only actions
      if (TAP_ACTIONS.has(action)) {
        if (key.handled) continue;
        // For tap actions, only fire once when first pressed
        // (already handled in _handleKeyDown)
        continue;
      }

      // DAS: delay then repeat
      const elapsed = now - key.time;

      if (elapsed < DAS_DELAY) {
        // Still in initial delay, no repeat yet
        continue;
      }

      if (!key.dasTriggered) {
        // First repeat after DAS delay
        key.dasTriggered = true;
        key.lastRepeat = now;
        this._fireAction(action, code);
      } else {
        // Subsequent repeats at DAS_INTERVAL
        const timeSinceLastRepeat = now - (key.lastRepeat || 0);
        if (timeSinceLastRepeat >= DAS_INTERVAL) {
          key.lastRepeat = now;
          this._fireAction(action, code);
        }
      }
    }
  }

  /**
   * Reset all key states (useful on pause/unpause).
   */
  resetKeys() {
    this._keys = {};
  }
}
