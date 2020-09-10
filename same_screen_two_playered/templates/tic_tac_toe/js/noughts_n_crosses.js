// Superclass for the large grid
class Grid {
    constructor() {
      //0 = empty, 1 = X, 2=0
      this.arr_grid_vals = [0,0,0,0,0,0,0,0,0]
  
    }
    checkWon(XorO) {
      var check_list = []
      var first_list = []
      var second_list = []
  
  
      //create lists of values we need
      for(i = 0; i<9; i++){
        if (this.arr_grid_vals[i] == XorO){
          check_list.push(to_tenary(i)[0].toString()+to_tenary(i)[1].toString())
          first_list.push(to_tenary(i)[0])
          second_list.push(to_tenary(i)[1])
        }
      }
  
      //check for horizontal and vertical 3 in a row
      if(this.threeAnalysis(first_list)){
        return true
      }
  
      if(this.threeAnalysis(second_list)){
        return true
      }
  
      //diagonal check
      if ((check_list.includes("00")&&check_list.includes("11"))&&check_list.includes("22")){
        return true
      }
  
      if ((check_list.includes("02")&&check_list.includes("11"))&&check_list.includes("20")){
        return true
      }
  
      return false
    };
  
    threeAnalysis(input_list){
        input_list.sort()
        var len = input_list.length;
        for (i = 0; i< (len-2); i++){
          if (input_list[i] == input_list[i+2]){
            return true
          }
        }
        return false
    };
  
  // Check if a grid is full
  
    isFull() {
      for (i=0;i<9;i++) {
        if (this.arr_grid_vals[i]==0) {
          return false
        }
      }
      return true
    };
  }
  
  class BigGrid extends Grid {
    constructor() {
      super()
    }
  }
  
  class SmallGrid extends Grid {
    constructor() {
      super()
      this.full = false
      this.won = false
    }
  }
  
  
  function change_colour(square_position) {
    var id_string
    for (i=0;i<9;i++){
      for (j=0;j<9;j++){
        id_string = i.toString()+j.toString()
        document.getElementById(id_string).style.backgroundColor = "white";
        if (i==square_position) {
          document.getElementById(id_string).style.backgroundColor = "yellow";      }
      }
    }
  
  
  };
  
  function is_playerX_turn(turn){
    if(turn%2==1){
      return 1
    }
    else{
      return 2
    }
  };
  
  function to_tenary(decimal_numb){
    var first = Math.floor(decimal_numb/3)
    var second = decimal_numb%3
    return[first, second]
  };
  
  function disable_all_buttons(){
    var id_string
    for (i=0;i<9;i++){
      for (j=0;j<9;j++){
        id_string = i.toString()+j.toString()
        document.getElementById(id_string).disabled = true;
      }
    }
  };
  
  function highlight_avail(arrr_grid_vals){
    var id_string
    for (i=0;i<9;i++){
      for (j=0;j<9;j++){
        id_string = i.toString()+j.toString()
        document.getElementById(id_string).style.backgroundColor = "white";
        if(!arrr_grid_vals[i].full){
          document.getElementById(id_string).style.backgroundColor = "yellow";
        }
      }
    }
  };
  
  let bigGrid = new BigGrid();
  arr_small_grids=[]
  for (i=0;i<9;i++){
    let tempObj = new SmallGrid();
    arr_small_grids.push(tempObj);
  }
  
  //global variables
  var turn = 0
  var next_square =9
  
  function move_made(buttObj) {
  
    //string manipulation to extract small grid and square
    small_grid_num=parseInt(buttObj.id.slice(0,1))
    square_position= parseInt(buttObj.id.slice(1,2))
  
    //Check legal
    if (!(next_square == small_grid_num || next_square == 9)){
      return
    }
    next_square = square_position
  
    turn=turn+1
  
    //determine whose turn
    XorO = is_playerX_turn(turn)
  
  
    //assign correct small grid square value
    arr_small_grids[small_grid_num].arr_grid_vals[square_position] = XorO
  
  
    //check small grid is full
    arr_small_grids[small_grid_num].full = arr_small_grids[small_grid_num].isFull()
    if (arr_small_grids[square_position].full==true){
        next_square=9
    }
  
    //change current display
    document.getElementById(buttObj.id).innerHTML = "O";
    document.getElementById(buttObj.id).style.color = "red";
    if(XorO == 1) {
      document.getElementById(buttObj.id).innerHTML = "X";
      document.getElementById(buttObj.id).style.color = "green";
    }
  
    //highlight next square
    change_colour(square_position)
    if(next_square==9){
      highlight_avail(arr_small_grids)
    }
  
    //disable button
    document.getElementById(buttObj.id).disabled = true;
  
    //check for small grid finish & overall finish
    if(!(arr_small_grids[small_grid_num].won)){
      if(arr_small_grids[small_grid_num].checkWon(XorO)){
        document.getElementById(small_grid_num).style.backgroundColor = "green";
        if(XorO==2){
          document.getElementById(small_grid_num).style.backgroundColor = "red";
        }
        bigGrid.arr_grid_vals[small_grid_num]=XorO
        arr_small_grids[small_grid_num].won=true
        if(bigGrid.checkWon(XorO)){
          disable_all_buttons()
          window.alert("YOU WON")
          location.reload();
        }
  
      }
    }
  
  
  
    //won = arr_small_grids[small_grid_num].checkWon(XorO)
    //document.getElementById("demo").innerHTML = arr_small_grids[small_grid_num].checkWon(XorO);
  
  
    //Disable button
  
  };