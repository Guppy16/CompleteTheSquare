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
  }

  valid(pos) {
    const occupied = this.p1.pieces.has(pos) || this.p2.pieces.has(pos);
    const [r,c] = pos.split('').map(Number);
    return !occupied && r>=1 && r<=5 && c>=1 && c<=5;
  }

  // Get all possible corner positions that form a square with the given position
  getSquareCorners(pos) {
    const [r, c] = pos.split('').map(Number);
    const corners = [];
    
    // Check all possible square sizes
    for (let size = 1; size <= 4; size++) {
      // Check all four possible squares with this position as a corner
      
      // Square with (r,c) as top-left
      if (r+size <= 5 && c+size <= 5) {
        corners.push([
          `${r}${c}`,
          `${r}${c+size}`,
          `${r+size}${c}`,
          `${r+size}${c+size}`
        ]);
      }
      
      // Square with (r,c) as top-right
      if (r+size <= 5 && c-size >= 1) {
        corners.push([
          `${r}${c}`,
          `${r}${c-size}`,
          `${r+size}${c}`,
          `${r+size}${c-size}`
        ]);
      }
      
      // Square with (r,c) as bottom-left
      if (r-size >= 1 && c+size <= 5) {
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
    // For now, let's just implement a placeholder that doesn't remove any pieces
    // You can implement the actual removal logic later
    return;
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

let game = new Game(new Player("You", "W"), new Player("AI", "B"));

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
  // Update to show color name instead of W/B, with colored text
  statusEl.innerHTML = createColoredStatus(game.current.colour);
  resetButtonContainer.innerHTML = '';
  
  // create 5×5 cells
  for (let r=1; r<=5; r++) {
    for (let c=1; c<=5; c++) {
      const cell = document.createElement('div');
      cell.classList.add('cell');
      cell.dataset.pos = `${r}${c}`;
      cell.addEventListener('click', () => {
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
      });
      boardEl.appendChild(cell);
    }
  }
}

// Initialize the board
setupBoard();
