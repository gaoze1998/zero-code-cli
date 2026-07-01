# Tetris

A classic Tetris game built with vanilla JavaScript, HTML5 Canvas, and CSS3.

## Features

- **7 standard Tetrominos** (I, O, T, S, Z, J, L) with classic colors
- **SRS (Super Rotation System)** with full wall kick tables
- **7-Bag Randomizer** for fair piece distribution
- **Ghost piece** showing landing position
- **Lock delay** with move reset (15 moves max, 500ms timeout)
- **DAS (Delayed Auto Shift)** for smooth horizontal movement
- **Scoring system** with level progression
- **Next piece preview**
- **Keyboard controls** with full game state management

## Controls

| Key | Action |
|-----|--------|
| ← → | Move piece left/right |
| ↑ | Rotate clockwise |
| Z | Rotate counter-clockwise |
| ↓ | Soft drop (+1 point per cell) |
| Space | Hard drop (instant) |
| P | Pause / Resume |
| Enter | Start / Restart game |

## Scoring

| Action | Points |
|--------|--------|
| Single (1 line) | 100 × level |
| Double (2 lines) | 300 × level |
| Triple (3 lines) | 500 × level |
| Tetris (4 lines) | 800 × level |
| Soft drop | 1 per cell |
| Hard drop | 2 per cell |

- Level increases every 10 lines cleared
- Game speed increases with each level (max level 15)

## How to Run

Simply serve the directory with any HTTP server and open in a browser:

```bash
# Python 3
python3 -m http.server 8000

# Node.js
npx serve .
```

Then open http://localhost:8000 in your browser.

## Project Structure

```
├── index.html       # Main HTML page
├── css/
│   └── style.css    # Dark theme styling
├── js/
│   ├── constants.js # Game constants, shapes, colors, wall kicks
│   ├── board.js     # Board grid, collision, line clearing
│   ├── tetromino.js # Piece definition, rotation states
│   ├── input.js     # Keyboard input with DAS
│   ├── renderer.js  # Canvas rendering
│   ├── score.js     # Scoring, levels, speed
│   ├── game.js      # Game engine, main loop
│   └── main.js      # Entry point, initialization
└── README.md
```

## Technical Details

- **Zero dependencies** — pure HTML5/CSS3/JavaScript (ES modules)
- **Canvas 2D API** for rendering (300×600 game board)
- **requestAnimationFrame** driven game loop
- **ES modules** for clean code organization
- **Responsive layout** adapts to mobile screens
