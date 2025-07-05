const BOARD_SIZE = 5; // Configurable board size

const url = 'https://completethesquare-32441666402.europe-west1.run.app'
// const url = 'http://localhost:5000'

function getBitboardsFromGame(game) {
  // Returns [playerW_bitboard, playerB_bitboard]
  let w = 0, b = 0;
  for (let r = 1; r <= BOARD_SIZE; r++) {
    for (let c = 1; c <= BOARD_SIZE; c++) {
      const pos = `${r}${c}`;
      const bit = 1 << ((r - 1) * BOARD_SIZE + (c - 1));
      if (game.p1.pieces.has(pos)) w |= bit;
      if (game.p2.pieces.has(pos)) b |= bit;
    }
  }
  return [w, b];
}

async function getAIMove(game) {
  console.log("Requesting AI move...");

  // Determine current player: 0 for W, 1 for B
  const current_player = game.current.colour === 'W' ? 0 : 1;
  const boards = getBitboardsFromGame(game);

  const response = await fetch(`${url}/ai-move`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ boards, current_player })
  });
  const data = await response.json();
  return data.move; // [row, col]
}

class Player {
  constructor(name, colour) {
    this.name = name;
    this.colour = colour;      // 'W' or 'B'
    this.pieces = new Set();   // store positions as strings "11", "25", etc.
  }
  add(pos) { this.pieces.add(pos); }
}

class Game {
  constructor(p1, p2) {
    this.p1 = p1;
    this.p2 = p2;
    this.allMoves = [];
    this.current = p1;
    this.winningSquare = null;
    this.boardSize = BOARD_SIZE; // Store board size in the game object
  }

  valid(pos) {
    const occupied = this.p1.pieces.has(pos) || this.p2.pieces.has(pos);
    const [r,c] = pos.split('').map(Number);
    return !occupied && r>=1 && r<=this.boardSize && c>=1 && c<=this.boardSize;
  }

  // Get all possible corner positions that form a square with the given position
  getSquareCorners(pos) {
    const [r, c] = pos.split('').map(Number);
    const corners = [];
    
    // Check all possible square sizes
    for (let size = 1; size <= this.boardSize-1; size++) {
      // Check all four possible squares with this position as a corner
      
      // Square with (r,c) as top-left
      if (r+size <= this.boardSize && c+size <= this.boardSize) {
        corners.push([
          `${r}${c}`,
          `${r}${c+size}`,
          `${r+size}${c}`,
          `${r+size}${c+size}`
        ]);
      }
      
      // Square with (r,c) as top-right
      if (r+size <= this.boardSize && c-size >= 1) {
        corners.push([
          `${r}${c}`,
          `${r}${c-size}`,
          `${r+size}${c}`,
          `${r+size}${c-size}`
        ]);
      }
      
      // Square with (r,c) as bottom-left
      if (r-size >= 1 && c+size <= this.boardSize) {
        corners.push([
          `${r}${c}`,
          `${r}${c+size}`,
          `${r-size}${c}`,
          `${r-size}${c+size}`
        ]);
      }
      
      // Square with (r,c) as bottom-right
      if (r-size >= 1 && c-size >= 1) {
        corners.push([
          `${r}${c}`,
          `${r}${c-size}`,
          `${r-size}${c}`,
          `${r-size}${c-size}`
        ]);
      }
    }
    
    return corners;
  }

  checkWin(pos, player) {
    const possibleSquares = this.getSquareCorners(pos);
    
    for (const square of possibleSquares) {
      if (square.every(corner => player.pieces.has(corner))) {
        this.winningSquare = square; // Store the winning square
        return true;
      }
    }
    
    return false;
  }
  
  removePieces(pos) {
    const [row, col] = pos.split('').map(Number);
    const currentPlayer = this.current;
    const opponentPlayer = currentPlayer === this.p1 ? this.p2 : this.p1;
    let piecesRemoved = false;
    
    // Define all possible directions
    const directions = [
      [1, 0], [1, 1], [0, 1], [-1, 1],
      [-1, 0], [-1, -1], [0, -1], [1, -1]
    ];
    
    // Check each direction
    for (const [dx, dy] of directions) {
      let capturedPieces = [];
      let x = row + dx;
      let y = col + dy;
      
      // Collect opponent pieces in this direction
      while (
        x >= 1 && x <= this.boardSize && 
        y >= 1 && y <= this.boardSize && 
        opponentPlayer.pieces.has(`${x}${y}`)
      ) {
        capturedPieces.push(`${x}${y}`);
        x += dx;
        y += dy;
      }
      
      // If we found opponent pieces and ended at one of our pieces, capture them
      if (
        capturedPieces.length > 0 && 
        x >= 1 && x <= this.boardSize && 
        y >= 1 && y <= this.boardSize && 
        currentPlayer.pieces.has(`${x}${y}`)
      ) {
        // Remove captured pieces
        for (const capturedPos of capturedPieces) {
          opponentPlayer.pieces.delete(capturedPos);
          
          // Update the UI
          const cellToUpdate = document.querySelector(`.cell[data-pos="${capturedPos}"]`);
          if (cellToUpdate) {
            cellToUpdate.classList.remove(`player-${opponentPlayer.colour}`, 'disabled');
            // Make the cell clickable again
            cellToUpdate.classList.remove('disabled');
          }
        }
        
        piecesRemoved = true;
      }
    }
    
    return piecesRemoved;
  }

