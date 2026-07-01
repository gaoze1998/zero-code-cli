/**
 * Tetris - Entry Point
 * Initializes the game and sets up global controls.
 */
import { Game } from './game.js';

// ─── DOM References ───
const canvas = document.getElementById('game-canvas');
const previewCanvas = document.getElementById('preview-canvas');
const startBtn = document.getElementById('start-btn');

// ─── Global Game Instance ───
let game = null;

// ─── Initialization ───
function init() {
  if (!canvas || !previewCanvas) {
    console.error('Tetris: Required canvas elements not found.');
    return;
  }

  game = new Game(canvas, previewCanvas);

  // Set up the start/restart button
  if (startBtn) {
    startBtn.addEventListener('click', () => {
      if (game.state === 'idle' || game.state === 'gameover') {
        game.restart();
        updateButtonState();
      }
    });
  }

  // Global keyboard shortcut for ENTER to start/restart
  document.addEventListener('keydown', (e) => {
    if (e.code === 'Enter') {
      if (game.state === 'idle' || game.state === 'gameover') {
        e.preventDefault();
        game.restart();
        updateButtonState();
      }
    }
  });

  // Initial render
  game.renderer.render(game);
  updateButtonState();
}

/**
 * Update the start button text based on game state.
 */
function updateButtonState() {
  if (!startBtn) return;

  switch (game.state) {
    case 'idle':
      startBtn.textContent = 'Start Game';
      break;
    case 'playing':
    case 'paused':
      startBtn.textContent = 'Restart';
      break;
    case 'gameover':
      startBtn.textContent = 'Play Again';
      break;
  }
}

// ─── Boot ───
document.addEventListener('DOMContentLoaded', init);

// Expose game instance for debugging (optional)
window.__tetris = { get game() { return game; } };