  makeMove(pos) {
    if (!this.valid(pos)) {
      alert("Invalid move!");
      return false;
    }
    this.current.add(pos);
    this.allMoves.push(pos);
    const won = this.checkWin(pos, this.current);
    this.removePieces(pos);
    this.current = (this.current === this.p1 ? this.p2 : this.p1);
    return won;
  }
  
  reset() {
    this.p1.pieces.clear();
    this.p2.pieces.clear();
    this.allMoves = [];
    this.current = this.p1;
    this.winningSquare = null;
    return this;
  }
}

// wire up the board UI:
const boardEl = document.getElementById('board');
const statusEl = document.getElementById('status');
const resetButtonContainer = document.createElement('div');
document.body.appendChild(resetButtonContainer);

// Set CSS variables for board size - add this line
document.documentElement.style.setProperty('--board-size', BOARD_SIZE);

let game = new Game(new Player("You", "W"), new Player("AI", "B"));

let playAgainstAI = false; // Default: human vs human

const aiToggle = document.getElementById('ai-toggle');
const toggleLabel = document.getElementById('toggle-label');

aiToggle.addEventListener('change', () => {
  playAgainstAI = aiToggle.checked;
  if (playAgainstAI) {
    toggleLabel.textContent = "Play vs AI";
    game = new Game(new Player("You", "W"), new Player("AI", "B"));
  } else {
    toggleLabel.textContent = "Play vs Human";
    game = new Game(new Player("You", "W"), new Player("Opponent", "B"));
  }
  setupBoard();
});

// Add this helper function to convert W/B to actual color names
function getColorName(colorCode) {
  return colorCode === 'W' ? 'Green' : 'Red';
}

// Add a function to get the hex color value
function getHexColor(colorCode) {
  return colorCode === 'W' ? '#4CAF50' : '#F44336'; // Green for W, Red for B
}

// Helper function to create status messages with color highlights
function createColoredStatus(colorCode, isWinner = false) {
  const colorName = getColorName(colorCode);
  const hexColor = getHexColor(colorCode);
  
  if (isWinner) {
    return `<span style="background-color: ${hexColor}50; padding: 2px 8px; border-radius: 4px; text-shadow: 0 0 3px ${hexColor}80;">${colorName} wins!</span>`;
  } else {
    return `<span style="background-color: ${hexColor}50; padding: 2px 8px; border-radius: 4px; text-shadow: 0 0 3px ${hexColor}80;">${colorName}</span> to move`;
  }
}

function setupBoard() {
  boardEl.innerHTML = '';
  statusEl.innerHTML = createColoredStatus(game.current.colour);
  resetButtonContainer.innerHTML = '';
  
  // create BOARD_SIZE × BOARD_SIZE cells
  for (let r=1; r<=BOARD_SIZE; r++) {
    for (let c=1; c<=BOARD_SIZE; c++) {
      const cell = document.createElement('div');
      cell.classList.add('cell');
      cell.dataset.pos = `${r}${c}`;
      cell.addEventListener('click', () => {
        console.log(`Clicked cell at position: ${cell.dataset.pos}`);
        const pos = cell.dataset.pos;
        if (!game.valid(pos)) return; // Skip invalid moves

        // Store the current player's color BEFORE making the move
        const currentPlayerColor = game.current.colour;
        
        // Make the move and update the current player
        const finished = game.makeMove(pos);
        
        // Instead of setting text content, add a player-specific class
        cell.classList.add(`player-${currentPlayerColor}`, 'disabled');
        
        // Update status message with colored text
        statusEl.innerHTML = createColoredStatus(game.current.colour);

        if (finished) {
          // Highlight winning square
          game.winningSquare.forEach(cornerPos => {
            const cornerCell = document.querySelector(`.cell[data-pos="${cornerPos}"]`);
            cornerCell.classList.add('winner');
          });
          
          // Show win message with colored text
          statusEl.innerHTML = createColoredStatus(currentPlayerColor, true);
          
          // Disable all cells
          boardEl.querySelectorAll('.cell').forEach(c => c.classList.add('disabled'));
          
          // Show reset button
          const resetButton = document.createElement('button');
          resetButton.textContent = 'Play Again';
          resetButton.addEventListener('click', () => {
            game.reset();
            setupBoard();
          });
          resetButtonContainer.appendChild(resetButton);
        }

        // Only trigger AI if playing vs AI and it's AI's turn
        console.log(`Current player: ${game.current.name}`);
        if (playAgainstAI && game.current.name === "AI" && !finished) {
          getAIMove(game)
            .then(([row, col]) => {
              const aiPos = `${row + 1}${col + 1}`; // Bitboard is 0-indexed, JS is 1-indexed
              const aiCell = document.querySelector(`.cell[data-pos="${aiPos}"]`);
              if (aiCell && !aiCell.classList.contains('disabled')) {
                aiCell.click();
              }
            });
        }
      });
      boardEl.appendChild(cell);
    }
  }
}

// Initialize the board with default mode
setupBoard();
